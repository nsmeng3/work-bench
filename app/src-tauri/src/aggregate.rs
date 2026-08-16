//! m5-5.3 · 聚合窗口缓冲 + 去重（详细设计 §5.2「5 秒聚合窗口 + 同路径去重」）
//!
//! 消费 5.2 过滤后的 `WatchEvent` 流，按 5 秒窗口缓冲合并：
//! - 同窗口同路径多事件去重（取 `at` 最新一条）
//! - 路径已被正式引用（`resource_reference.locator_json` 的 `$.path` 匹配）→ 丢弃
//! - 同路径已有未处理 `inbox_item`（`status='pending'`）→ UPDATE mtime/event_kind，不新建
//! - 否则 → 新建 `inbox_item`
//!
//! 聚合窗口为纯函数（除 DB IO 外），单测覆盖（§6 性能要求）。

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AggregateResult {
    /// 新建 inbox_item 数。
    pub created: usize,
    /// 更新已有 inbox_item 数。
    pub updated: usize,
    /// 丢弃数（已正式引用 / 同窗口重复）。
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

/// 同路径去重：保留 `at` 最新的一条。
///
/// 返回 (去重后事件列表, 重复丢弃数)。顺序不保证。
fn dedup_by_path(events: Vec<WatchEvent>) -> (Vec<WatchEvent>, usize) {
    let mut map: HashMap<PathBuf, WatchEvent> = HashMap::new();
    let total = events.len();
    for ev in events {
        match map.get(&ev.path) {
            Some(existing) if existing.at >= ev.at => {
                // 已有更新或同时戳的事件，丢弃当前
            }
            _ => {
                map.insert(ev.path.clone(), ev);
            }
        }
    }
    let deduped: Vec<WatchEvent> = map.into_values().collect();
    let dropped = total - deduped.len();
    (deduped, dropped)
}

// ============================================================
// 聚合核心（纯函数 + DB IO）
// ============================================================

/// 聚合一个窗口批次，写入 inbox_item。
///
/// 流程：
/// 1. 同路径去重（取 `at` 最新）
/// 2. 已被正式引用（`resource_reference.locator_json` 的 `$.path` 匹配）→ dropped
/// 3. 已有 pending inbox_item → UPDATE mtime/event_kind → updated
/// 4. 否则 → INSERT 新 inbox_item → created
pub async fn aggregate_batch(
    pool: &SqlitePool,
    batch: EventBatch,
) -> CmdResult<AggregateResult> {
    let mut result = AggregateResult::default();

    if batch.events.is_empty() {
        return Ok(result);
    }

    // 1) 同路径去重
    let (deduped, dup_dropped) = dedup_by_path(batch.events);
    result.dropped += dup_dropped;

    // 2) 逐条处理
    for ev in deduped {
        let path_str = ev.path.to_string_lossy().to_string();

        // 2a) 已正式引用检查：resource_reference.locator_json 的 $.path 匹配
        let ref_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(1) FROM resource_reference \
             WHERE json_extract(locator_json, '$.path') = ?",
        )
        .bind(&path_str)
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?;
        if ref_count.0 > 0 {
            result.dropped += 1;
            continue;
        }

        // 2b) 已有 pending inbox_item → UPDATE
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM inbox_item WHERE path = ? AND status = 'pending' LIMIT 1",
        )
        .bind(&path_str)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;

        let mtime = ev.at;
        let kind_str = kind_to_db(ev.kind);

        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE inbox_item SET mtime = ?, event_kind = ? WHERE id = ?",
            )
            .bind(mtime)
            .bind(kind_str)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
            result.updated += 1;
            continue;
        }

        // 2c) 新建 inbox_item
        let id = Uuid::new_v4().to_string();
        let ext = lower_ext(&ev.path);
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
        .bind(&path_str)
        .bind(kind_str)
        .bind(mtime)
        .bind(&ext)
        .bind(discovered_at)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
        result.created += 1;
    }

    Ok(result)
}

// ============================================================
// 聚合管道（5.2 接入点）
// ============================================================

/// 在 `WatchEvent` 流上应用聚合窗口：
/// - 每 `AGGREGATE_WINDOW_SECS` 秒触发一次 `aggregate_batch`
/// - 窗口内累积的事件作为一个批次
/// - 输入通道关闭后，对残留事件做最后一次聚合再退出
///
/// 返回 `JoinHandle`，调用方可选择 `await` 或 detach。
pub fn start_aggregator(
    pool: SqlitePool,
    mut rx: mpsc::Receiver<WatchEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer: Vec<WatchEvent> = Vec::new();
        let mut window_start = now_unix_secs();
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(
            AGGREGATE_WINDOW_SECS as u64,
        ));
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
                            if r.created + r.updated + r.dropped > 0 {
                                eprintln!(
                                    "[aggregate] 窗口处理: created={} updated={} dropped={}",
                                    r.created, r.updated, r.dropped
                                );
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
            watch_dir_id: "wd1".to_string(),
            at,
        }
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
        let r = aggregate_batch(&pool, batch(Vec::new())).await.expect("agg");
        assert_eq!(r.created, 0);
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 0);
    }

    // ---------- 同窗口同路径 3 事件 → 1 条 ----------

    #[tokio::test]
    async fn same_path_three_events_creates_one() {
        let pool = setup_pool().await;
        let events = vec![
            ev("/a/x.txt", WatchEventKind::Created, 100),
            ev("/a/x.txt", WatchEventKind::Modified, 101),
            ev("/a/x.txt", WatchEventKind::Modified, 102),
        ];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 1, "同路径去重后只新建 1 条");
        assert_eq!(r.updated, 0);
        assert_eq!(r.dropped, 2, "2 条同路径重复被丢弃");

        // 数据库里只有 1 条，且 mtime/event_kind 取最新（at=102, modified）
        let rows: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT path, mtime, event_kind FROM inbox_item WHERE path = '/a/x.txt'",
        )
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
        let events = vec![
            ev("/a/1.txt", WatchEventKind::Created, 100),
            ev("/a/2.txt", WatchEventKind::Created, 100),
            ev("/a/3.txt", WatchEventKind::Created, 100),
        ];
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
        // 预置一条 pending inbox_item
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES ('i1', 'wd1', '/a/p.txt', 'created', 50, 'pending', 50)",
        )
        .execute(&pool)
        .await
        .expect("insert inbox");

        let events = vec![ev("/a/p.txt", WatchEventKind::Modified, 200)];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 0, "已有 pending 不应新建");
        assert_eq!(r.updated, 1);
        assert_eq!(r.dropped, 0);

        // 仅一条记录，mtime/event_kind 已更新
        let rows: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT id, mtime, event_kind FROM inbox_item WHERE path = '/a/p.txt'",
        )
        .fetch_all(&pool)
        .await
        .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "i1", "应更新已有行而非新建");
        assert_eq!(rows[0].1, 200);
        assert_eq!(rows[0].2, "modified");
    }

    // ---------- 已 processed 不更新，应新建 ----------

    #[tokio::test]
    async fn processed_inbox_item_does_not_block_new_insert() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES ('i_old', 'wd1', '/a/q.txt', 'created', 50, 'processed', 50)",
        )
        .execute(&pool)
        .await
        .expect("insert");

        let events = vec![ev("/a/q.txt", WatchEventKind::Created, 300)];
        let r = aggregate_batch(&pool, batch(events)).await.expect("agg");
        assert_eq!(r.created, 1, "已 processed 不阻塞新建");
        assert_eq!(r.updated, 0);

        let (cnt,): (i64,) =
            sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE path = '/a/q.txt'")
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
}
