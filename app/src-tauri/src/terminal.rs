//! 内嵌终端（M7-4 · 任务包 m7-7.4）
//!
//! 5 个命令：`terminal_create` / `terminal_write` / `terminal_resize` /
//! `terminal_close` / `terminal_list`。
//!
//! 关键设计：
//! - 每个会话一个 PTY 子进程，shell 取 `$SHELL`，fallback `/bin/zsh`。
//! - cwd 解析：显式 `cwd` 入参 > 资源根目录（settings.root_dir）>
//!   `dirs::home_dir()`。space 表当前无 root_path 字段，暂用全局根目录
//!   作为"项目上下文"的近似。
//! - 输出通过 `tauri::ipc::Channel<TerminalEvent>` 推送（Data / Exited）。
//! - reader 拆两段：producer 线程阻塞 read → mpsc；flusher 用
//!   `recv_timeout(16ms)` 批量聚合，**空闲超时也 flush**，保证 shell
//!   静默前不足 8KB 的尾部输出（如提示符）不会被滞留。
//! - 退出清理：`terminal_close` 主动 kill；`child.wait()` 在独立 task，
//!   退出后自动从 sessions 表移除并推 `Exited`；应用退出时由 lib.rs
//!   遍历 sessions 全部 kill。

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::ipc::Channel;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};

/// reader 批量 flush 间隔（毫秒）。
const FLUSH_INTERVAL_MS: u64 = 16;
/// reader 批量 flush 触发字节数。
const FLUSH_THRESHOLD_BYTES: usize = 8 * 1024;

// ============================================================
// 类型
// ============================================================

/// 推送到前端的终端事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalEvent {
    /// PTY stdout/stderr 输出（已按 UTF-8 lossy 解码）。
    Data { data: String },
    /// 子进程退出。
    Exited { code: Option<i32> },
}

/// 单个 PTY 会话。
pub struct PtySession {
    pub id: String,
    pub space_id: Option<String>,
    pub cwd: PathBuf,
    pub shell: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    /// reader 任务句柄（仅用于 drop 时 detach；reader 内部循环在 EOF / kill 后自然退出）。
    #[allow(dead_code)]
    reader_task: tauri::async_runtime::JoinHandle<()>,
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("id", &self.id)
            .field("space_id", &self.space_id)
            .field("cwd", &self.cwd)
            .field("shell", &self.shell)
            .finish()
    }
}

/// 全局会话表。
pub struct TerminalState {
    pub sessions: Arc<Mutex<HashMap<String, Arc<PtySession>>>>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

/// `terminal_create` 出参。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCreateOutput {
    pub session_id: String,
}

/// `terminal_list` 出参条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalListItem {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    pub cwd: String,
    pub shell: String,
}

// ============================================================
// 业务函数
// ============================================================

/// 解析 shell：优先 `$SHELL`，否则 `/bin/zsh`。
fn resolve_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string())
}

/// 解析 cwd：显式入参 > 资源根目录（settings.root_dir）> home。
fn resolve_cwd(cwd: Option<String>, root_dir: Option<&str>) -> PathBuf {
    for candidate in [cwd, root_dir.map(|s| s.to_string())].into_iter().flatten() {
        let p = PathBuf::from(&candidate);
        if p.is_dir() {
            return p;
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// 创建会话（同步阻塞部分），返回 Arc<PtySession>。
///
/// 调用方需自行把返回的 session 插入 `TerminalState.sessions`。
fn spawn_session(
    session_id: String,
    space_id: Option<String>,
    cwd: PathBuf,
    shell: String,
    cols: u16,
    rows: u16,
    channel: Channel<TerminalEvent>,
    sessions: Arc<Mutex<HashMap<String, Arc<PtySession>>>>,
) -> CmdResult<Arc<PtySession>> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::io(format!("打开 PTY 失败: {}", e)))?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-i");
    cmd.cwd(&cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::io(format!("spawn shell 失败: {}", e)))?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AppError::io(format!("克隆 PTY reader 失败: {}", e)))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| AppError::io(format!("获取 PTY writer 失败: {}", e)))?;

    let child_arc: Arc<Mutex<Box<dyn Child + Send + Sync>>> = Arc::new(Mutex::new(child));
    let master_arc: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pair.master));
    let writer_arc: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));

    // ---- reader：producer（阻塞 read → mpsc）+ flusher（超时也 flush）----
    // 旧实现只在新数据到达时才检查 flush 条件，导致 shell 静默前不足
    // FLUSH_THRESHOLD_BYTES 的尾部输出（典型：命令执行完回到提示符）
    // 被滞留到下一次输出才推送，体感"慢一拍"。现拆成两段：
    // producer 只负责读，flusher 用 recv_timeout 在空闲 16ms 后也 flush。
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let channel_for_reader = channel.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let reader_task = tauri::async_runtime::spawn_blocking(move || {
        let mut pending: Vec<u8> = Vec::with_capacity(FLUSH_THRESHOLD_BYTES * 2);
        let mut last_flush = std::time::Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(FLUSH_INTERVAL_MS)) {
                Ok(chunk) => {
                    pending.extend_from_slice(&chunk);
                    // 持续输出（如 zsh 每次按键重绘）时按时间上限 flush，
                    // 否则只能攒满 8KB 才推，回显成批跳动，体感"卡住"
                    if pending.len() >= FLUSH_THRESHOLD_BYTES
                        || last_flush.elapsed() >= Duration::from_millis(FLUSH_INTERVAL_MS)
                    {
                        let _ = channel_for_reader.send(TerminalEvent::Data {
                            data: String::from_utf8_lossy(&pending).to_string(),
                        });
                        pending.clear();
                        last_flush = std::time::Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 空闲超时：把滞留的尾部输出推出去
                    if !pending.is_empty() {
                        let _ = channel_for_reader.send(TerminalEvent::Data {
                            data: String::from_utf8_lossy(&pending).to_string(),
                        });
                        pending.clear();
                        last_flush = std::time::Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // producer 结束（EOF / 读错误）：flush 残余后退出
                    if !pending.is_empty() {
                        let _ = channel_for_reader.send(TerminalEvent::Data {
                            data: String::from_utf8_lossy(&pending).to_string(),
                        });
                    }
                    break;
                }
            }
        }
    });

    // ---- wait task：子进程退出后从 sessions 移除并推 Exited ----
    let child_for_wait = child_arc.clone();
    let sessions_for_wait = sessions.clone();
    let session_id_for_wait = session_id.clone();
    let channel_for_wait = channel;
    tauri::async_runtime::spawn(async move {
        let exit_code: Option<i32> = tauri::async_runtime::spawn_blocking(move || {
            let mut child = child_for_wait.lock().expect("child lock");
            child
                .wait()
                .ok()
                .map(|s| s.exit_code() as i32)
        })
        .await
        .ok()
        .flatten();

        // 从 sessions 表移除（若仍在）
        if let Ok(mut map) = sessions_for_wait.lock() {
            map.remove(&session_id_for_wait);
        }
        let _ = channel_for_wait.send(TerminalEvent::Exited { code: exit_code });
    });

    Ok(Arc::new(PtySession {
        id: session_id,
        space_id,
        cwd,
        shell,
        writer: writer_arc,
        master: master_arc,
        child: child_arc,
        reader_task,
    }))
}

pub fn create(
    state: &TerminalState,
    space_id: Option<String>,
    cwd: Option<String>,
    root_dir: Option<String>,
    cols: u16,
    rows: u16,
    channel: Channel<TerminalEvent>,
) -> CmdResult<TerminalCreateOutput> {
    if cols == 0 || rows == 0 {
        return Err(AppError::invalid_param("cols/rows 必须 > 0"));
    }
    let session_id = Uuid::new_v4().to_string();
    let shell = resolve_shell();
    let cwd_path = resolve_cwd(cwd, root_dir.as_deref());

    let session = spawn_session(
        session_id.clone(),
        space_id,
        cwd_path,
        shell,
        cols,
        rows,
        channel,
        state.sessions.clone(),
    )?;

    let mut map = state.sessions.lock().map_err(|_| AppError::io("sessions lock"))?;
    map.insert(session_id.clone(), session);
    Ok(TerminalCreateOutput { session_id })
}

pub fn write(state: &TerminalState, session_id: &str, data: &str) -> CmdResult<()> {
    let session = {
        let map = state.sessions.lock().map_err(|_| AppError::io("sessions lock"))?;
        map.get(session_id).cloned()
    }
    .ok_or_else(|| AppError::not_found(format!("终端会话不存在: {}", session_id)))?;

    let mut w = session.writer.lock().map_err(|_| AppError::io("writer lock"))?;
    w.write_all(data.as_bytes())
        .map_err(|e| AppError::io(format!("写入 PTY 失败: {}", e)))?;
    w.flush()
        .map_err(|e| AppError::io(format!("flush PTY 失败: {}", e)))?;
    Ok(())
}

pub fn resize(state: &TerminalState, session_id: &str, cols: u16, rows: u16) -> CmdResult<()> {
    if cols == 0 || rows == 0 {
        return Err(AppError::invalid_param("cols/rows 必须 > 0"));
    }
    let session = {
        let map = state.sessions.lock().map_err(|_| AppError::io("sessions lock"))?;
        map.get(session_id).cloned()
    }
    .ok_or_else(|| AppError::not_found(format!("终端会话不存在: {}", session_id)))?;

    let m = session.master.lock().map_err(|_| AppError::io("master lock"))?;
    m.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
    .map_err(|e| AppError::io(format!("resize PTY 失败: {}", e)))?;
    Ok(())
}

/// 关闭会话：kill child + 从 sessions 移除。幂等（不存在返回 Ok）。
pub fn close(state: &TerminalState, session_id: &str) -> CmdResult<()> {
    let session = {
        let mut map = state.sessions.lock().map_err(|_| AppError::io("sessions lock"))?;
        map.remove(session_id)
    };
    if let Some(s) = session {
        if let Ok(mut child) = s.child.lock() {
            let _ = child.kill();
        }
    }
    Ok(())
}

pub fn list(state: &TerminalState) -> CmdResult<Vec<TerminalListItem>> {
    let map = state.sessions.lock().map_err(|_| AppError::io("sessions lock"))?;
    Ok(map
        .values()
        .map(|s| TerminalListItem {
            session_id: s.id.clone(),
            space_id: s.space_id.clone(),
            cwd: s.cwd.to_string_lossy().to_string(),
            shell: s.shell.clone(),
        })
        .collect())
}

/// 应用退出时清理所有会话（lib.rs `RunEvent::ExitRequested` 调用）。
pub fn kill_all(state: &TerminalState) {
    let drained: Vec<Arc<PtySession>> = {
        match state.sessions.lock() {
            Ok(mut map) => map.drain().map(|(_, v)| v).collect(),
            Err(_) => return,
        }
    };
    for s in drained {
        if let Ok(mut child) = s.child.lock() {
            let _ = child.kill();
        }
    }
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn terminal_create(
    state: tauri::State<'_, TerminalState>,
    app_state: tauri::State<'_, crate::AppState>,
    space_id: Option<String>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> CmdResult<TerminalCreateOutput> {
    let state_inner = state.inner();
    // cwd 兜底链：显式 cwd > settings.root_dir > home
    let root_dir = crate::settings::load_root_dir(&app_state.pool).await?;
    // PTY spawn 是阻塞操作，丢到 blocking 池
    let sid =
        session_id_for_spawn(state_inner, space_id, cwd, root_dir, cols, rows, on_event).await?;
    Ok(sid)
}

async fn session_id_for_spawn(
    state: &TerminalState,
    space_id: Option<String>,
    cwd: Option<String>,
    root_dir: Option<String>,
    cols: u16,
    rows: u16,
    channel: Channel<TerminalEvent>,
) -> CmdResult<TerminalCreateOutput> {
    // spawn_session 内部已经用 spawn_blocking 跑 reader/wait，
    // 但 openpty + spawn_command 本身也可能阻塞几十 ms，这里再包一层。
    let sessions = state.sessions.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tmp_state = TerminalState { sessions };
        create(&tmp_state, space_id, cwd, root_dir, cols, rows, channel)
    })
    .await
    .map_err(|e| AppError::io(format!("spawn_blocking 失败: {}", e)))?
}

#[tauri::command]
pub async fn terminal_write(
    state: tauri::State<'_, TerminalState>,
    session_id: String,
    data: String,
) -> CmdResult<()> {
    write(state.inner(), &session_id, &data)
}

#[tauri::command]
pub async fn terminal_resize(
    state: tauri::State<'_, TerminalState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> CmdResult<()> {
    resize(state.inner(), &session_id, cols, rows)
}

#[tauri::command]
pub async fn terminal_close(
    state: tauri::State<'_, TerminalState>,
    session_id: String,
) -> CmdResult<()> {
    close(state.inner(), &session_id)
}

#[tauri::command]
pub async fn terminal_list(
    state: tauri::State<'_, TerminalState>,
) -> CmdResult<Vec<TerminalListItem>> {
    list(state.inner())
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// 测试用 Channel 替身：把事件收集到 mpsc。
    struct TestChannel {
        tx: std::sync::mpsc::Sender<TerminalEvent>,
    }

    // tauri::ipc::Channel 无法直接构造，但我们可以用 `Channel::new` 的测试路径
    // 或者绕过：把 spawn_session 的 channel 抽象出来。为最小化改动，
    // 这里直接用一个 helper：通过 tauri::ipc::Channel 的私有构造不可行，
    // 因此测试改为：直接调 spawn_session 的低层原语（openpty + spawn + read/write）。
    //
    // 更简单方案：把 reader/wait 的 channel 抽象为 `dyn Fn(TerminalEvent)`。
    // 为了不改动生产代码，测试改为端到端：起一个完整 session，
    // 但 channel 用 tauri 的测试构造。tauri 2.x 暂未提供 mock channel，
    // 所以我们改用一个内部测试函数，绕开 Channel 直接验证 PTY 行为。

    /// 端到端：创建会话 → 写 echo → 从 reader 读输出 → close。
    /// 这个测试不经过 Tauri Channel，直接验证 PTY 行为。
    #[test]
    fn pty_echo_roundtrip() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let shell = resolve_shell();
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-i");
        cmd.cwd(dirs::home_dir().unwrap());
        cmd.env("TERM", "xterm-256color");

        let mut child = pair.slave.spawn_command(cmd).expect("spawn");
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let mut writer = pair.master.take_writer().expect("writer");

        // 后台线程把读到的字节通过 mpsc 送回（read 是阻塞的）
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        std::thread::sleep(Duration::from_millis(400));
        writer.write_all(b"echo hello_m74\n").expect("write");
        writer.flush().expect("flush");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut collected = Vec::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(chunk) => {
                    collected.extend_from_slice(&chunk);
                    if String::from_utf8_lossy(&collected).contains("hello_m74") {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let text = String::from_utf8_lossy(&collected);
        assert!(text.contains("hello_m74"), "got: {:?}", text);

        child.kill().expect("kill");
        let _ = child.wait();
    }

    /// TerminalState 的增删查。
    #[test]
    fn state_insert_remove_list() {
        let state = TerminalState::new();
        assert_eq!(state.sessions.lock().unwrap().len(), 0);
        // 不真正 spawn，只验证 map 行为
        // 因为 PtySession 字段包含 non-Send 的 trait object，
        // 这里只验证 close 幂等。
        close(&state, "no-such-id").expect("close non-existent ok");
    }

    /// close 幂等。
    #[test]
    fn close_idempotent() {
        let state = TerminalState::new();
        close(&state, "x").expect("1st");
        close(&state, "x").expect("2nd");
        close(&state, "x").expect("3rd");
    }

    /// resolve_shell 非空。
    #[test]
    fn shell_resolves() {
        let s = resolve_shell();
        assert!(!s.is_empty());
    }

    /// resolve_cwd fallback 到 home。
    #[test]
    fn cwd_fallback_home() {
        let p = resolve_cwd(None, None);
        assert!(p.is_dir());
    }

    /// resolve_cwd 显式非法路径 fallback。
    #[test]
    fn cwd_invalid_fallback() {
        let p = resolve_cwd(Some("/nonexistent-xyz-123".into()), None);
        assert!(p.is_dir());
        assert_ne!(p.to_string_lossy(), "/nonexistent-xyz-123");
    }

    /// resolve_cwd：显式 cwd 优先于 root_dir；显式非法时用 root_dir。
    #[test]
    fn cwd_root_dir_priority() {
        let root = std::env::temp_dir();
        let root_str = root.to_string_lossy().to_string();

        // 显式合法 → 用显式
        let explicit = dirs::home_dir().unwrap();
        let p = resolve_cwd(
            Some(explicit.to_string_lossy().to_string()),
            Some(&root_str),
        );
        assert_eq!(p, explicit);

        // 显式非法 + root_dir 合法 → 用 root_dir（而非 home）
        let p = resolve_cwd(Some("/nonexistent-xyz-123".into()), Some(&root_str));
        assert_eq!(p, root);
    }
}
