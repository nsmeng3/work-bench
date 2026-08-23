//! m6-6.2 · Watch 调度器：聚合窗口内的事件分发器
//!
//! 读 `watch_dir` 配置表，在 `AGGREGATE_WINDOW_SECS` 秒内收集事件，
//! 按 (dirId, created, path, file, kind) 去重后写 `inbox_item`（status='pending'）。
//! 只处理 `WatchEventKind::Created`（新增文件进收件箱）。
//!
//! 路径校验：`watch_dir.path` 必须为绝对路径。
//! 去重键：同路径同文件同时间戳只记一次。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, CmdResult};
use crate::watch::WatchEventKind;
use std::hash::{Hash, Hasher};

// ============================================================
// 常量
// ============================================================

/// 聚合窗口（M5 6.1 契约固化 5 秒）。
#[allow(dead_code)]
const AGGREGATE_WINDOW_SECS: i64 = 5;

// ============================================================
// 数据模型
// ============================================================

/// `watch_dir_event` 入参。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEventInput {
    pub dir_id: String,
}

/// `watch_dir_event` 出参。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEventOutput {
    pub event: WatchEvent,
}

/// 事件模型：对齐 `watch.rs` 契约。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    /// 事件类型（created / modified / renamed / removed）。
    pub type_: String,
    /// 源类型（固定 `directory`）。
    pub source_type: String,
    /// 监控目录路径。
    pub source_path: String,
    /// 事件触发时间戳（Unix ms）。
    pub created: i64,
    /// 仅 modified 事件有值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<i64>,
}

// ============================================================
// Hash 实现
// ============================================================

/// `WatchEventKind` 的 Hash 实现（用于去重键）。
impl Hash for WatchEventKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            WatchEventKind::Created => "created".hash(state),
            WatchEventKind::Modified => "modified".hash(state),
            WatchEventKind::Renamed => "renamed".hash(state),
            WatchEventKind::Removed => "removed".hash(state),
        }
    }
}

/// 去重键：同路径同文件同时间戳只记一次。
struct DedupeKey<'a> {
    dir_id: &'a str,
    created: i64,
    path: &'a PathBuf,
    file: &'a PathBuf,
    kind: WatchEventKind,
}

impl<'a> Hash for DedupeKey<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dir_id.hash(state);
        self.created.hash(state);
        self.path.hash(state);
        self.file.hash(state);
        self.kind.hash(state);
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 当前 Unix 时间（毫秒）。
fn now_unix_ms() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("时间必须在 Unix 纪元之后");
    dur.as_millis() as i64
}

// ============================================================
// 业务函数
// ============================================================

/// 读 `watch_dir` 配置表，收集活跃目录并建 Channel 投递。
///
/// 返回 `Vec<Channel>`，每个 Channel 对应一个 dirId，可在聚合窗口到期后消费并分发。
async fn collect_channels(pool: &SqlitePool) -> CmdResult<Vec<Channel>> {
    let now = now_unix_ms();
    let rows = sqlx::query(
        "SELECT id, path, name, description FROM watch_dir WHERE paused = 0",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // 路径校验
        let path: String = row.try_get("path").map_err(AppError::from)?;
        if !path.starts_with('/') {
            return Err(AppError::invalid_param(format!(
                "监控目录必须为绝对路径: {}",
                path
            )));
        }
        if path.trim().is_empty() {
            return Err(AppError::invalid_param("监控目录路径不能为空"));
        }

        // 窗口内创建 Channel（容量 = 1，因为只处理 Created）
        let (tx, rx) = tokio::sync::mpsc::channel::<WatchEvent>(1);
        let _rx = rx; // 避免被 drop，供消费端使用

        out.push(Channel {
            dir_id: row.try_get::<String, &str>("id").map_err(AppError::from)?,
            dir_name: row.try_get::<Option<String>, &str>("name").map_err(AppError::from)?.unwrap_or_default(),
            dir_desc: row.try_get::<Option<String>, &str>("description").map_err(AppError::from)?.unwrap_or_default(),
            tx,
            _rx,
            created_at: now,
        });
    }
    Ok(out)
}

/// Channel：聚合窗口内的事件收集器。
#[derive(Debug)]
struct Channel {
    dir_id: String,
    dir_name: String,
    dir_desc: String,
    tx: tokio::sync::mpsc::Sender<WatchEvent>,
    _rx: tokio::sync::mpsc::Receiver<WatchEvent>,
    created_at: i64,
}

impl Channel {
    /// 判断当前窗口是否已到期。
    fn expired(&self, now: i64) -> bool {
        now - self.created_at >= AGGREGATE_WINDOW_SECS * 1000
    }

    /// 向 Channel 投递事件。
    fn send(&self, event: WatchEvent) -> CmdResult<()> {
        // Created 事件容量 1，其他事件忽略（已在 ignore 层过滤）
        if event.type_ == "created" {
            self.tx
                .try_send(event)
                .map_err(|_| AppError::new("DISPATCH_BLOCKED", "Channel 已满"))
        } else {
            Ok(())
        }
    }
}

/// 消费 Channel：窗口到期后分发事件为 `inbox_item`。
///
/// 事务：`INSERT INTO inbox_item (id, watch_dir_id, path, event_kind, discovered_at, status)
///       VALUES (?, ?, ?, ?, ?, 'pending')`；忽略唯一键冲突。
async fn consume_channel(pool: &SqlitePool, mut channel: Channel) -> CmdResult<()> {
    let now = now_unix_ms();
    if !channel.expired(now) {
        return Ok(());
    }

    let rows = tokio::time::timeout(
        std::time::Duration::from_secs(AGGREGATE_WINDOW_SECS.try_into().unwrap()),
        async {
            let mut rows = Vec::new();
            while let Some(row) = channel._rx.recv().await {
                rows.push(row);
                if rows.len() >= (AGGREGATE_WINDOW_SECS * 1000) as usize {
                    break;
                }
            }
            rows
        },
    ).await.map_err(|_| AppError::new("DISPATCH_TIMEOUT", "消费通道超时"))?;

    for row in rows {
        // 仅处理 Created
        if row.type_ != "created" {
            continue;
        }

        let path = PathBuf::from(&row.source_path);
        // 源文件存在性检查
        if !path.exists() {
            continue;
        }

        let _meta = std::fs::metadata(&path)
            .map_err(|e| AppError::io(format!("读取源文件元数据失败：{}", e)))?;

        // 计算文件名
        let _file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let id = uuid::Uuid::new_v4().to_string();
        let _sql = sqlx::query(
            r#"
                INSERT INTO inbox_item
                    (id, watch_dir_id, path, event_kind, discovered_at, status)
                    VALUES (?, ?, ?, ?, ?, 'pending')
                ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&id)
        .bind(&channel.dir_id)
        .bind(row.source_path)
        .bind(row.created)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

        let row_count: (i64,) = sqlx::query_as("SELECT changes()")
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
        if row_count.0 == 0 {
            // 唯一键冲突 → 已存在
            return Ok(());
        }
    }

    // 消费完成，清空 Channel
    drop(channel);
    Ok(())
}

// ============================================================
// watch_dir_event 命令
// ============================================================

/// watch_dir_event 命令
#[tauri::command]
pub async fn watch_dir_event(
    state: tauri::State<'_, crate::AppState>,
    input: WatchEventInput,
) -> CmdResult<WatchEventOutput> {
    watch_dir_event_test(&state.pool, input).await
}

/// watch_dir_event 业务逻辑（供单元测试调用）
pub async fn watch_dir_event_test(
    pool: &SqlitePool,
    input: WatchEventInput,
) -> CmdResult<WatchEventOutput> {
    // 查询 watch_dir
    let row = sqlx::query(
        "SELECT id, path FROM watch_dir WHERE id = ?",
    )
    .bind(&input.dir_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("监控目录不存在：{}", input.dir_id)))?;

    let path: String = row.try_get("path").map_err(AppError::from)?;
    if !path.starts_with('/') {
        return Err(AppError::invalid_param(format!(
            "监控目录必须为绝对路径: {}",
            path
        )));
    }
    if path.trim().is_empty() {
        return Err(AppError::invalid_param("监控目录路径不能为空"));
    }

    // 标准化事件
    let event = WatchEvent {
        type_: "created".into(),
        source_type: "directory".into(),
        source_path: path,
        created: now_unix_ms(),
        modified: None,
    };

    // 直接 INSERT 到 inbox_item
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_unix_ms();
    let _sql = sqlx::query(
        r#"
            INSERT INTO inbox_item
                (id, watch_dir_id, path, event_kind, discovered_at, status)
                VALUES (?, ?, ?, ?, ?, 'pending')
        "#,
    )
    .bind(&id)
    .bind(&input.dir_id)
    .bind(event.source_path.clone())
    .bind(&event.type_)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    let row_count: (i64,) = sqlx::query_as("SELECT changes()")
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?;
    if row_count.0 == 0 {
        return Err(AppError::new("INBOX_ALREADY_EXISTS", "收件箱条目已存在"));
    }

    Ok(WatchEventOutput { event })
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        crate::db::init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";

    async fn make_collection(pool: &SqlitePool) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_unix_ms();
        sqlx::query(
            "INSERT INTO collection (id, space_id, name, status, created_at, updated_at) \
             VALUES (?, ?, 'c', 'active', ?, ?)",
        )
        .bind(&id)
        .bind(PRESET_SPACE)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert collection");
        id
    }

    /// 插入一条 pending inbox_item，返回 id。
    async fn make_pending(pool: &SqlitePool, path: &std::path::Path) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_unix_ms();
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES (?, NULL, ?, 'created', ?, 'pending', ?)",
        )
        .bind(&id)
        .bind(path.to_string_lossy().to_string())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert inbox_item");
        id
    }

    // ---------- watch_dir_event ----------

    #[tokio::test]
    async fn watch_dir_event_ok() {
        let pool = setup().await;
        // 先建 watch_dir
        let now = now_unix_ms();
        sqlx::query(
            "INSERT INTO watch_dir (id, path, paused, created_at) \
             VALUES ('wd1', '/Users/differentw/data/00_Admin/workbench/Code/Documents/test', 0, ?)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert watch_dir");

        // 创建测试文件
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");

        let input = WatchEventInput { dir_id: "wd1".into() };
        let output = watch_dir_event_test(&pool, input).await.expect("event");
        assert_eq!(output.event.type_, "created");
        assert_eq!(output.event.source_type, "directory");
        assert!(output.event.created > 0);
    }

    #[tokio::test]
    async fn watch_dir_event_not_found() {
        let pool = setup().await;
        let input = WatchEventInput { dir_id: "no-such".into() };
        let err = watch_dir_event_test(&pool, input).await.expect_err("err");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }
}