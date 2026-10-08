//! m5-5.1 · 目录监听基础设施（详细设计 §5.2，契约冻结）
//!
//! 用 `notify` crate 接入平台原生事件：
//! - macOS  → FSEvents（默认）
//! - Windows → ReadDirectoryChangesW
//! - Linux   → inotify
//!
//! 平台事件统一标准化为 `Created / Modified / Renamed / Removed` 四种，
//! 通过 `tokio::sync::mpsc::Sender<WatchEvent>` 发给上层（5.2 忽略规则、5.3 聚合窗口）。
//!
//! 重命名事件按信息量分三路标准化（改名重复待处理修复）：
//! - `RenameMode::Both`（from/to 均知）→ `Renamed{ path: from, new_path: Some(to) }`，
//!   聚合层执行"改名跟随"（旧条目 path 直接改为新路径，不新建）；
//! - `RenameMode::From` 单侧（macOS FSEvents 常见拆分）→ `Removed(from)`，
//!   语义上文件确实从该路径消失，聚合层走 stale 兜底；
//! - `RenameMode::To` 单侧 → `Created(to)`，聚合层正常新建。
//! 由此无论平台给的是配对的 Both 还是分裂的 From+To（乃至编辑器的
//! remove+create 安全保存），改名后收件箱都只保留 1 条 pending。
//!
//! 单个 watcher 启动失败不影响其他；失败记录日志并把对应 `watch_dir.paused = 1`。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use notify::event::ModifyKind;
use notify::{Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::sqlite::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{AppError, CmdResult};

// ============================================================
// 标准化事件模型（§5.2 冻结）
// ============================================================

/// 标准化事件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Renamed,
    Removed,
}

/// 标准化事件载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub path: PathBuf,
    /// 仅 `Renamed` 且平台一次性给出 from+to（`RenameMode::Both`）时有值：
    /// 重命名的目标路径。其余情况恒为 `None`。
    pub new_path: Option<PathBuf>,
    /// 来自 `watch_dir` 表的 id。
    pub watch_dir_id: String,
    /// Unix 秒。
    pub at: i64,
}

// ============================================================
// 事件标准化（§5.2 映射 + 改名重复待处理修复）
// ============================================================
//
// notify 8.x 中重命名归类为 `EventKind::Modify(ModifyKind::Name(RenameMode))`：
// - `RenameMode::Both` → `Renamed`（携带 from + to，聚合层改名跟随）。
// - `RenameMode::From` → `Removed`（文件从该路径消失，聚合层 stale 兜底）。
// - `RenameMode::To`   → `Created`（文件出现在该路径，聚合层正常新建）。
// - `RenameMode::Any` / 其他 → `Renamed`（信息不足，new_path=None，
//   由聚合层 stat 裁决走跟随还是兜底）。
// - 其他 `ModifyKind`（Data / Metadata / Any / Other）→ `Modified`。
fn classify_event_kind(kind: &EventKind) -> Option<WatchEventKind> {
    match kind {
        EventKind::Create(_) => Some(WatchEventKind::Created),
        EventKind::Remove(_) => Some(WatchEventKind::Removed),
        EventKind::Modify(ModifyKind::Name(mode)) => match mode {
            notify::event::RenameMode::Both => Some(WatchEventKind::Renamed),
            notify::event::RenameMode::From => Some(WatchEventKind::Removed),
            notify::event::RenameMode::To => Some(WatchEventKind::Created),
            // Any / Other / 未来新增变体：保守归为 Renamed（new_path=None）
            _ => Some(WatchEventKind::Renamed),
        },
        EventKind::Modify(_) => Some(WatchEventKind::Modified),
        // Access / Any / Other 不上报
        _ => None,
    }
}

/// 把一条 notify 事件展开为 0..N 条标准化事件。
///
/// - 不能分类的 kind（Access / Any / Other）→ 空 vec。
/// - `RenameMode::Both` 且路径 ≥2 → 单条 `Renamed`，path=from、new_path=Some(to)
///   （notify 契约：rename from 在前）。
/// - 其他 `Renamed`（Any 等）→ 单条，取 from 路径（`paths[0]`），new_path=None。
/// - 其他事件对每个 path 各发一条。
fn normalize_event(ev: &NotifyEvent, watch_dir_id: &str, at: i64) -> Vec<WatchEvent> {
    let kind = match classify_event_kind(&ev.kind) {
        Some(k) => k,
        None => return Vec::new(),
    };

    let mk = |path: PathBuf, new_path: Option<PathBuf>| WatchEvent {
        kind,
        path,
        new_path,
        watch_dir_id: watch_dir_id.to_string(),
        at,
    };

    if kind == WatchEventKind::Renamed {
        // Both：from + to 同时可得 → 改名跟随所需的完整信息
        if let EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::Both)) = &ev.kind {
            if ev.paths.len() >= 2 {
                return vec![mk(ev.paths[0].clone(), Some(ev.paths[1].clone()))];
            }
        }
        // 其余 rename（Any 或路径不足）：只取 from 路径
        return match ev.paths.first() {
            Some(p) => vec![mk(p.clone(), None)],
            None => Vec::new(),
        };
    }

    ev.paths.iter().map(|p| mk(p.clone(), None)).collect()
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ============================================================
// WatcherHandle：管理一组活跃 watcher
// ============================================================

/// 单个 watcher 条目：保存底层 watcher 与路径（unwatch 用）。
struct WatcherEntry {
    watcher: RecommendedWatcher,
    path: PathBuf,
}

/// 监听器句柄。`Clone` 后可跨任务共享；内部用 `Arc<Mutex<..>>` 保护。
///
/// 注意：`RecommendedWatcher` drop 即停止监听，因此 `stop_watchers` 只需清空 map。
#[derive(Clone)]
pub struct WatcherHandle {
    inner: Arc<Mutex<HashMap<String, WatcherEntry>>>,
    tx: mpsc::Sender<WatchEvent>,
}

impl WatcherHandle {
    fn new(tx: mpsc::Sender<WatchEvent>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    /// 当前活跃 watcher 数量（测试用）。
    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }
}

// ============================================================
// 内部：为单个目录建 watcher
// ============================================================

fn build_watcher(
    watch_dir_id: &str,
    path: &std::path::Path,
    recursive: bool,
    tx: mpsc::Sender<WatchEvent>,
) -> Result<WatcherEntry, AppError> {
    let id_for_cb = watch_dir_id.to_string();
    let tx_for_cb = tx.clone();

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: Result<NotifyEvent, notify::Error>| {
            match res {
                Ok(ev) => {
                    let at = now_unix_secs();
                    for we in normalize_event(&ev, &id_for_cb, at) {
                        // 用 blocking_send 不行（回调在 notify 内部线程，不在 tokio runtime），
                        // 用 try_send 避免阻塞；mpsc 满则丢弃并记日志。
                        if let Err(e) = tx_for_cb.try_send(we) {
                            eprintln!("[watch] 事件通道已满或已关闭，丢弃事件: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[watch] notify 错误 (watch_dir={}): {}", id_for_cb, e);
                }
            }
        })
        .map_err(|e| AppError::io(format!("创建 watcher 失败 ({}): {}", path.display(), e)))?;

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher
        .watch(path, mode)
        .map_err(|e| AppError::io(format!("watch 失败 ({}): {}", path.display(), e)))?;

    Ok(WatcherEntry {
        watcher,
        path: path.to_path_buf(),
    })
}

// ============================================================
// 内部：把 watch_dir 标记为 paused（故障处理 §5.2）
// ============================================================

async fn mark_paused(pool: &SqlitePool, watch_dir_id: &str) {
    if let Err(e) = sqlx::query("UPDATE watch_dir SET paused = 1 WHERE id = ?")
        .bind(watch_dir_id)
        .execute(pool)
        .await
    {
        eprintln!(
            "[watch] 标记 watch_dir.paused=1 失败 (id={}): {}",
            watch_dir_id, e
        );
    }
}

// ============================================================
// 公开 API
// ============================================================

/// 启动监听器：读 `watch_dir` 表，为每个 `paused = 0` 的目录建 watcher。
///
/// 单个 watcher 启动失败不影响其他；失败会记录日志并把对应行 `paused = 1`。
pub async fn start_watchers(
    pool: SqlitePool,
    tx: mpsc::Sender<WatchEvent>,
) -> CmdResult<WatcherHandle> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT id, path, recursive FROM watch_dir WHERE paused = 0")
            .fetch_all(&pool)
            .await
            .map_err(AppError::from)?;

    let handle = WatcherHandle::new(tx);

    for (id, path_str, recursive_i) in rows {
        let path = PathBuf::from(&path_str);
        let recursive = recursive_i != 0;

        let tx_clone = handle.tx.clone();
        match build_watcher(&id, &path, recursive, tx_clone) {
            Ok(entry) => {
                let mut map = handle
                    .inner
                    .lock()
                    .map_err(|_| AppError::new("COMMON_INTERNAL", "watcher map 锁中毒"))?;
                map.insert(id.clone(), entry);
            }
            Err(e) => {
                eprintln!(
                    "[watch] 启动 watcher 失败 (id={}, path={}): {}",
                    id, path_str, e
                );
                mark_paused(&pool, &id).await;
            }
        }
    }

    Ok(handle)
}

/// 停止所有监听器（应用退出时调用）。
#[allow(dead_code)] // 5.4 / 应用退出钩子接入
pub async fn stop_watchers(handle: WatcherHandle) -> CmdResult<()> {
    let mut map = handle
        .inner
        .lock()
        .map_err(|_| AppError::new("COMMON_INTERNAL", "watcher map 锁中毒"))?;
    // 显式 unwatch + drop，确保底层平台资源释放。
    for (_, mut entry) in map.drain() {
        let _ = entry.watcher.unwatch(&entry.path);
        drop(entry.watcher);
    }
    Ok(())
}

/// 增量订阅：`watch_dir_set` 保存配置后即时调用（免重启）。
///
/// 若同一 `watch_dir_id` 已存在，先 unwatch 再重建（幂等）。
pub async fn add_watch(
    handle: &WatcherHandle,
    watch_dir_id: String,
    path: PathBuf,
    recursive: bool,
) -> CmdResult<()> {
    let entry = build_watcher(&watch_dir_id, &path, recursive, handle.tx.clone())?;
    let mut map = handle
        .inner
        .lock()
        .map_err(|_| AppError::new("COMMON_INTERNAL", "watcher map 锁中毒"))?;
    map.insert(watch_dir_id, entry);
    Ok(())
}

/// 增量退订：`watch_dir_unset` 删除配置后即时调用（免重启）。
///
/// 不存在时静默成功（幂等）。
pub async fn remove_watch(handle: &WatcherHandle, watch_dir_id: &str) -> CmdResult<()> {
    let mut map = handle
        .inner
        .lock()
        .map_err(|_| AppError::new("COMMON_INTERNAL", "watcher map 锁中毒"))?;
    if let Some(mut entry) = map.remove(watch_dir_id) {
        // 显式 unwatch，再让 watcher drop
        let _ = entry.watcher.unwatch(&entry.path);
    }
    Ok(())
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::timeout;

    // ---------- 事件标准化映射 ----------

    fn make_event(kind: EventKind, paths: Vec<PathBuf>) -> NotifyEvent {
        let mut ev = NotifyEvent::new(kind);
        for p in paths {
            ev = ev.add_path(p);
        }
        ev
    }

    #[test]
    fn normalize_create_maps_to_created() {
        let ev = make_event(
            EventKind::Create(CreateKind::File),
            vec![PathBuf::from("/tmp/a.txt")],
        );
        let out = normalize_event(&ev, "wd1", 123);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Created);
        assert_eq!(out[0].path, PathBuf::from("/tmp/a.txt"));
        assert_eq!(out[0].watch_dir_id, "wd1");
        assert_eq!(out[0].at, 123);
    }

    #[test]
    fn normalize_modify_data_maps_to_modified() {
        let ev = make_event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            vec![PathBuf::from("/tmp/b.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Modified);
    }

    #[test]
    fn normalize_remove_maps_to_removed() {
        let ev = make_event(
            EventKind::Remove(RemoveKind::File),
            vec![PathBuf::from("/tmp/c.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Removed);
    }

    #[test]
    fn normalize_rename_both_carries_from_and_to() {
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            vec![PathBuf::from("/tmp/old.txt"), PathBuf::from("/tmp/new.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1, "rename Both 只发一条");
        assert_eq!(out[0].kind, WatchEventKind::Renamed);
        assert_eq!(out[0].path, PathBuf::from("/tmp/old.txt"), "path = from");
        assert_eq!(
            out[0].new_path,
            Some(PathBuf::from("/tmp/new.txt")),
            "new_path = to（聚合层改名跟随用）"
        );
    }

    #[test]
    fn normalize_rename_from_maps_to_removed() {
        // 单侧 From（macOS FSEvents 常见拆分）：文件从该路径消失 → Removed
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            vec![PathBuf::from("/tmp/x.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Removed);
        assert_eq!(out[0].path, PathBuf::from("/tmp/x.txt"));
        assert_eq!(out[0].new_path, None);
    }

    #[test]
    fn normalize_rename_to_maps_to_created() {
        // 单侧 To：文件出现在该路径 → Created（聚合层正常新建）
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            vec![PathBuf::from("/tmp/y.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Created);
        assert_eq!(out[0].path, PathBuf::from("/tmp/y.txt"));
        assert_eq!(out[0].new_path, None);
    }

    #[test]
    fn normalize_rename_any_is_renamed_without_new_path() {
        // Any：信息不足，保持 Renamed + new_path=None，由聚合层 stat 裁决
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            vec![PathBuf::from("/tmp/z.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Renamed);
        assert_eq!(out[0].new_path, None);
    }

    #[test]
    fn normalize_access_is_dropped() {
        let ev = make_event(
            EventKind::Access(notify::event::AccessKind::Read),
            vec![PathBuf::from("/tmp/a.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert!(out.is_empty(), "Access 事件不上报");
    }

    #[test]
    fn normalize_other_is_dropped() {
        let ev = make_event(EventKind::Other, vec![PathBuf::from("/tmp/a.txt")]);
        let out = normalize_event(&ev, "wd1", 1);
        assert!(out.is_empty(), "Other 事件不上报");
    }

    #[test]
    fn normalize_multi_path_modify_fans_out() {
        let ev = make_event(
            EventKind::Modify(ModifyKind::Any),
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| e.kind == WatchEventKind::Modified));
    }

    // ---------- 集成：tempdir 上启动 watcher 收发事件 ----------
    //
    // 平台相关：macOS FSEvents 在 CI 无 GUI 会话时可能不可靠；
    // 本机（开发机 macOS）跑通即可。Linux CI 通常 inotify 可用。
    //
    // 用一个较大的超时（5s）容忍 FSEvents 的合并/延迟。

    /// 路径等价比较：容忍 macOS `/var` ↔ `/private/var` symlink 差异。
    /// 通过比较 canonicalize 后的父目录 + 文件名实现；文件已删除时退化为 raw 比较。
    fn path_eq(a: &std::path::Path, b: &std::path::Path) -> bool {
        if a == b {
            return true;
        }
        let canon = |p: &std::path::Path| -> Option<std::path::PathBuf> {
            let parent = p.parent()?.canonicalize().ok()?;
            let name = p.file_name()?;
            Some(parent.join(name))
        };
        match (canon(a), canon(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    async fn recv_kind_within(
        rx: &mut mpsc::Receiver<WatchEvent>,
        want: WatchEventKind,
        want_path: &std::path::Path,
        dur: Duration,
    ) -> Option<WatchEvent> {
        let deadline = std::time::Instant::now() + dur;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                return None;
            }
            let remain = deadline - now;
            match timeout(remain, rx.recv()).await {
                Ok(Some(ev)) => {
                    if ev.kind == want && path_eq(&ev.path, want_path) {
                        return Some(ev);
                    }
                    // 继续等（FSEvents 可能先发其他事件）
                }
                Ok(None) => return None, // 通道关闭
                Err(_) => return None,   // 超时
            }
        }
    }

    // 调试辅助：打印一段时间内收到的所有事件（不 assert）。
    #[tokio::test]
    #[ignore = "手动调试：cargo test watch::tests::debug_dump_events -- --nocapture --ignored"]
    async fn debug_dump_events() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let file = root.join("hello.txt");

        let (tx, mut rx) = mpsc::channel::<WatchEvent>(256);
        let handle = WatcherHandle::new(tx);
        add_watch(&handle, "wd_dbg".to_string(), root.clone(), true)
            .await
            .expect("add_watch");
        tokio::time::sleep(Duration::from_millis(300)).await;

        fs::write(&file, b"hello").expect("write");
        tokio::time::sleep(Duration::from_millis(500)).await;
        fs::write(&file, b"world").expect("rewrite");
        tokio::time::sleep(Duration::from_millis(500)).await;
        // 改名：观察平台投递 Both 还是 From+To 分裂
        let renamed = root.join("renamed.txt");
        fs::rename(&file, &renamed).expect("rename");
        tokio::time::sleep(Duration::from_millis(500)).await;
        fs::remove_file(&renamed).expect("remove");

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            match timeout(Duration::from_millis(300), rx.recv()).await {
                Ok(Some(ev)) => eprintln!("[dbg] {:?} {}", ev.kind, ev.path.display()),
                Ok(None) => break,
                Err(_) => {}
            }
        }
    }

    #[tokio::test]
    #[cfg_attr(
        not(target_os = "macos"),
        ignore = "仅在 macOS 本机验证；CI 上 inotify 行为类似但时序不同"
    )]
    async fn watcher_emits_create_modify_remove_on_tempdir() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let file = root.join("hello.txt");

        let (tx, mut rx) = mpsc::channel::<WatchEvent>(64);
        let handle = WatcherHandle::new(tx);
        add_watch(&handle, "wd_test".to_string(), root.clone(), true)
            .await
            .expect("add_watch");
        assert_eq!(handle.len(), 1);

        // 给 FSEvents 一点启动时间
        tokio::time::sleep(Duration::from_millis(300)).await;

        // 1) 创建
        fs::write(&file, b"hello").expect("write");
        let ev = recv_kind_within(
            &mut rx,
            WatchEventKind::Created,
            &file,
            Duration::from_secs(5),
        )
        .await
        .expect("应收到 Created 事件");
        assert_eq!(ev.watch_dir_id, "wd_test");
        // 2) 修改
        fs::write(&file, b"hello world, modified").expect("rewrite");
        let ev = recv_kind_within(
            &mut rx,
            WatchEventKind::Modified,
            &file,
            Duration::from_secs(5),
        )
        .await
        .expect("应收到 Modified 事件");
        assert_eq!(ev.watch_dir_id, "wd_test");

        // 3) 删除
        fs::remove_file(&file).expect("remove");
        let ev = recv_kind_within(
            &mut rx,
            WatchEventKind::Removed,
            &file,
            Duration::from_secs(5),
        )
        .await
        .expect("应收到 Removed 事件");
        assert_eq!(ev.watch_dir_id, "wd_test");

        // 4) 停止后不再收
        stop_watchers(handle).await.expect("stop");
        // 再写一个新文件
        let file2 = root.join("after_stop.txt");
        fs::write(&file2, b"x").expect("write2");
        // 收一段时间，应当没有新事件（通道可能仍有残留，过滤掉 file2 路径）
        let res = timeout(Duration::from_millis(800), async {
            loop {
                match rx.recv().await {
                    Some(ev) => {
                        if path_eq(&ev.path, &file2) {
                            return Some(ev);
                        }
                        // 残留事件，丢弃继续
                    }
                    None => return None,
                }
            }
        })
        .await;
        match res {
            Ok(Some(_)) => panic!("停止后不应再收到新文件事件"),
            _ => {} // Ok(None) 通道关闭 / Err 超时 都视为通过
        }
    }

    #[tokio::test]
    #[cfg_attr(not(target_os = "macos"), ignore = "仅在 macOS 本机验证")]
    async fn remove_watch_stops_events_for_that_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();

        let (tx, mut rx) = mpsc::channel::<WatchEvent>(32);
        let handle = WatcherHandle::new(tx);
        add_watch(&handle, "wd_rm".to_string(), root.clone(), true)
            .await
            .expect("add_watch");
        assert_eq!(handle.len(), 1);

        tokio::time::sleep(Duration::from_millis(200)).await;

        remove_watch(&handle, "wd_rm").await.expect("remove_watch");
        assert_eq!(handle.len(), 0);

        // 触发一个事件
        let file = root.join("should_not_see.txt");
        fs::write(&file, b"x").expect("write");

        // 短暂等待，不应收到该路径事件
        let res = timeout(Duration::from_millis(600), async {
            loop {
                match rx.recv().await {
                    Some(ev) => {
                        if path_eq(&ev.path, &file) {
                            return Some(ev);
                        }
                    }
                    None => return None,
                }
            }
        })
        .await;
        match res {
            Ok(Some(_)) => panic!("remove_watch 后不应再收到事件"),
            _ => {}
        }
    }

    // ---------- 端到端诊断：真实 FSEvents → 过滤 → 聚合 → inbox_item ----------
    //
    // 诊断目标：「有时能收到通知有时就没有」。
    // 用真实文件操作驱动完整管道（watcher → filter_events → aggregate_batch），
    // 断言「操作结束后监控目录里存在的文件，必须有 pending 收件箱条目」。

    /// 建内存 DB：aggregate_batch 需要的最小表（resource_reference / inbox_item）。
    async fn setup_e2e_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.expect("pool");
        sqlx::query(
            "CREATE TABLE resource_reference (
                id TEXT PRIMARY KEY,
                collection_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                name TEXT NOT NULL,
                type TEXT NOT NULL,
                hosting TEXT NOT NULL,
                locator_json TEXT NOT NULL,
                description TEXT,
                lifecycle TEXT NOT NULL DEFAULT 'active',
                confidentiality TEXT NOT NULL DEFAULT 'internal',
                indexed INTEGER NOT NULL DEFAULT 1,
                disposition TEXT NOT NULL DEFAULT 'none',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create resource_reference");
        sqlx::query(
            "CREATE TABLE inbox_item (
                id TEXT PRIMARY KEY,
                watch_dir_id TEXT,
                path TEXT NOT NULL,
                event_kind TEXT CHECK (event_kind IN ('created','modified','renamed','removed')),
                size_bytes INTEGER,
                mtime INTEGER,
                ext TEXT,
                suggested_type TEXT,
                status TEXT CHECK (status IN ('pending','snoozed','processed','ignored','stale')),
                assign_json TEXT,
                ignore_rule_id TEXT,
                snooze_note TEXT,
                remind_at INTEGER,
                discovered_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create inbox_item");
        pool
    }

    /// 收集事件直到安静 `quiet` 或总时长超过 `cap`。
    async fn drain_events(rx: &mut mpsc::Receiver<WatchEvent>, quiet: Duration, cap: Duration) -> Vec<WatchEvent> {
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + cap;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline {
                break;
            }
            let step = quiet.min(deadline - now);
            match timeout(step, rx.recv()).await {
                Ok(Some(ev)) => out.push(ev),
                Ok(None) => break,   // 通道关闭
                Err(_) => break,     // 安静期到
            }
        }
        out
    }

    /// 查询某路径的 pending 条目数。
    async fn pending_count_at(pool: &SqlitePool, path: &std::path::Path) -> i64 {
        let (cnt,): (i64,) = sqlx::query_as(
            "SELECT COUNT(1) FROM inbox_item WHERE path = ? AND status = 'pending'",
        )
        .bind(path.to_string_lossy().to_string())
        .fetch_one(pool)
        .await
        .expect("count");
        cnt
    }

    /// 打印一批事件（诊断观察用）。
    fn dump_events(label: &str, events: &[WatchEvent]) {
        eprintln!("[e2e] --- {} ({} events) ---", label, events.len());
        for ev in events {
            eprintln!(
                "[e2e]   {:?} path={} new_path={:?}",
                ev.kind,
                ev.path.display(),
                ev.new_path.as_ref().map(|p| p.display().to_string())
            );
        }
    }

    #[tokio::test]
    #[cfg_attr(not(target_os = "macos"), ignore = "FSEvents 行为仅 macOS 本机可验")]
    async fn e2e_real_world_ops_reach_inbox() {
        use crate::aggregate::{aggregate_batch, EventBatch};

        let pool = setup_e2e_pool().await;
        let watched = TempDir::new().expect("watched tempdir");
        let outside = TempDir::new().expect("outside tempdir");
        // FSEvents 上报的是解析 symlink 后的路径（macOS /var → /private/var），
        // watch 与断言统一用 canonicalize 后的路径。
        let root = watched.path().canonicalize().expect("canonical root");
        let outside_root = outside.path().canonicalize().expect("canonical outside");

        // 管道：watcher → filter_events（空规则）→ 收集
        let (tx, rx) = mpsc::channel::<WatchEvent>(256);
        let handle = WatcherHandle::new(tx);
        add_watch(&handle, "wd_e2e".to_string(), root.clone(), true)
            .await
            .expect("add_watch");
        let (ftx, mut frx) = mpsc::channel::<WatchEvent>(256);
        let rules = std::sync::Arc::new(std::sync::RwLock::new(Vec::new()));
        let _filter = crate::ignore::filter_events(rx, ftx, rules);

        // 等 FSEvents 流就绪
        tokio::time::sleep(Duration::from_millis(500)).await;

        let quiet = Duration::from_millis(800);
        let cap = Duration::from_secs(8);
        // 记录每个操作的最终落账结果，最后一并断言（先观察全量行为）
        let mut misses: Vec<String> = Vec::new();

        // ===== 操作 A：直接新建（基线，契约内行为） =====
        let file_a = root.join("a_direct.txt");
        fs::write(&file_a, b"hello").expect("write A");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("A 直接新建", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg A");
        eprintln!("[e2e] A 聚合结果: {:?}", r);
        if pending_count_at(&pool, &file_a).await != 1 { misses.push("A 直接新建".into()); }

        // ===== 操作 B：从监控目录外移入（Finder 拖入 / mv 等价物） =====
        let outside_file = outside_root.join("b_moved.txt");
        fs::write(&outside_file, b"moved in").expect("write outside");
        let file_b = root.join("b_moved.txt");
        fs::rename(&outside_file, &file_b).expect("rename into watched");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("B 外部移入", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg B");
        eprintln!("[e2e] B 聚合结果: {:?}", r);
        if pending_count_at(&pool, &file_b).await != 1 { misses.push("B 外部移入".into()); }

        // ===== 操作 C：原子保存（写临时文件 + rename 覆盖目标，编辑器/App 保存语义） =====
        let file_c = root.join("c_atomic.txt");
        fs::write(&file_c, b"v1").expect("write C v1");
        let evs = drain_events(&mut frx, quiet, cap).await;
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg C1");
        eprintln!("[e2e] C1 聚合结果: {:?}", r);
        if pending_count_at(&pool, &file_c).await != 1 { misses.push("C 初始创建".into()); }

        let tmp_c = root.join(".c_atomic_tmp");
        fs::write(&tmp_c, b"v2").expect("write C tmp");
        fs::rename(&tmp_c, &file_c).expect("atomic rename over");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("C 原子保存", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg C2");
        eprintln!("[e2e] C2 聚合结果: {:?}", r);
        if pending_count_at(&pool, &file_c).await != 1 { misses.push("C 原子保存".into()); }

        // ===== 操作 D：rename 替换一个从未进过收件箱的目标（移动覆盖） =====
        let file_d = root.join("d_replaced.txt");
        fs::write(&file_d, b"old").expect("write D old");
        // 旧文件先落账（模拟它已被收件箱跟踪）
        let evs = drain_events(&mut frx, quiet, cap).await;
        let _ = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg D1");
        // 从外部移入同名文件覆盖（ Finder 拖入同名文件选择「替换」的语义）
        let outside_d = outside_root.join("d_new.txt");
        fs::write(&outside_d, b"new").expect("write D new");
        fs::rename(&outside_d, &file_d).expect("rename replace D");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("D 移入覆盖", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg D2");
        eprintln!("[e2e] D 聚合结果: {:?}", r);
        if pending_count_at(&pool, &file_d).await != 1 { misses.push("D 移入覆盖".into()); }

        stop_watchers(handle).await.expect("stop");
        eprintln!("[e2e] ===== 丢条目汇总: {:?} =====", misses);
        assert!(misses.is_empty(), "以下操作丢条目: {:?}", misses);
    }

    /// 回归：Chrome 下载（先建临时文件，完成后改名正式名）。
    ///
    /// 历史 bug：慢速下载时 `.crdownload` 先入收件箱，改名后其 pending 条目
    /// 残留成幽灵（消失侧事件与 Created/Modified 同秒到达被去重丢弃）；
    /// 且临时文件本体不该出现在收件箱/通知里。
    /// macOS 上 Chromium 系还有第二种临时文件形态：`.com.google.Chrome.<随机串>`
    /// 隐藏文件（无 .crdownload 扩展名），同样覆盖。
    ///
    /// 断言：三种下载形态下，正式文件恰好 1 条 pending，临时文件 0 条。
    #[tokio::test]
    #[cfg_attr(not(target_os = "macos"), ignore = "FSEvents 行为仅 macOS 本机可验")]
    async fn e2e_chrome_download_reaches_inbox() {
        use crate::aggregate::{aggregate_batch, EventBatch};

        let pool = setup_e2e_pool().await;
        let watched = TempDir::new().expect("watched tempdir");
        let root = watched.path().canonicalize().expect("canonical root");

        let (tx, rx) = mpsc::channel::<WatchEvent>(256);
        let handle = WatcherHandle::new(tx);
        add_watch(&handle, "wd_chrome".to_string(), root.clone(), true)
            .await
            .expect("add_watch");
        let (ftx, mut frx) = mpsc::channel::<WatchEvent>(256);
        // 关键：用默认忽略规则（和生产一致，含 .crdownload 过滤）
        let rules = std::sync::Arc::new(std::sync::RwLock::new(crate::ignore::default_rules()));
        let _filter = crate::ignore::filter_events(rx, ftx, rules);

        tokio::time::sleep(Duration::from_millis(500)).await;

        let quiet = Duration::from_millis(800);
        let cap = Duration::from_secs(8);

        // ===== 场景 1：小文件秒下（create + rename 落在同一聚合窗口同一秒） =====
        let fast_tmp = root.join("small.zip.crdownload");
        let fast_final = root.join("small.zip");
        fs::write(&fast_tmp, b"chunk1").expect("write crdownload");
        fs::rename(&fast_tmp, &fast_final).expect("rename to final");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("Chrome 小文件秒下", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg fast");
        eprintln!("[chrome] 小文件聚合结果: {:?}", r);
        assert_eq!(
            pending_count_at(&pool, &fast_final).await,
            1,
            "小文件秒下：正式文件应进收件箱"
        );

        // ===== 场景 2：大文件慢速下载（跨多个 5s 窗口） =====
        let slow_tmp = root.join("big.zip.crdownload");
        let slow_final = root.join("big.zip");
        fs::write(&slow_tmp, b"chunk1").expect("write chunk1");
        // 窗口 1：下载中
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("Chrome 大文件·窗口1(下载中)", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg slow w1");
        eprintln!("[chrome] 大文件窗口1 聚合: {:?}", r);
        // 窗口 2：下载完成，改名
        fs::rename(&slow_tmp, &slow_final).expect("rename big");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("Chrome 大文件·窗口2(完成改名)", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg slow w2");
        eprintln!("[chrome] 大文件窗口2 聚合: {:?}", r);
        assert_eq!(
            pending_count_at(&pool, &slow_final).await,
            1,
            "大文件慢下：正式文件应进收件箱"
        );

        // ===== 场景 3：macOS Chromium 隐藏临时文件（.com.google.Chrome.<随机串>） =====
        // 用户实测形态：/Users/x/Downloads/.com.google.Chrome.qXESF8
        let hidden_tmp = root.join(".com.google.Chrome.qXESF8");
        let hidden_final = root.join("photo.png");
        fs::write(&hidden_tmp, b"png-bytes").expect("write hidden tmp");
        // 窗口 1：下载中（临时文件应被忽略规则拦截，不进收件箱）
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("Chrome 隐藏临时文件·窗口1(下载中)", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg hidden w1");
        eprintln!("[chrome] 隐藏临时文件窗口1 聚合: {:?}", r);
        // 窗口 2：下载完成，改名正式名
        fs::rename(&hidden_tmp, &hidden_final).expect("rename hidden");
        let evs = drain_events(&mut frx, quiet, cap).await;
        dump_events("Chrome 隐藏临时文件·窗口2(完成改名)", &evs);
        let r = aggregate_batch(&pool, EventBatch { window_start: 0, window_end: 0, events: evs })
            .await
            .expect("agg hidden w2");
        eprintln!("[chrome] 隐藏临时文件窗口2 聚合: {:?}", r);
        assert_eq!(
            pending_count_at(&pool, &hidden_final).await,
            1,
            "隐藏临时文件改名后：正式文件应进收件箱"
        );

        // .crdownload / Chromium 隐藏临时文件：任何状态都不应残留收件箱
        let (tmp_cnt,): (i64,) = sqlx::query_as(
            "SELECT COUNT(1) FROM inbox_item \
             WHERE path LIKE '%.crdownload' OR path LIKE '%/.com.google.Chrome.%'",
        )
        .fetch_one(&pool)
        .await
        .expect("count tmp");
        assert_eq!(tmp_cnt, 0, "浏览器下载临时文件不应出现在收件箱");

        stop_watchers(handle).await.expect("stop");
    }

    // ---------- start_watchers 故障隔离（单 watcher 失败不影响其他） ----------

    #[tokio::test]
    async fn start_watchers_marks_failed_dir_paused() {
        let pool = SqlitePool::connect(":memory:").await.expect("pool");
        sqlx::query(
            "CREATE TABLE watch_dir (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                recursive INTEGER NOT NULL DEFAULT 1,
                paused INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");

        // 一行指向不存在路径（应失败 → paused=1）
        sqlx::query("INSERT INTO watch_dir (id, path, recursive, paused) VALUES (?, ?, ?, 0)")
            .bind("bad")
            .bind("/nonexistent/path/that/should/not/exist/anywhere")
            .bind(1i64)
            .execute(&pool)
            .await
            .expect("insert bad");

        let (tx, _rx) = mpsc::channel::<WatchEvent>(8);
        let handle = start_watchers(pool.clone(), tx)
            .await
            .expect("start_watchers");
        // 失败的 watcher 不应进入 map
        assert_eq!(handle.len(), 0);

        // 对应行应被标记 paused=1
        let (paused,): (i64,) = sqlx::query_as("SELECT paused FROM watch_dir WHERE id = 'bad'")
            .fetch_one(&pool)
            .await
            .expect("select");
        assert_eq!(paused, 1, "失败 watcher 应被标记 paused=1");
    }
}
