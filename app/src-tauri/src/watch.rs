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
//! 单个 watcher 启动失败不影响其他；失败记录日志并把对应 `watch_dir.paused = 1`。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use notify::{
    Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use notify::event::ModifyKind;
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
    /// 来自 `watch_dir` 表的 id。
    pub watch_dir_id: String,
    /// Unix 秒。
    pub at: i64,
}

// ============================================================
// 事件标准化（§5.2 冻结映射）
// ============================================================
//
// notify 8.x 中重命名归类为 `EventKind::Modify(ModifyKind::Name(RenameMode))`：
// - `RenameMode::From` / `RenameMode::To` / `RenameMode::Both` / `RenameMode::Any`
//   均映射为 `Renamed`。
// - 其他 `ModifyKind`（Data / Metadata / Any / Other）→ `Modified`。
fn classify_event_kind(kind: &EventKind) -> Option<WatchEventKind> {
    match kind {
        EventKind::Create(_) => Some(WatchEventKind::Created),
        EventKind::Remove(_) => Some(WatchEventKind::Removed),
        EventKind::Modify(ModifyKind::Name(_)) => Some(WatchEventKind::Renamed),
        EventKind::Modify(_) => Some(WatchEventKind::Modified),
        // Access / Any / Other 不上报
        _ => None,
    }
}

/// 把一条 notify 事件展开为 0..N 条标准化事件。
///
/// - 不能分类的 kind（Access / Any / Other）→ 空 vec。
/// - 重命名事件取 from 路径（`paths[0]`，notify 契约：rename from 在前）。
/// - 其他事件对每个 path 各发一条。
fn normalize_event(
    ev: &NotifyEvent,
    watch_dir_id: &str,
    at: i64,
) -> Vec<WatchEvent> {
    let kind = match classify_event_kind(&ev.kind) {
        Some(k) => k,
        None => return Vec::new(),
    };

    let paths: &[PathBuf] = if kind == WatchEventKind::Renamed {
        // 重命名只取 from 路径（§5.2 冻结：from 路径）
        ev.paths.first().map(std::slice::from_ref).unwrap_or(&[])
    } else {
        ev.paths.as_slice()
    };

    paths
        .iter()
        .map(|p| WatchEvent {
            kind,
            path: p.clone(),
            watch_dir_id: watch_dir_id.to_string(),
            at,
        })
        .collect()
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

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<NotifyEvent, notify::Error>| {
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
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT id, path, recursive FROM watch_dir WHERE paused = 0",
    )
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

/// 增量订阅：5.4 `settings_add_watch_dir` 时调用。
///
/// 若同一 `watch_dir_id` 已存在，先 unwatch 再重建（幂等）。
#[allow(dead_code)] // 5.4 接入
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

/// 增量退订：5.4 `settings_remove_watch_dir` 时调用。
///
/// 不存在时静默成功（幂等）。
#[allow(dead_code)] // 5.4 接入
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
    fn normalize_rename_uses_from_path_only() {
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            vec![
                PathBuf::from("/tmp/old.txt"),
                PathBuf::from("/tmp/new.txt"),
            ],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1, "rename 只发一条 (from 路径)");
        assert_eq!(out[0].kind, WatchEventKind::Renamed);
        assert_eq!(out[0].path, PathBuf::from("/tmp/old.txt"));
    }

    #[test]
    fn normalize_rename_from_variant() {
        let ev = make_event(
            EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            vec![PathBuf::from("/tmp/x.txt")],
        );
        let out = normalize_event(&ev, "wd1", 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WatchEventKind::Renamed);
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
        fs::remove_file(&file).expect("remove");

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
    #[cfg_attr(not(target_os = "macos"), ignore = "仅在 macOS 本机验证；CI 上 inotify 行为类似但时序不同")]
    async fn watcher_emits_create_modify_remove_on_tempdir() {        let tmp = TempDir::new().expect("tempdir");
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
        let ev = recv_kind_within(&mut rx, WatchEventKind::Created, &file, Duration::from_secs(5))
            .await
            .expect("应收到 Created 事件");
        assert_eq!(ev.watch_dir_id, "wd_test");
        // 2) 修改
        fs::write(&file, b"hello world, modified").expect("rewrite");
        let ev = recv_kind_within(&mut rx, WatchEventKind::Modified, &file, Duration::from_secs(5))
            .await
            .expect("应收到 Modified 事件");
        assert_eq!(ev.watch_dir_id, "wd_test");

        // 3) 删除
        fs::remove_file(&file).expect("remove");
        let ev = recv_kind_within(&mut rx, WatchEventKind::Removed, &file, Duration::from_secs(5))
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

    // ---------- start_watchers 故障隔离（单 watcher 失败不影响其他） ----------

    #[tokio::test]
    async fn start_watchers_marks_failed_dir_paused() {
        // 用 in-memory sqlite 建最小 watch_dir 表
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
        let handle = start_watchers(pool.clone(), tx).await.expect("start_watchers");
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
