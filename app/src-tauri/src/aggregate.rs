//! m5-5.3 · 聚合窗口缓冲 + 去重（详细设计 §5.2「5 秒聚合窗口 + 同路径去重」）
//!
//! 消费 5.2 过滤后的 `WatchEvent` 流，按 5 秒窗口缓冲合并：
//! - 同窗口同路径多事件去重（取 `at` 最新一条），窗口内按 `at` 排序处理
//! - 路径已被正式引用（`resource_reference.locator_json` 的 `$.path` 匹配）→ 丢弃
//! - `Renamed` 且携带 `new_path`（RenameMode::Both）→ 改名跟随：
//!   from 有 pending/snoozed 条目则直接把 path 改为新路径，不新建
//! - `Removed` / `Renamed`（无 new_path）：pending/snoozed 条目且磁盘上
//!   路径已不存在 → 标记 `stale`（改名重复待处理修复的跨平台兜底）；
//!   无已有条目且路径已不存在 → 丢弃（不为已消失的文件新建待处理）；
//!   无已有条目但路径实际存在 → 按「出现」处理（FSEvents 把移入监控目录
//!   报成无目标路径的 Renamed/Any，路径是目标路径，以磁盘状态裁决）
//! - `Created` / `Modified`：同路径已有未处理 `inbox_item`（`status='pending'`）
//!   → 以磁盘状态裁决：文件仍在则 UPDATE mtime/event_kind，已消失则标记 stale
//!   （消失侧事件在同秒去重中丢失时避免幽灵 pending 残留）；无已有条目则新建前
//!   stat：路径已不存在（窗口内消失的临时文件）→ 丢弃，存在 → 新建 `inbox_item`
//!
//! 聚合窗口为纯函数（除 DB / stat IO 外），单测覆盖（§6 性能要求）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::SqlitePool;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};
use crate::watch::{WatchEvent, WatchEventKind};

// ============================================================
// 常量（§5.2 冻结）
// ============================================================

/// 聚合窗口长度（秒）。详细设计 §5.2 冻结值。
pub const AGGREGATE_WINDOW_SECS: i64 = 5;

// ============================================================
// 数据模型
// ============================================================

/// 一个聚合窗口的事件批次。
#[derive(Debug, Clone)]
pub struct EventBatch {
    /// 窗口起始（Unix 秒，含）。
    #[allow(dead_code)] // 契约字段（§5.2 冻结），5.4 收件箱 UI 可能用到
    pub window_start: i64,
    /// 窗口结束（Unix 秒，不含）。
    #[allow(dead_code)] // 契约字段（§5.2 冻结）
    pub window_end: i64,
    /// 窗口内累积的事件。
    pub events: Vec<WatchEvent>,
}

/// 聚合结果统计。
///
/// 窗口有实际写入（created/updated/staled > 0）时作为
/// `inbox-changed` 事件的载荷 emit 给前端，驱动收件箱列表即时刷新。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct AggregateResult {
    /// 新建 inbox_item 数。
    pub created: usize,
    /// 更新已有 inbox_item 数（含改名跟随的 path 迁移）。
    pub updated: usize,
    /// 源文件已不存在、被标记为 stale 的条目数。
    pub staled: usize,
    /// 丢弃数（已正式引用 / 同窗口重复 / 已消失文件无已有条目）。
    pub dropped: usize,
}

// ============================================================
// 内部辅助
// ============================================================

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// WatchEventKind → DB 字符串（与 inbox_item.event_kind CHECK 一致）。
fn kind_to_db(kind: WatchEventKind) -> &'static str {
    match kind {
        WatchEventKind::Created => "created",
        WatchEventKind::Modified => "modified",
        WatchEventKind::Renamed => "renamed",
        WatchEventKind::Removed => "removed",
    }
}

/// 提取路径扩展名（小写，不带点）。无扩展名返回 None。
fn lower_ext(path: &std::path::Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

/// 同路径去重：保留 `at` 最新的一条；`at` 并列（同一秒内多事件）时保留**后到达**的。
///
/// 并列时取后到的理由：浏览器下载、编辑器原子保存等场景里，同一路径的
/// `Created` 与 `Renamed`/`Removed` 常落在同一秒，后到达的事件携带更新的
/// 文件状态（如「已改名消失」），保留先到者会把幽灵 Created 留给落账阶段。
///
/// 返回 (去重后事件列表, 重复丢弃数)。顺序不保证。
fn dedup_by_path(events: Vec<WatchEvent>) -> (Vec<WatchEvent>, usize) {
    let mut map: HashMap<PathBuf, WatchEvent> = HashMap::new();
    let total = events.len();
    for ev in events {
        match map.get(&ev.path) {
            Some(existing) if existing.at > ev.at => {
                // 已有更晚（严格大于）的事件，丢弃当前
            }
            _ => {
                // 更晚或并列（at 相等）：以后到达者为准
                map.insert(ev.path.clone(), ev);
            }
        }
    }
    let deduped: Vec<WatchEvent> = map.into_values().collect();
    let dropped = total - deduped.len();
    (deduped, dropped)
}

// ============================================================
// 聚合核心（纯函数 + DB / stat IO）
// ============================================================

/// 路径是否已被正式引用（`resource_reference.locator_json` 的 `$.path` 匹配）。
async fn is_referenced(pool: &SqlitePool, path_str: &str) -> CmdResult<bool> {
    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(1) FROM resource_reference \
         WHERE json_extract(locator_json, '$.path') = ?",
    )
    .bind(path_str)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;
    Ok(cnt > 0)
}

/// 查指定路径上处于 pending/snoozed 的 inbox_item id（取最新一条）。
async fn find_open_item_at(pool: &SqlitePool, path_str: &str) -> CmdResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM inbox_item \
         WHERE path = ? AND status IN ('pending', 'snoozed') \
         ORDER BY discovered_at DESC LIMIT 1",
    )
    .bind(path_str)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?;
    Ok(row.map(|(id,)| id))
}

/// 新建一条 pending inbox_item。
async fn insert_item(
    pool: &SqlitePool,
    ev: &WatchEvent,
    path_str: &str,
    path: &std::path::Path,
    kind_str: &str,
) -> CmdResult<()> {
    let id = Uuid::new_v4().to_string();
    let ext = lower_ext(path);
    let discovered_at = now_unix_secs();
    sqlx::query(
        "INSERT INTO inbox_item \
         (id, watch_dir_id, path, event_kind, size_bytes, mtime, ext, \
          suggested_type, status, assign_json, ignore_rule_id, snooze_note, \
          remind_at, discovered_at) \
         VALUES (?, ?, ?, ?, NULL, ?, ?, NULL, 'pending', NULL, NULL, NULL, NULL, ?)",
    )
    .bind(&id)
    .bind(&ev.watch_dir_id)
    .bind(path_str)
    .bind(kind_str)
    .bind(ev.at)
    .bind(&ext)
    .bind(discovered_at)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// `Created` / `Modified`：文件出现在某路径。
/// 已正式引用 → dropped；已有 pending → 文件仍在则 UPDATE，
/// 文件已消失（改名/删除的消失侧事件在同秒去重中丢失等）→ 标记 stale；
/// 否则新建前 stat：路径已不存在（窗口内消失的临时文件）→ dropped，存在 → INSERT。
async fn handle_appeared(
    pool: &SqlitePool,
    ev: &WatchEvent,
    path: &std::path::Path,
    path_str: &str,
    kind_str: &str,
    result: &mut AggregateResult,
) -> CmdResult<()> {
    if is_referenced(pool, path_str).await? {
        result.dropped += 1;
        return Ok(());
    }

    // 已有 pending inbox_item（注意：stale 不阻塞，允许同路径重新出现）
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM inbox_item WHERE path = ? AND status = 'pending' LIMIT 1")
            .bind(path_str)
            .fetch_optional(pool)
            .await
            .map_err(AppError::from)?;

    if let Some((id,)) = existing {
        // 落账前以磁盘状态裁决：文件已消失 → 该条目是改名/删除后的幽灵，
        // 标记 stale 而非 UPDATE（否则浏览器下载的 .crdownload 临时文件条目
        // 会永远以 pending 残留——其 Renamed 消失侧事件可能与 Created/Modified
        // 同秒到达并在去重中丢失）。
        if !path.exists() {
            sqlx::query(
                "UPDATE inbox_item SET status = 'stale', mtime = ?, event_kind = ? WHERE id = ?",
            )
            .bind(ev.at)
            .bind(kind_str)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
            result.staled += 1;
            return Ok(());
        }
        sqlx::query("UPDATE inbox_item SET mtime = ?, event_kind = ? WHERE id = ?")
            .bind(ev.at)
            .bind(kind_str)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
        result.updated += 1;
        return Ok(());
    }

    // 新建前 stat：文件在聚合窗口内已消失（编辑器临时文件、下载缓冲等）
    // → 丢弃，不新建「源文件已不在」的幽灵 pending 条目。
    if !path.exists() {
        result.dropped += 1;
        return Ok(());
    }

    insert_item(pool, ev, path_str, path, kind_str).await?;
    result.created += 1;
    Ok(())
}

/// `Removed` / `Renamed`（无 new_path，Any 等）：文件从某路径消失。
/// - 有 pending/snoozed 条目：磁盘上路径已不存在 → 标记 `stale`；
///   路径仍在（事件误报 / 快速重建）→ 仅更新 mtime/event_kind。
/// - 无已有条目：路径已不存在 → dropped（不为已消失的文件新建待处理）；
///   路径实际存在 → 按「出现」处理（新建 pending）。
///   关键场景：macOS FSEvents 把「从监控目录外移入文件」报成
///   `Modify(Name(RenameMode::Any))`，标准化为 Renamed + new_path=None，
///   路径是**目标**路径；若以「消失」语义丢弃，移入的文件永远进不了收件箱
///   （目录监控时灵时不灵的根因）。聚合层以磁盘状态裁决。
async fn handle_vanished(
    pool: &SqlitePool,
    ev: &WatchEvent,
    path_str: &str,
    kind_str: &str,
    result: &mut AggregateResult,
) -> CmdResult<()> {
    let Some(id) = find_open_item_at(pool, path_str).await? else {
        if ev.path.exists() {
            // 路径存在但未跟踪：实际是文件「出现」在该路径
            // （FSEvents 移入报 Any / 删除后同窗口快速重建等），按出现处理。
            return handle_appeared(pool, ev, &ev.path, path_str, kind_str, result).await;
        }
        result.dropped += 1;
        return Ok(());
    };

    if ev.path.exists() {
        sqlx::query("UPDATE inbox_item SET mtime = ?, event_kind = ? WHERE id = ?")
            .bind(ev.at)
            .bind(kind_str)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
        result.updated += 1;
    } else {
        sqlx::query(
            "UPDATE inbox_item SET status = 'stale', mtime = ?, event_kind = ? WHERE id = ?",
        )
        .bind(ev.at)
        .bind(kind_str)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
        result.staled += 1;
    }
    Ok(())
}

/// `Renamed` 且携带 `new_path`（RenameMode::Both）：改名跟随。
/// - from 有 pending/snoozed 条目 → 直接把该条目 path 改为 to（不新建）；
///   to 上若恰好已有 pending 条目（极端冲突）→ from 条目标记 stale，避免重复。
/// - from 无条目 → 按 Created(to) 语义处理（event_kind 记为 renamed）。
async fn handle_rename(
    pool: &SqlitePool,
    ev: &WatchEvent,
    to_path: &std::path::Path,
    result: &mut AggregateResult,
) -> CmdResult<()> {
    let from_str = ev.path.to_string_lossy().to_string();
    let to_str = to_path.to_string_lossy().to_string();

    let Some(from_id) = find_open_item_at(pool, &from_str).await? else {
        // from 侧没有被跟踪的条目：等价于新文件出现在 to
        return handle_appeared(pool, ev, to_path, &to_str, "renamed", result).await;
    };

    // to 侧已有其他 pending 条目 → 不迁移，from 条目 stale，避免同路径重复
    let to_existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM inbox_item WHERE path = ? AND status = 'pending' AND id != ? LIMIT 1",
    )
    .bind(&to_str)
    .bind(&from_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?;
    if to_existing.is_some() {
        sqlx::query("UPDATE inbox_item SET status = 'stale', mtime = ?, event_kind = 'renamed' WHERE id = ?")
            .bind(ev.at)
            .bind(&from_id)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
        result.staled += 1;
        return Ok(());
    }

    // 改名跟随：path / ext / mtime / event_kind 一并迁移
    let ext = lower_ext(to_path);
    sqlx::query(
        "UPDATE inbox_item SET path = ?, ext = ?, mtime = ?, event_kind = 'renamed' WHERE id = ?",
    )
    .bind(&to_str)
    .bind(&ext)
    .bind(ev.at)
    .bind(&from_id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    result.updated += 1;
    Ok(())
}

/// 聚合一个窗口批次，写入 inbox_item。
///
/// 流程：
/// 1. 同路径去重（取 `at` 最新），按 `at` 排序保证处理顺序可预期
/// 2. `Renamed` + new_path → 改名跟随（handle_rename）
/// 3. `Removed` / `Renamed`（无 new_path）→ stale 兜底（handle_vanished）
/// 4. `Created` / `Modified` → 引用检查 / 更新 / 新建（handle_appeared）
pub async fn aggregate_batch(pool: &SqlitePool, batch: EventBatch) -> CmdResult<AggregateResult> {
    let mut result = AggregateResult::default();

    if batch.events.is_empty() {
        return Ok(result);
    }

    // 1) 同路径去重 + 按 at 排序
    let (mut deduped, dup_dropped) = dedup_by_path(batch.events);
    result.dropped += dup_dropped;
    deduped.sort_by_key(|ev| ev.at);

    // 2) 逐条处理
    for ev in deduped {
        let kind_str = kind_to_db(ev.kind);
        match (ev.kind, ev.new_path.clone()) {
            (WatchEventKind::Renamed, Some(to)) => {
                handle_rename(pool, &ev, &to, &mut result).await?;
            }
            (WatchEventKind::Removed, _) | (WatchEventKind::Renamed, None) => {
                let path_str = ev.path.to_string_lossy().to_string();
                handle_vanished(pool, &ev, &path_str, kind_str, &mut result).await?;
            }
            (WatchEventKind::Created | WatchEventKind::Modified, _) => {
                let path_str = ev.path.to_string_lossy().to_string();
                handle_appeared(pool, &ev, &ev.path, &path_str, kind_str, &mut result).await?;
            }
        }
    }

    Ok(result)
}

// ============================================================
// 聚合管道（5.2 接入点）
// ============================================================

/// 收件箱变更事件名：聚合窗口有实际写入时 emit，前端收件箱页据此静默刷新列表。
pub const INBOX_CHANGED_EVENT: &str = "inbox-changed";

/// 在 `WatchEvent` 流上应用聚合窗口：
/// - 每 `AGGREGATE_WINDOW_SECS` 秒触发一次 `aggregate_batch`
/// - 窗口内累积的事件作为一个批次
/// - 输入通道关闭后，对残留事件做最后一次聚合再退出
/// - 窗口有实际写入（created/updated/staled > 0）时 emit `inbox-changed`
///
/// 返回 `JoinHandle`，调用方可选择 `await` 或 detach。
pub fn start_aggregator(
    pool: SqlitePool,
    mut rx: mpsc::Receiver<WatchEvent>,
    app_handle: tauri::AppHandle,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer: Vec<WatchEvent> = Vec::new();
        let mut window_start = now_unix_secs();
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs(AGGREGATE_WINDOW_SECS as u64));
        // 第一次 tick 立即触发，跳过以避免空批次
        ticker.tick().await;

        loop {
            tokio::select! {
                maybe_ev = rx.recv() => {
                    match maybe_ev {
                        Some(ev) => buffer.push(ev),
                        None => {
                            // 通道关闭：flush 残留后退出
                            if !buffer.is_empty() {
                                let batch = EventBatch {
                                    window_start,
                                    window_end: now_unix_secs(),
                                    events: std::mem::take(&mut buffer),
                                };
                                if let Err(e) = aggregate_batch(&pool, batch).await {
                                    eprintln!("[aggregate] 末次聚合失败: {}", e);
                                }
                            }
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    if buffer.is_empty() {
                        window_start = now_unix_secs();
                        continue;
                    }
                    let now = now_unix_secs();
                    let batch = EventBatch {
                        window_start,
                        window_end: now,
                        events: std::mem::take(&mut buffer),
                    };
                    window_start = now;
                    match aggregate_batch(&pool, batch).await {
                        Ok(r) => {
                            if r.created + r.updated + r.staled + r.dropped > 0 {
                                eprintln!(
                                    "[aggregate] 窗口处理: created={} updated={} staled={} dropped={}",
                                    r.created, r.updated, r.staled, r.dropped
                                );
                            }
                            // 有实际写入 → 通知前端收件箱刷新（事件驱动，免轮询）
                            if r.created + r.updated + r.staled > 0 {
                                use tauri::Emitter;
                                if let Err(e) = app_handle.emit(INBOX_CHANGED_EVENT, r) {
                                    eprintln!(
                                        "[aggregate] emit {} 失败: {}",
                                        INBOX_CHANGED_EVENT, e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[aggregate] 聚合失败: {}", e);
                        }
                    }
                }
            }
        }
    })
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 建内存 DB：最小化的 resource_reference / inbox_item / watch_dir 表。
    async fn setup_pool() -> SqlitePool {
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
        .expect("create watch_dir");

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

    fn ev(path: &str, kind: WatchEventKind, at: i64) -> WatchEvent {
        WatchEvent {
            kind,
            path: PathBuf::from(path),
            new_path: None,
            watch_dir_id: "wd1".to_string(),
            at,
        }
    }

    /// 构造一条携带目标路径的改名事件（RenameMode::Both 标准化结果）。
    fn ev_rename(from: &str, to: &str, at: i64) -> WatchEvent {
        WatchEvent {
            kind: WatchEventKind::Renamed,
            path: PathBuf::from(from),
            new_path: Some(PathBuf::from(to)),
            watch_dir_id: "wd1".to_string(),
            at,
        }
    }

    /// 预置一条 inbox_item（默认 pending）。
    async fn seed_item(pool: &SqlitePool, id: &str, path: &str, status: &str) {
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, ext, status, discovered_at) \
             VALUES (?, 'wd1', ?, 'created', 50, 'txt', ?, 50)",
        )
        .bind(id)
        .bind(path)
        .bind(status)
        .execute(pool)
        .await
        .expect("seed inbox_item");
    }

    async fn fetch_row(pool: &SqlitePool, id: &str) -> (String, String, String, Option<String>) {
        sqlx::query_as("SELECT id, path, status, event_kind FROM inbox_item WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("fetch inbox_item")
    }

    fn batch(events: Vec<WatchEvent>) -> EventBatch {
        EventBatch {
            window_start: 0,
            window_end: AGGREGATE_WINDOW_SECS,
            events,
        }
    }

    // ---------- 窗口常量 ----------

    #[test]
    fn aggregate_window_secs_is_five() {
        assert_eq!(AGGREGATE_WINDOW_SECS, 5, "§5.2 冻结：5 秒聚合窗口");
    }

    // ---------- 空批次 ----------

    #[tokio::test]
    async fn empty_batch_returns_zeros() {
        let pool = setup_pool().await;
        let r = aggregate_batch(&pool, batch(Vec::new()))
            .await
            .expect("agg");
        assert_eq!(r.created, 0);
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 0);
    }

    // ---------- 同窗口同路径 3 事件 → 1 条 ----------

    #[tokio::test]
    async fn same_path_three_events_creates_one() {
        let pool = setup_pool().await;
        // 真实文件：新建路径在落账时必须存在于磁盘
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("x.txt");
        std::fs::write(&file, b"x").expect("write");
        let p = file.to_string_lossy().to_string();
        let events = vec![
            ev(&p, WatchEventKind::Created, 100),
            ev(&p, WatchEventKind::Modified, 101),
            ev(&p, WatchEventKind::Modified, 102),
        ];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 1, "同路径去重后只新建 1 条");
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 2, "2 条同路径重复被丢弃");

        // 数据库里只有 1 条，且 mtime/event_kind 取最新（at=102, modified）
        let rows: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT path, mtime, event_kind FROM inbox_item WHERE path = ?",
        )
        .bind(&p)
        .fetch_all(&pool)
        .await
        .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, 102);
        assert_eq!(rows[0].2, "modified");
    }

    // ---------- 同窗口不同路径 3 事件 → 3 条 ----------

    #[tokio::test]
    async fn different_paths_create_three() {
        let pool = setup_pool().await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let mut events = Vec::new();
        for name in ["1.txt", "2.txt", "3.txt"] {
            let f = tmp.path().join(name);
            std::fs::write(&f, b"x").expect("write");
            events.push(ev(&f.to_string_lossy(), WatchEventKind::Created, 100));
        }
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 3);
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 0);

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 3);
    }

    // ---------- 已正式引用 → dropped ----------

    #[tokio::test]
    async fn referenced_path_is_dropped() {
        let pool = setup_pool().await;
        // 插入一条 resource_reference，locator_json.path = /a/ref.txt
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, created_at, updated_at) \
             VALUES ('r1', 'c1', 's1', 'ref', 'document', 'external', \
                     '{\"kind\":\"path\",\"path\":\"/a/ref.txt\"}', 0, 0)",
        )
        .execute(&pool)
        .await
        .expect("insert ref");

        let events = vec![ev("/a/ref.txt", WatchEventKind::Created, 100)];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 0);
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 1, "已正式引用路径应丢弃");

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0, "不应写入 inbox_item");
    }

    // ---------- 已有 pending → updated ----------

    #[tokio::test]
    async fn pending_inbox_item_is_updated_not_created() {
        let pool = setup_pool().await;
        // 文件落账时真实存在 → 已有 pending 应更新而非新建
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("p.txt");
        std::fs::write(&file, b"x").expect("write");
        let p = file.to_string_lossy().to_string();
        // 预置一条 pending inbox_item
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES ('i1', 'wd1', ?, 'created', 50, 'pending', 50)",
        )
        .bind(&p)
        .execute(&pool)
        .await
        .expect("insert inbox");

        let events = vec![ev(&p, WatchEventKind::Modified, 200)];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 0, "已有 pending 不应新建");
        assert_eq!(r.updated, 1);
        assert_eq!(r.dropped, 0);

        // 仅一条记录，mtime/event_kind 已更新
        let rows: Vec<(String, i64, String)> =
            sqlx::query_as("SELECT id, mtime, event_kind FROM inbox_item WHERE path = ?")
                .bind(&p)
                .fetch_all(&pool)
                .await
                .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "i1", "应更新已有行而非新建");
        assert_eq!(rows[0].1, 200);
        assert_eq!(rows[0].2, "modified");
    }

    // ---------- 已有 pending 但文件已消失 → stale（Chrome .crdownload 幽灵条目修复） ----------

    #[tokio::test]
    async fn pending_item_with_vanished_file_is_staled_not_updated() {
        let pool = setup_pool().await;
        // 场景：慢速下载中 .crdownload 已落账 pending；下载完成改名后，
        // 同秒到达的 Renamed（消失侧）在去重中丢失，只剩 Created/Modified。
        // 落账时磁盘上该路径已不存在 → 应标记 stale，而非 UPDATE 成永远残留的幽灵。
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let gone = tmp.path().join("big.zip.crdownload").to_string_lossy().to_string();
        seed_item(&pool, "i1", &gone, "pending").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&gone, WatchEventKind::Modified, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.staled, 1, "文件已消失的 pending 条目应标记 stale");
        assert_eq!(r.updated, 0);

        let (_, _, status, _) = fetch_row(&pool, "i1").await;
        assert_eq!(status, "stale");
    }

    #[tokio::test]
    async fn dedup_same_second_tie_keeps_later_arrival() {
        let pool = setup_pool().await;
        // 场景：小文件秒下，Created 与 Renamed 同秒到达同一路径。
        // 去重应保留后到达的 Renamed（消失语义），而非信息更少的 Created。
        // from 无被跟踪条目且磁盘上已不存在 → dropped，不产生幽灵 pending。
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let gone = tmp.path().join("small.zip.crdownload").to_string_lossy().to_string();

        let events = vec![
            ev(&gone, WatchEventKind::Created, 200),
            ev(&gone, WatchEventKind::Renamed, 200),
        ];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 0, "改名消失的临时文件不应落账");
        assert_eq!(r.dropped, 2, "1 条同路径重复 + 1 条已消失文件被丢弃");

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0);
    }

    // ---------- 已 processed 不更新，应新建 ----------

    #[tokio::test]
    async fn processed_inbox_item_does_not_block_new_insert() {
        let pool = setup_pool().await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("q.txt");
        std::fs::write(&file, b"x").expect("write");
        let p = file.to_string_lossy().to_string();
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES ('i_old', 'wd1', ?, 'created', 50, 'processed', 50)",
        )
        .bind(&p)
        .execute(&pool)
        .await
        .expect("insert");

        let events = vec![ev(&p, WatchEventKind::Created, 300)];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 1, "已 processed 不阻塞新建");
        assert_eq!(r.updated, 0);

        let (cnt,): (i64,) =
            sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE path = ?")
                .bind(&p)
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(cnt, 2, "应存在旧 processed + 新 pending 两条");
    }

    // ---------- dedup_by_path：取最新 ----------

    #[test]
    fn dedup_keeps_latest_event() {
        let events = vec![
            ev("/a/x", WatchEventKind::Created, 1),
            ev("/a/x", WatchEventKind::Modified, 5),
            ev("/a/x", WatchEventKind::Removed, 3),
        ];
        let (out, dropped) = dedup_by_path(events);
        assert_eq!(out.len(), 1);
        assert_eq!(dropped, 2);
        assert_eq!(out[0].at, 5, "保留 at 最新一条");
        assert_eq!(out[0].kind, WatchEventKind::Modified);
    }

    // ---------- 改名跟随（Renamed + new_path，RenameMode::Both） ----------

    #[tokio::test]
    async fn rename_follow_moves_pending_item_to_new_path() {
        let pool = setup_pool().await;
        seed_item(&pool, "i1", "/a/old.txt", "pending").await;

        // 改名同时换了扩展名，ext 应一并迁移
        let r = aggregate_batch(
            &pool,
            batch(vec![ev_rename("/a/old.txt", "/a/new.md", 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.updated, 1, "应迁移已有条目而非新建");
        assert_eq!(r.created, 0);
        assert_eq!(r.staled, 0);

        let (id, path, status, kind) = fetch_row(&pool, "i1").await;
        assert_eq!(id, "i1");
        assert_eq!(path, "/a/new.md", "path 应跟随到新路径");
        assert_eq!(status, "pending");
        assert_eq!(kind.as_deref(), Some("renamed"));
        let (ext,): (Option<String>,) =
            sqlx::query_as("SELECT ext FROM inbox_item WHERE id = 'i1'")
                .fetch_one(&pool)
                .await
                .expect("ext");
        assert_eq!(ext.as_deref(), Some("md"), "ext 应随新路径重算");

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 1, "全库仍只有 1 条，不产生重复待处理");
    }

    #[tokio::test]
    async fn rename_follow_moves_snoozed_item_keeping_status() {
        let pool = setup_pool().await;
        seed_item(&pool, "i1", "/a/old.txt", "snoozed").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev_rename("/a/old.txt", "/a/new.txt", 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.updated, 1);

        let (_, path, status, _) = fetch_row(&pool, "i1").await;
        assert_eq!(path, "/a/new.txt");
        assert_eq!(status, "snoozed", "暂后状态应保持连续");
    }

    #[tokio::test]
    async fn rename_follow_without_from_item_creates_at_to() {
        let pool = setup_pool().await;
        // from 无被跟踪条目（例如从监听目录外改名进来）→ 等价于新文件出现在 to
        // to 路径落账时必须真实存在（新建前 stat）
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let to = tmp.path().join("in.txt");
        std::fs::write(&to, b"x").expect("write");
        let to_str = to.to_string_lossy().to_string();
        let from_str = tmp.path().join("out.txt").to_string_lossy().to_string();

        let r = aggregate_batch(
            &pool,
            batch(vec![ev_rename(&from_str, &to_str, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 1);
        assert_eq!(r.updated, 0);

        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT path, event_kind FROM inbox_item")
                .fetch_all(&pool)
                .await
                .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, to_str);
        assert_eq!(rows[0].1.as_deref(), Some("renamed"));
    }

    #[tokio::test]
    async fn rename_onto_existing_pending_marks_from_stale() {
        let pool = setup_pool().await;
        seed_item(&pool, "i_from", "/a/old.txt", "pending").await;
        seed_item(&pool, "i_to", "/a/new.txt", "pending").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev_rename("/a/old.txt", "/a/new.txt", 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.staled, 1, "to 已有 pending，from 条目应 stale 避免重复");
        assert_eq!(r.updated, 0);

        let (_, _, from_status, _) = fetch_row(&pool, "i_from").await;
        assert_eq!(from_status, "stale");
        let (_, to_path, to_status, _) = fetch_row(&pool, "i_to").await;
        assert_eq!(to_path, "/a/new.txt");
        assert_eq!(to_status, "pending");
    }

    // ---------- 单侧 From+To 分裂（macOS FSEvents 常见）：A 兜底 ----------

    #[tokio::test]
    async fn rename_split_from_to_yields_one_stale_one_pending() {
        let pool = setup_pool().await;
        // old 不存在（已改名消失）；new 必须真实存在（新建前 stat）
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let new_file = tmp.path().join("new.txt");
        std::fs::write(&new_file, b"x").expect("write");
        let new_str = new_file.to_string_lossy().to_string();
        let old_str = tmp.path().join("old.txt").to_string_lossy().to_string();
        seed_item(&pool, "i1", &old_str, "pending").await;

        // watch 层标准化结果：From → Removed(old)，To → Created(new)
        let events = vec![
            ev(&old_str, WatchEventKind::Removed, 200),
            ev(&new_str, WatchEventKind::Created, 200),
        ];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.staled, 1, "旧路径条目应标记 stale");
        assert_eq!(r.created, 1, "新路径应新建 1 条 pending");

        let (_, _, old_status, _) = fetch_row(&pool, "i1").await;
        assert_eq!(old_status, "stale");
        let (pending_cnt,): (i64,) =
            sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE status = 'pending'")
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(pending_cnt, 1, "改名后只剩 1 条待处理");
    }

    // ---------- Removed / 无 new_path 的 Renamed：stale 兜底 ----------

    #[tokio::test]
    async fn removed_marks_pending_item_stale_when_file_gone() {
        let pool = setup_pool().await;
        // /a/gone.txt 在磁盘上不存在 → 应标记 stale
        seed_item(&pool, "i1", "/a/gone.txt", "pending").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev("/a/gone.txt", WatchEventKind::Removed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.staled, 1);
        assert_eq!(r.updated, 0);

        let (_, _, status, kind) = fetch_row(&pool, "i1").await;
        assert_eq!(status, "stale");
        assert_eq!(kind.as_deref(), Some("removed"));
    }

    #[tokio::test]
    async fn removed_marks_snoozed_item_stale_when_file_gone() {
        let pool = setup_pool().await;
        seed_item(&pool, "i1", "/a/gone.txt", "snoozed").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev("/a/gone.txt", WatchEventKind::Removed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.staled, 1);

        let (_, _, status, _) = fetch_row(&pool, "i1").await;
        assert_eq!(status, "stale", "暂后条目源文件消失同样应标记 stale");
    }

    #[tokio::test]
    async fn removed_keeps_pending_when_file_still_exists() {
        let pool = setup_pool().await;
        // 文件实际仍存在（事件误报 / 删除后快速重建）→ 不标 stale，仅更新
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("still-here.txt");
        std::fs::write(&file, b"x").expect("write");
        let path_str = file.to_string_lossy().to_string();
        seed_item(&pool, "i1", &path_str, "pending").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&path_str, WatchEventKind::Removed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.updated, 1);
        assert_eq!(r.staled, 0);

        let (_, _, status, _) = fetch_row(&pool, "i1").await;
        assert_eq!(status, "pending", "文件仍在，条目应保持 pending");
    }

    #[tokio::test]
    async fn removed_without_existing_item_is_dropped() {
        let pool = setup_pool().await;
        // 无已有条目：不为已消失的文件新建待处理
        let r = aggregate_batch(
            &pool,
            batch(vec![ev("/a/ghost.txt", WatchEventKind::Removed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 0);
        assert_eq!(r.dropped, 1);

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0);
    }

    // ---------- Removed / 无 new_path 的 Renamed：路径存在但未跟踪 → 按出现处理 ----------
    //
    // 背景：macOS FSEvents 把「从监控目录外移入文件」报成 Modify(Name(RenameMode::Any))，
    // watch 层标准化为 Renamed + new_path=None，路径是**目标**路径。
    // 旧逻辑走 handle_vanished 且无已有条目 → dropped，文件在目录里却永远进不了收件箱
    // （用户感知：目录监控时灵时不灵）。聚合层以磁盘状态裁决：路径存在 → 按出现处理。

    #[tokio::test]
    async fn renamed_any_untracked_existing_file_creates_pending() {
        let pool = setup_pool().await;
        // 真实文件：模拟移入监控目录后落在磁盘上的目标文件
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("moved-in.txt");
        std::fs::write(&file, b"x").expect("write");
        let path_str = file.to_string_lossy().to_string();

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&path_str, WatchEventKind::Renamed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 1, "路径存在但未跟踪的 Renamed(Any) 应新建 pending 条目");
        assert_eq!(r.dropped, 0);

        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT status, event_kind FROM inbox_item WHERE path = ?")
                .bind(&path_str)
                .fetch_all(&pool)
                .await
                .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "pending");
        assert_eq!(rows[0].1.as_deref(), Some("renamed"));
    }

    #[tokio::test]
    async fn renamed_any_nonexistent_path_still_dropped() {
        let pool = setup_pool().await;
        // 路径不存在且无已有条目：维持旧行为（不为已消失的文件新建待处理）
        let r = aggregate_batch(
            &pool,
            batch(vec![ev("/a/ghost.txt", WatchEventKind::Renamed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 0);
        assert_eq!(r.dropped, 1);

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0);
    }

    #[tokio::test]
    async fn removed_untracked_existing_file_creates_pending() {
        let pool = setup_pool().await;
        // 删除后同窗口快速重建：dedup 可能只保留 Removed，但文件实际在 → 按出现处理
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("recreated.txt");
        std::fs::write(&file, b"x").expect("write");
        let path_str = file.to_string_lossy().to_string();

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&path_str, WatchEventKind::Removed, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 1, "路径存在但未跟踪的 Removed（快速重建）应新建 pending 条目");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM inbox_item WHERE path = ?")
                .bind(&path_str)
                .fetch_one(&pool)
                .await
                .expect("select");
        assert_eq!(status, "pending");
    }

    // ---------- 窗口内已消失的文件不新建幽灵条目 ----------

    #[tokio::test]
    async fn created_for_vanished_file_is_dropped() {
        let pool = setup_pool().await;
        // 文件在聚合窗口内出现又消失（编辑器临时文件、下载缓冲等）：
        // 落账时磁盘上已不存在 → 不新建幽灵 pending 条目。
        let r = aggregate_batch(
            &pool,
            batch(vec![ev("/a/ghost-tmp.txt", WatchEventKind::Created, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 0, "落账时文件已不存在，不应新建");
        assert_eq!(r.dropped, 1);

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM inbox_item")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0);
    }

    #[tokio::test]
    async fn modified_for_existing_file_still_creates() {
        let pool = setup_pool().await;
        // 对照：文件落账时真实存在 → 正常新建
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("real.txt");
        std::fs::write(&file, b"x").expect("write");
        let path_str = file.to_string_lossy().to_string();

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&path_str, WatchEventKind::Modified, 200)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 1);
    }

    // ---------- stale 不阻塞同路径重新出现 ----------

    #[tokio::test]
    async fn stale_item_does_not_block_recreated_file() {
        let pool = setup_pool().await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let file = tmp.path().join("back.txt");
        std::fs::write(&file, b"x").expect("write");
        let p = file.to_string_lossy().to_string();
        seed_item(&pool, "i_old", &p, "stale").await;

        let r = aggregate_batch(
            &pool,
            batch(vec![ev(&p, WatchEventKind::Created, 300)]),
        )
        .await
        .expect("agg");
        assert_eq!(r.created, 1, "stale 不阻塞同路径新建");
        assert_eq!(r.updated, 0);

        let (cnt,): (i64,) =
            sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE path = ?")
                .bind(&p)
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(cnt, 2, "旧 stale + 新 pending 两条");
    }
}
