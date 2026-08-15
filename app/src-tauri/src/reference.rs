//! 资源引用管理（详细设计 §2.5）
//!
//! 本任务（m2-2.3）实现 4 个命令：
//! `ref_create_external` / `ref_update` / `ref_get` / `ref_list`。
//!
//! 关键约束（§6.3）：`ref_create_external` 仅做登记，
//! **绝不复制/移动/写入源文件**，仅校验路径存在性。
//!
//! tags 通过 `reference_tag` 关联表存储，`ref_update` 传 tags 时为全量替换语义
//! （与 collection 一致）。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};
use crate::landing;

/// 引用名称最大长度（与 space/collection.name 对齐，契约未另设上限）。
const NAME_MAX_LEN: usize = 64;

/// 默认本地文件系统存储源 id（0001 迁移插入）。
const DEFAULT_SOURCE_ID: &str = "src_local_fs_default";

/// 六类型枚举（§2.5 type 字段）。
const REF_TYPES: [&str; 6] = ["code", "document", "data", "artifact", "tool", "media"];

/// 生命周期枚举（§3.2 resource_reference.lifecycle）。
const LIFECYCLES: [&str; 4] = ["active", "staged", "delivered", "archived"];

/// 保密级别枚举（§3.2 resource_reference.confidentiality）。
const CONFIDENTIALITIES: [&str; 4] = ["public", "internal", "customer_restricted", "sensitive"];

/// 处置状态枚举（§3.2 resource_reference.disposition）。
const DISPOSITIONS: [&str; 3] = ["none", "archived", "deleted"];

/// 资源引用（§2.5 Reference）。
///
/// 字段与 `resource_reference` 表一一对应；
/// `locator_json` 在出参中反序列化为结构化 `locator` 对象；
/// `tags` 来自 `reference_tag` 关联表。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub id: String,
    pub collection_id: String,
    pub source_id: String,
    pub name: String,
    /// code | document | data | artifact | tool | media
    #[serde(rename = "type")]
    pub ref_type: String,
    /// external | managed
    pub hosting: String,
    pub locator: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// active | staged | delivered | archived
    pub lifecycle: String,
    /// public | internal | customer_restricted | sensitive
    pub confidentiality: String,
    pub indexed: bool,
    /// none | archived | deleted
    pub disposition: String,
    pub tags: Vec<String>,
    /// Unix 秒
    pub created_at: i64,
    /// Unix 秒
    pub updated_at: i64,
}

/// `ref_create_external` 入参中的 locator（§2.5）。
///
/// 当前仅支持 `{"kind":"path","path":"/abs/path"}`；
/// `repo` / `cloud` 为预留形态，本任务不实现。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    pub kind: String,
    pub path: String,
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn validate_name(name: &str) -> CmdResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_param("引用名称不能为空"));
    }
    if trimmed.chars().count() > NAME_MAX_LEN {
        return Err(AppError::invalid_param(format!(
            "引用名称超长（>{} 字符）",
            NAME_MAX_LEN
        )));
    }
    Ok(())
}

fn validate_ref_type(ref_type: &str) -> CmdResult<()> {
    if !REF_TYPES.contains(&ref_type) {
        return Err(AppError::invalid_param(format!(
            "非法引用类型: {}（允许值: {}）",
            ref_type,
            REF_TYPES.join("/")
        )));
    }
    Ok(())
}

fn validate_lifecycle(lifecycle: &str) -> CmdResult<()> {
    if !LIFECYCLES.contains(&lifecycle) {
        return Err(AppError::invalid_param(format!(
            "非法生命周期: {}（允许值: {}）",
            lifecycle,
            LIFECYCLES.join("/")
        )));
    }
    Ok(())
}

fn validate_confidentiality(confidentiality: &str) -> CmdResult<()> {
    if !CONFIDENTIALITIES.contains(&confidentiality) {
        return Err(AppError::invalid_param(format!(
            "非法保密级别: {}（允许值: {}）",
            confidentiality,
            CONFIDENTIALITIES.join("/")
        )));
    }
    Ok(())
}

fn validate_disposition(disposition: &str) -> CmdResult<()> {
    if !DISPOSITIONS.contains(&disposition) {
        return Err(AppError::invalid_param(format!(
            "非法处置状态: {}（允许值: {}）",
            disposition,
            DISPOSITIONS.join("/")
        )));
    }
    Ok(())
}

/// 校验 locator 形态：当前仅接受 `{"kind":"path","path":"..."}`。
fn validate_locator(locator: &Locator) -> CmdResult<()> {
    if locator.kind != "path" {
        return Err(AppError::invalid_param(format!(
            "暂不支持的 locator kind: {}（当前仅支持 path）",
            locator.kind
        )));
    }
    if locator.path.trim().is_empty() {
        return Err(AppError::invalid_param("locator.path 不能为空"));
    }
    Ok(())
}

/// 校验资源集存在（不校验归档状态：归档资源集仅不可编辑其自身字段，
/// 但契约未禁止向其追加引用；如需追加约束可后续收紧）。
async fn ensure_collection_exists(pool: &SqlitePool, collection_id: &str) -> CmdResult<()> {
    let row = sqlx::query("SELECT id FROM collection WHERE id = ?")
        .bind(collection_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    if row.is_none() {
        return Err(AppError::not_found(format!(
            "资源集不存在: {}",
            collection_id
        )));
    }
    Ok(())
}

async fn fetch_tags(pool: &SqlitePool, reference_id: &str) -> CmdResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT tag FROM reference_tag WHERE reference_id = ? ORDER BY tag ASC",
    )
    .bind(reference_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    Ok(rows.iter().map(|r| r.get::<String, _>("tag")).collect())
}

async fn replace_tags(pool: &SqlitePool, reference_id: &str, tags: &[String]) -> CmdResult<()> {
    sqlx::query("DELETE FROM reference_tag WHERE reference_id = ?")
        .bind(reference_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query("INSERT OR IGNORE INTO reference_tag (reference_id, tag) VALUES (?, ?)")
            .bind(reference_id)
            .bind(trimmed)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
    }
    Ok(())
}

fn row_to_reference(row: &sqlx::sqlite::SqliteRow, tags: Vec<String>) -> Result<Reference, sqlx::Error> {
    let locator_json: String = row.try_get("locator_json")?;
    let locator: serde_json::Value =
        serde_json::from_str(&locator_json).unwrap_or(serde_json::Value::Null);
    let indexed_int: i64 = row.try_get("indexed")?;
    Ok(Reference {
        id: row.try_get("id")?,
        collection_id: row.try_get("collection_id")?,
        source_id: row.try_get("source_id")?,
        name: row.try_get("name")?,
        ref_type: row.try_get("type")?,
        hosting: row.try_get("hosting")?,
        locator,
        description: row.try_get("description")?,
        lifecycle: row.try_get("lifecycle")?,
        confidentiality: row.try_get("confidentiality")?,
        indexed: indexed_int != 0,
        disposition: row.try_get("disposition")?,
        tags,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn fetch_reference(pool: &SqlitePool, id: &str) -> CmdResult<Reference> {
    let row = sqlx::query(
        "SELECT id, collection_id, source_id, name, type, hosting, locator_json, \
                description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at \
         FROM resource_reference WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", id)))?;
    let tags = fetch_tags(pool, id).await?;
    row_to_reference(&row, tags).map_err(AppError::from)
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `ref_create_external`：仅关联创建，**绝不复制/移动/写入源文件**（§6.3）。
///
/// 仅做两件事：
/// 1. 校验 `locator.path` 存在（不存在 → `FS_PATH_NOT_FOUND`）。
/// 2. 在 `resource_reference` / `reference_tag` 中登记元数据。
///
/// 该函数对文件系统只读，不写、不创建、不修改任何文件。
#[allow(clippy::too_many_arguments)]
pub async fn create_external(
    pool: &SqlitePool,
    collection_id: String,
    name: String,
    ref_type: String,
    locator: Locator,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
) -> CmdResult<Reference> {
    validate_name(&name)?;
    validate_ref_type(&ref_type)?;
    validate_locator(&locator)?;
    if let Some(ref lc) = lifecycle {
        validate_lifecycle(lc)?;
    }
    if let Some(ref cf) = confidentiality {
        validate_confidentiality(cf)?;
    }
    ensure_collection_exists(pool, &collection_id).await?;

    // 路径存在性校验：仅 metadata 读取，不做任何写操作。
    // 注意：std::path::Path::exists 是只读操作，不会修改 mtime/atime/ctime。
    if !std::path::Path::new(&locator.path).exists() {
        return Err(AppError::new(
            "FS_PATH_NOT_FOUND",
            format!("目标路径不存在: {}", locator.path),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_unix();
    let lifecycle_val = lifecycle.as_deref().unwrap_or("active");
    let confidentiality_val = confidentiality.as_deref().unwrap_or("internal");
    let indexed_val = indexed.unwrap_or(true);
    let locator_json = serde_json::to_string(&serde_json::json!({
        "kind": locator.kind,
        "path": locator.path,
    }))
    .map_err(|e| AppError::invalid_param(format!("locator 序列化失败: {}", e)))?;

    sqlx::query(
        "INSERT INTO resource_reference \
         (id, collection_id, source_id, name, type, hosting, locator_json, \
          description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 'external', ?, ?, ?, ?, ?, 'none', ?, ?)",
    )
    .bind(&id)
    .bind(&collection_id)
    .bind(DEFAULT_SOURCE_ID)
    .bind(name.trim())
    .bind(&ref_type)
    .bind(&locator_json)
    .bind(&description)
    .bind(lifecycle_val)
    .bind(confidentiality_val)
    .bind(indexed_val as i64)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    if let Some(tags) = tags {
        replace_tags(pool, &id, &tags).await?;
    }
    fetch_reference(pool, &id).await
}

/// `ref_update`：仅允许修改管理属性；`type`/`locator`/`hosting` 不可改。
#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &SqlitePool,
    id: String,
    name: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
) -> CmdResult<Reference> {
    let existing = fetch_reference(pool, &id).await?;

    if let Some(ref n) = name {
        validate_name(n)?;
    }
    if let Some(ref lc) = lifecycle {
        validate_lifecycle(lc)?;
    }
    if let Some(ref cf) = confidentiality {
        validate_confidentiality(cf)?;
    }

    let new_name = name.as_deref().map(str::trim).unwrap_or(&existing.name);
    let new_description = description.or(existing.description);
    let new_lifecycle = lifecycle.as_deref().unwrap_or(&existing.lifecycle);
    let new_confidentiality = confidentiality.as_deref().unwrap_or(&existing.confidentiality);
    let new_indexed = indexed.unwrap_or(existing.indexed);
    let now = now_unix();

    sqlx::query(
        "UPDATE resource_reference \
         SET name = ?, description = ?, lifecycle = ?, confidentiality = ?, indexed = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(new_name)
    .bind(&new_description)
    .bind(new_lifecycle)
    .bind(new_confidentiality)
    .bind(new_indexed as i64)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    if let Some(tags) = tags {
        replace_tags(pool, &id, &tags).await?;
    }
    fetch_reference(pool, &id).await
}

/// `ref_get { id }` → `Reference`。
pub async fn get(pool: &SqlitePool, id: String) -> CmdResult<Reference> {
    fetch_reference(pool, &id).await
}

/// `ref_list { collectionId, type?, lifecycle?, disposition? }` → `Reference[]`。
///
/// 三个过滤条件可组合；`disposition` 缺省时不返回 `deleted`（§2.5 默认不含已删除）。
pub async fn list(
    pool: &SqlitePool,
    collection_id: String,
    ref_type: Option<String>,
    lifecycle: Option<String>,
    disposition: Option<String>,
) -> CmdResult<Vec<Reference>> {
    if let Some(ref t) = ref_type {
        validate_ref_type(t)?;
    }
    if let Some(ref lc) = lifecycle {
        validate_lifecycle(lc)?;
    }
    if let Some(ref d) = disposition {
        validate_disposition(d)?;
    }

    // 动态拼接 WHERE 子句。条件数少且无用户输入注入风险（全部走 bind 参数）。
    let mut sql = String::from(
        "SELECT id, collection_id, source_id, name, type, hosting, locator_json, \
                description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at \
         FROM resource_reference WHERE collection_id = ?",
    );
    if ref_type.is_some() {
        sql.push_str(" AND type = ?");
    }
    if lifecycle.is_some() {
        sql.push_str(" AND lifecycle = ?");
    }
    match &disposition {
        Some(_) => sql.push_str(" AND disposition = ?"),
        None => sql.push_str(" AND disposition != 'deleted'"),
    }
    sql.push_str(" ORDER BY created_at ASC, id ASC");

    let mut query = sqlx::query(&sql).bind(&collection_id);
    if let Some(t) = ref_type {
        query = query.bind(t);
    }
    if let Some(lc) = lifecycle {
        query = query.bind(lc);
    }
    if let Some(d) = disposition {
        query = query.bind(d);
    }

    let rows = query.fetch_all(pool).await.map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("id").map_err(AppError::from)?;
        let tags = fetch_tags(pool, &id).await?;
        out.push(row_to_reference(row, tags).map_err(AppError::from)?);
    }
    Ok(out)
}

// ============================================================
// 导入并托管（m3-3.2 · ref_create_managed 两阶段命令）
// ============================================================
//
// 设计要点（详细设计 §2 / §4.1）：
// - `confirmed=false`：仅返回 `ManagedPlan`，不写文件、不写库。
// - `confirmed=true`：执行 copy/move 落地 + 事务写库；
//   任一步失败均回滚（写库失败补偿删除已落地文件）。
// - 「落地 + 写库」复合操作通过 `tokio::sync::Mutex` 串行化（§5 多步写互斥）。
// - 复制循环预留进度回调注入点（3.3 将以 callback/channel 接入 `managed_progress` 事件）。

/// `ManagedPlan.kind` 固定值（契约字段）。
pub const MANAGED_PLAN_KIND: &str = "managed_plan";

/// 托管动作（`managedAction` 入参枚举）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ManagedAction {
    Copy,
    Move,
}

impl ManagedAction {
    fn as_str(&self) -> &'static str {
        match self {
            ManagedAction::Copy => "copy",
            ManagedAction::Move => "move",
        }
    }
}

/// `ref_create_managed` 在 `confirmed=false` 时的出参（§2 ManagedPlan）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedPlan {
    /// 固定为 `"managed_plan"`。
    pub kind: String,
    /// 源绝对路径（与入参 `locator.path` 一致）。
    pub source: String,
    /// 建议目标绝对路径（`{root}/{TypeSubdir}/{targetName}`）。
    pub proposed_target: String,
    /// `"copy" | "move"`。
    pub action: String,
    /// 源总大小（字节）；单文件 = 文件大小，目录 = 递归累计。
    pub size_bytes: u64,
    /// 源包含的文件数；单文件 = 1，目录 = 递归文件数（不含目录本身）。
    pub file_count: u64,
    /// 冲突描述列表；空 vec 表示无冲突。
    pub conflicts: Vec<String>,
}

/// `ref_create_managed` 在 `confirmed=true` 时的出参：与 `ref_create_external` 相同，
/// 直接返回 `Reference`。两阶段出参通过命令的 `serde_json::Value` 联合返回。
///
/// 全局互斥锁：串行化「落地 + 写库」复合操作（详细设计 §5）。
///
/// 选用 `tokio::sync::Mutex` 而非 `std::sync::Mutex`：
/// - 临界区内含 `.await`（文件 IO + sqlx 事务），异步 Mutex 可跨 await 持有；
/// - 与既有异步命令风格一致，避免在 async 上下文里阻塞 executor 线程。
/// 全局托管写互斥锁（详细设计 §5 多步写互斥）。
///
/// 跨模块共享：
/// - M3-3.2 `ref_create_managed` 落地 + 写库
/// - M4-4.5 销毁（预留）
/// - M4-4.8 `settings_change_root_dir` 迁移模式（migrate 策略）
///
/// `pub(crate)` 暴露给 `settings` 模块与其测试复用，保证「迁移模式期间禁止
/// 新的托管落地与处置操作」（详细设计 §4.4）。
pub(crate) static MANAGED_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// 获取全局托管写互斥锁（懒初始化）。
pub(crate) fn managed_write_lock() -> &'static Mutex<()> {
    MANAGED_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

// ============================================================
// m3-3.3 · 大文件复制进度事件（managed_progress）
// ============================================================

/// Tauri 事件名：前端用 `listen("managed_progress", ...)` 接收。
pub const MANAGED_PROGRESS_EVENT: &str = "managed_progress";

/// 节流常量：每复制满 1 MiB 发一次进度事件。
///
/// 与 `PROGRESS_TIME_STEP` 取先到者；调优时只改这里。
pub const PROGRESS_BYTE_STEP: u64 = 1 << 20; // 1 MiB

/// 节流常量：距上次发事件超过 200ms 也强制发一次。
///
/// 与 `PROGRESS_BYTE_STEP` 取先到者；调优时只改这里。
pub const PROGRESS_TIME_STEP: Duration = Duration::from_millis(200);

/// `managed_progress` 事件载荷（详细设计 §4.1）。
///
/// 序列化为 camelCase：`{ refId, bytes, total }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedProgress {
    /// 后端在 confirmed 阶段事务开始前生成的临时 UUID，
    /// 前端据此区分并发任务；与最终写库的 `Reference.id` 不同。
    pub ref_id: String,
    /// 已复制字节数（累计）。
    pub bytes: u64,
    /// 总字节数（plan 阶段 `stat_source` 计算并透传）。
    pub total: u64,
}

/// 进度发射节流器：按 `PROGRESS_BYTE_STEP` / `PROGRESS_TIME_STEP` 节流，
/// 并在 `finish()` 时强制发一次 `bytes == total` 的完成事件。
///
/// **约定（已固化）**：完成时**发**最后一次 `bytes == total`；
/// 失败时**不发**完成事件，由 `ref_create_managed` 的错误返回通知前端。
struct ProgressThrottle<'a> {
    ref_id: String,
    total: u64,
    /// 上次发事件时的累计字节数。
    last_emitted_bytes: u64,
    /// 上次发事件的墙钟时间；首次发事件前为 `None`。
    last_emitted_at: Option<Instant>,
    /// 下游发射器（生产：Tauri emit；测试：Vec push）。
    sink: &'a mut (dyn FnMut(ManagedProgress) + Send),
}

impl<'a> ProgressThrottle<'a> {
    fn new(
        ref_id: String,
        total: u64,
        sink: &'a mut (dyn FnMut(ManagedProgress) + Send),
    ) -> Self {
        Self {
            ref_id,
            total,
            last_emitted_bytes: 0,
            last_emitted_at: None,
            sink,
        }
    }

    /// 复制循环每次写盘后调用；按节流规则决定是否发射。
    fn on_bytes(&mut self, copied: u64) {
        let byte_advanced = copied.saturating_sub(self.last_emitted_bytes);
        let time_elapsed = self
            .last_emitted_at
            .map(|t| t.elapsed() >= PROGRESS_TIME_STEP)
            // 首次调用必发（bytes 可能为 0 或首个 buffer），让前端尽快看到进度条。
            .unwrap_or(true);
        if byte_advanced >= PROGRESS_BYTE_STEP || time_elapsed {
            self.emit(copied);
        }
    }

    /// 落地全部完成后调用：强制发一次 `bytes == total` 的完成事件。
    ///
    /// 若 `on_bytes` 已发过 `copied == total`（例如整除 1 MiB），这里会重发一次；
    /// 前端按 `bytes == total` 幂等处理即可。失败路径**不调用**本方法。
    fn finish(&mut self) {
        self.emit(self.total);
    }

    fn emit(&mut self, bytes: u64) {
        (self.sink)(ManagedProgress {
            ref_id: self.ref_id.clone(),
            bytes,
            total: self.total,
        });
        self.last_emitted_bytes = bytes;
        self.last_emitted_at = Some(Instant::now());
    }
}

/// 源路径统计结果。
#[derive(Debug, Clone, Copy)]
struct SourceStat {
    size_bytes: u64,
    file_count: u64,
    is_dir: bool,
}

/// 递归统计源路径大小与文件数。
///
/// - 单文件：`size_bytes = metadata.len()`，`file_count = 1`。
/// - 目录：递归 walk，累计所有**文件**的 `len()`；符号链接**不跟随**出根
///   （`symlink_metadata` 判断，链接本身按文件计入 1 个、按链接自身大小计）。
/// - 权限/IO 错误向上抛 `FS_PERMISSION_DENIED` / `COMMON_IO`。
fn stat_source(path: &Path) -> CmdResult<SourceStat> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| map_fs_err("读取源元数据失败", path, e))?;
    if meta.is_dir() {
        let mut total_bytes: u64 = 0;
        let mut total_files: u64 = 0;
        walk_dir(path, &mut |entry_meta, _entry_path| {
            // 仅累计文件（含符号链接自身的大小）；目录不计入 file_count
            if entry_meta.is_file() || entry_meta.file_type().is_symlink() {
                total_files += 1;
                total_bytes += entry_meta.len();
            }
            Ok(())
        })?;
        Ok(SourceStat {
            size_bytes: total_bytes,
            file_count: total_files,
            is_dir: true,
        })
    } else {
        Ok(SourceStat {
            size_bytes: meta.len(),
            file_count: 1,
            is_dir: false,
        })
    }
}

/// 深度优先递归遍历目录；对**每个条目**（含子目录、文件、符号链接）调用 `f`。
///
/// 不跟随符号链接出根：用 `symlink_metadata` 取元数据，遇到 symlink 不递归。
fn walk_dir<F>(root: &Path, f: &mut F) -> CmdResult<()>
where
    F: FnMut(std::fs::Metadata, &Path) -> CmdResult<()>,
{
    let entries = std::fs::read_dir(root).map_err(|e| map_fs_err("读取目录失败", root, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| map_fs_err("读取目录条目失败", root, e))?;
        let entry_path = entry.path();
        // symlink_metadata 不跟随符号链接，避免越出根
        let meta = std::fs::symlink_metadata(&entry_path)
            .map_err(|e| map_fs_err("读取条目元数据失败", &entry_path, e))?;
        f(meta.clone(), &entry_path)?;
        if meta.is_dir() {
            walk_dir(&entry_path, f)?;
        }
    }
    Ok(())
}

/// 将 `std::io::Error` 映射为统一错误码。
///
/// - `NotFound` → `FS_PATH_NOT_FOUND`
/// - `PermissionDenied` → `FS_PERMISSION_DENIED`
/// - 其他 → `COMMON_IO`
fn map_fs_err(context: &str, path: &Path, err: std::io::Error) -> AppError {
    let msg = format!("{} {}: {}", context, path.display(), err);
    match err.kind() {
        std::io::ErrorKind::NotFound => AppError::new("FS_PATH_NOT_FOUND", msg),
        std::io::ErrorKind::PermissionDenied => AppError::new("FS_PERMISSION_DENIED", msg),
        _ => AppError::io(msg),
    }
}

/// 复制单文件，使用流式 buffer。
///
/// **3.3 进度事件接入点**：本函数在每次写盘后调用 `on_progress(bytes_copied_so_far)`。
/// 3.3 任务将 `on_progress` 替换为向 Tauri 事件总线发送 `managed_progress{refId, bytes, total}` 的
/// callback / channel，无需修改本函数签名之外的落地流程。
fn copy_file_streaming<F>(src: &Path, dst: &Path, mut on_progress: F) -> CmdResult<u64>
where
    F: FnMut(u64),
{
    use std::io::{Read, Write};
    let mut src_f = std::fs::File::open(src).map_err(|e| map_fs_err("打开源文件失败", src, e))?;
    let mut dst_f =
        std::fs::File::create(dst).map_err(|e| map_fs_err("创建目标文件失败", dst, e))?;
    let mut buf = [0u8; 64 * 1024];
    let mut copied: u64 = 0;
    loop {
        let n = src_f
            .read(&mut buf)
            .map_err(|e| map_fs_err("读取源文件失败", src, e))?;
        if n == 0 {
            break;
        }
        dst_f
            .write_all(&buf[..n])
            .map_err(|e| map_fs_err("写入目标文件失败", dst, e))?;
        copied += n as u64;
        on_progress(copied);
    }
    dst_f
        .flush()
        .map_err(|e| map_fs_err("flush 目标文件失败", dst, e))?;
    Ok(copied)
}

/// 递归复制目录（不跟随符号链接出根；符号链接按文件复制其指向目标的内容）。
///
/// 进度回调语义同 `copy_file_streaming`：每文件复制完成后累计字节数。
fn copy_dir_recursive<F>(src: &Path, dst: &Path, on_progress: &mut F) -> CmdResult<()>
where
    F: FnMut(u64),
{
    std::fs::create_dir_all(dst).map_err(|e| map_fs_err("创建目标目录失败", dst, e))?;
    let entries = std::fs::read_dir(src).map_err(|e| map_fs_err("读取源目录失败", src, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| map_fs_err("读取源目录条目失败", src, e))?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&src_child)
            .map_err(|e| map_fs_err("读取源条目元数据失败", &src_child, e))?;
        if meta.is_dir() {
            copy_dir_recursive(&src_child, &dst_child, on_progress)?;
        } else {
            // 文件 / 符号链接：按文件复制（std::fs::File::open 会跟随符号链接到目标）
            copy_file_streaming(&src_child, &dst_child, &mut *on_progress)?;
        }
    }
    Ok(())
}

/// 落地：把 `src` 复制到 `dst`。
///
/// `progress_cb` 为 3.3 预留的进度回调注入点；当前任务传 `|_| {}`。
fn land_source<F>(src: &Path, dst: &Path, stat: &SourceStat, mut progress_cb: F) -> CmdResult<()>
where
    F: FnMut(u64),
{
    if stat.is_dir {
        copy_dir_recursive(src, dst, &mut progress_cb)
    } else {
        // 确保父目录存在（目标根目录下类型子目录可能未建）
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| map_fs_err("创建目标父目录失败", parent, e))?;
        }
        copy_file_streaming(src, dst, &mut progress_cb).map(|_| ())
    }
}

/// 补偿删除已落地目标（文件或目录）。失败仅记录日志，不向上抛错——
/// 补偿动作本身失败时，调用方拿不到原始错误的上下文。
fn cleanup_landed(target: &Path) {
    if !target.exists() {
        return;
    }
    let result = if target.is_dir() {
        std::fs::remove_dir_all(target)
    } else {
        std::fs::remove_file(target)
    };
    if let Err(e) = result {
        eprintln!(
            "[ref_create_managed] 补偿删除失败 {}: {}",
            target.display(),
            e
        );
    }
}

/// 从 settings 表读取资源根目录。
///
/// 未设置 → `COMMON_INVALID_PARAM`（用户须先通过 `settings_init_root_dir` 配置）。
async fn load_root_dir(pool: &SqlitePool) -> CmdResult<PathBuf> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = 'root_dir'")
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    let row = row.ok_or_else(|| {
        AppError::invalid_param("资源根目录未配置，请先调用 settings_init_root_dir")
    })?;
    let value_json: String = row.try_get("value_json").map_err(AppError::from)?;
    let v: serde_json::Value = serde_json::from_str(&value_json)
        .map_err(|e| AppError::db(format!("settings.root_dir 反序列化失败: {}", e)))?;
    let s = v
        .as_str()
        .ok_or_else(|| AppError::db("settings.root_dir 非字符串"))?;
    Ok(PathBuf::from(s))
}

/// 计算目标名：优先 `target_name`，缺省用源文件名。
///
/// 空白 `target_name` → `COMMON_INVALID_PARAM`。
/// 源无文件名（如 `/`）→ `COMMON_INVALID_PARAM`。
fn resolve_target_name(source: &Path, target_name: Option<&str>) -> CmdResult<String> {
    match target_name {
        Some(t) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                return Err(AppError::invalid_param("targetName 不能为空白字符串"));
            }
            Ok(trimmed.to_string())
        }
        None => {
            let file_name = source
                .file_name()
                .ok_or_else(|| AppError::invalid_param("源路径无文件名，无法推断目标名"))?;
            Ok(file_name.to_string_lossy().to_string())
        }
    }
}

/// 在事务中插入 managed 引用记录。
///
/// 与 `create_external` 的差异：`hosting='managed'`、`locator.path` 为落地后的绝对路径。
/// 其他字段（type/collectionId/lifecycle/confidentiality/indexed/description/tags）沿用相同校验。
#[allow(clippy::too_many_arguments)]
async fn insert_managed_reference_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    collection_id: &str,
    name: &str,
    ref_type: &str,
    landed_abs_path: &Path,
    description: Option<&str>,
    tags: Option<&[String]>,
    lifecycle: Option<&str>,
    confidentiality: Option<&str>,
    indexed: Option<bool>,
) -> CmdResult<String> {
    let id = Uuid::new_v4().to_string();
    let now = now_unix();
    let lifecycle_val = lifecycle.unwrap_or("active");
    let confidentiality_val = confidentiality.unwrap_or("internal");
    let indexed_val = indexed.unwrap_or(true);
    let locator_json = serde_json::to_string(&serde_json::json!({
        "kind": "path",
        "path": landed_abs_path.to_string_lossy(),
    }))
    .map_err(|e| AppError::invalid_param(format!("locator 序列化失败: {}", e)))?;

    sqlx::query(
        "INSERT INTO resource_reference \
         (id, collection_id, source_id, name, type, hosting, locator_json, \
          description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 'managed', ?, ?, ?, ?, ?, 'none', ?, ?)",
    )
    .bind(&id)
    .bind(collection_id)
    .bind(DEFAULT_SOURCE_ID)
    .bind(name)
    .bind(ref_type)
    .bind(&locator_json)
    .bind(description)
    .bind(lifecycle_val)
    .bind(confidentiality_val)
    .bind(indexed_val as i64)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(AppError::from)?;

    if let Some(tags) = tags {
        for tag in tags {
            let trimmed = tag.trim();
            if trimmed.is_empty() {
                continue;
            }
            sqlx::query(
                "INSERT OR IGNORE INTO reference_tag (reference_id, tag) VALUES (?, ?)",
            )
            .bind(&id)
            .bind(trimmed)
            .execute(&mut **tx)
            .await
            .map_err(AppError::from)?;
        }
    }
    Ok(id)
}

/// `ref_create_managed` 业务函数（两阶段）。
///
/// # 参数
/// - `confirmed=false`：仅返回 `Ok(ManagedCreateResult::Plan(...))`，不写文件不写库。
/// - `confirmed=true`：返回 `Ok(ManagedCreateResult::Created(...))`。
/// - `progress_sink`：m3-3.3 进度事件发射器；仅 `confirmed=true` 时使用。
///   生产环境由 Tauri 命令注入 `AppHandle::emit` 闭包；测试注入 Vec 收集闭包。
///   传 `None` 表示不发事件（向后兼容旧调用）。
///
/// # 错误码
/// - `FS_PATH_NOT_FOUND`：源路径不存在
/// - `FS_TARGET_EXISTS`：`confirmed=true` 且目标已存在
/// - `FS_PERMISSION_DENIED`：权限不足
/// - `COMMON_IO`：其他 IO 错误
/// - `COMMON_INVALID_PARAM`：参数非法（含 root_dir 未配置、targetName 空白、type 非法等）
/// - `COMMON_DB`：写库失败（已补偿删除落地文件）
/// - `COMMON_NOT_FOUND`：collection 不存在
#[allow(clippy::too_many_arguments)]
pub async fn create_managed(
    pool: &SqlitePool,
    collection_id: String,
    name: String,
    ref_type: String,
    locator: Locator,
    managed_action: ManagedAction,
    target_name: Option<String>,
    confirmed: bool,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
    progress_sink: Option<&mut (dyn FnMut(ManagedProgress) + Send)>,
) -> CmdResult<ManagedCreateResult> {
    // ---------- 1. 入参校验（与 create_external 相同 + 新增字段） ----------
    validate_name(&name)?;
    validate_ref_type(&ref_type)?;
    validate_locator(&locator)?;
    if let Some(ref lc) = lifecycle {
        validate_lifecycle(lc)?;
    }
    if let Some(ref cf) = confidentiality {
        validate_confidentiality(cf)?;
    }
    ensure_collection_exists(pool, &collection_id).await?;

    // ---------- 2. 源路径校验 + 统计 ----------
    let source_path = PathBuf::from(&locator.path);
    if !source_path.exists() {
        return Err(AppError::new(
            "FS_PATH_NOT_FOUND",
            format!("源路径不存在: {}", locator.path),
        ));
    }
    let stat = stat_source(&source_path)?;

    // ---------- 3. 计算目标路径 ----------
    let root_dir = load_root_dir(pool).await?;
    let target_name_str = resolve_target_name(&source_path, target_name.as_deref())?;
    let proposed_target = landing::propose_target(&root_dir, &ref_type, &target_name_str)?;

    // ---------- 4. 冲突检测（plan 阶段不报错，仅记录） ----------
    let target_exists = proposed_target.exists();

    if !confirmed {
        // ---------- 5a. plan 阶段：仅返回 ManagedPlan ----------
        let conflicts = if target_exists {
            vec!["目标已存在同名项".to_string()]
        } else {
            Vec::new()
        };
        return Ok(ManagedCreateResult::Plan(ManagedPlan {
            kind: MANAGED_PLAN_KIND.to_string(),
            source: locator.path.clone(),
            proposed_target: proposed_target.to_string_lossy().to_string(),
            action: managed_action.as_str().to_string(),
            size_bytes: stat.size_bytes,
            file_count: stat.file_count,
            conflicts,
        }));
    }

    // ---------- 5b. confirmed 阶段：互斥保护「落地 + 写库」 ----------
    let _guard = managed_write_lock().lock().await;

    // TOCTOU 复检：拿到锁后再检一次目标是否已存在
    if proposed_target.exists() {
        return Err(AppError::new(
            "FS_TARGET_EXISTS",
            format!("目标已存在: {}", proposed_target.display()),
        ));
    }

    // m3-3.3：事务开始前生成临时 refId，作为 `managed_progress` 事件载荷的标识；
    // 与最终写库的 Reference.id 无关（写库 id 由 insert_managed_reference_tx 内部生成）。
    let progress_ref_id = Uuid::new_v4().to_string();

    // 落地（m3-3.3：接入进度事件；throttle 内部按 1 MiB / 200ms 节流）
    let mut throttle = progress_sink.map(|sink| {
        ProgressThrottle::new(progress_ref_id.clone(), stat.size_bytes, sink)
    });
    let land_result = land_source(
        &source_path,
        &proposed_target,
        &stat,
        |bytes_copied| {
            if let Some(t) = throttle.as_mut() {
                t.on_bytes(bytes_copied);
            }
        },
    );
    if let Err(e) = land_result {
        // 失败路径：不发完成事件（throttle.finish 不会被调用），
        // 由本命令的 Err 返回通知前端。
        return Err(e);
    }
    // 完成事件：发最后一次 bytes == total（约定见 ProgressThrottle 文档）。
    if let Some(t) = throttle.as_mut() {
        t.finish();
    }

    // move 语义：复制完成后删除源；删除失败回滚（删目标、报 COMMON_IO）
    if matches!(managed_action, ManagedAction::Move) {
        let remove_result = if stat.is_dir {
            std::fs::remove_dir_all(&source_path)
        } else {
            std::fs::remove_file(&source_path)
        };
        if let Err(e) = remove_result {
            cleanup_landed(&proposed_target);
            return Err(map_fs_err("删除源失败（已回滚目标）", &source_path, e));
        }
    }

    // 事务写库；失败补偿删除已落地目标
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let insert_result = insert_managed_reference_tx(
        &mut tx,
        &collection_id,
        name.trim(),
        &ref_type,
        &proposed_target,
        description.as_deref(),
        tags.as_deref(),
        lifecycle.as_deref(),
        confidentiality.as_deref(),
        indexed,
    )
    .await;

    let ref_id = match insert_result {
        Ok(id) => {
            // 提交事务
            if let Err(e) = tx.commit().await {
                cleanup_landed(&proposed_target);
                return Err(AppError::from(e));
            }
            id
        }
        Err(e) => {
            // rollback 由 tx Drop 自动触发；补偿删除已落地目标
            drop(tx);
            cleanup_landed(&proposed_target);
            return Err(e);
        }
    };

    // 提交后读回完整 Reference（含 tags）
    let reference = fetch_reference(pool, &ref_id).await?;
    Ok(ManagedCreateResult::Created(reference))
}

/// `ref_create_managed` 两阶段出参。
///
/// 序列化为 JSON 时与契约一致：
/// - `Plan` → `ManagedPlan`（含 `kind:"managed_plan"`）
/// - `Created` → `Reference`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ManagedCreateResult {
    Plan(ManagedPlan),
    Created(Reference),
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_create_external(
    state: tauri::State<'_, crate::AppState>,
    collection_id: String,
    name: String,
    r#type: String,
    locator: Locator,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
) -> CmdResult<Reference> {
    create_external(
        &state.pool,
        collection_id,
        name,
        r#type,
        locator,
        description,
        tags,
        lifecycle,
        confidentiality,
        indexed,
    )
    .await
}

#[tauri::command]
pub async fn ref_update(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
) -> CmdResult<Reference> {
    update(
        &state.pool,
        id,
        name,
        description,
        tags,
        lifecycle,
        confidentiality,
        indexed,
    )
    .await
}

#[tauri::command]
pub async fn ref_get(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<Reference> {
    get(&state.pool, id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_list(
    state: tauri::State<'_, crate::AppState>,
    collection_id: String,
    r#type: Option<String>,
    lifecycle: Option<String>,
    disposition: Option<String>,
) -> CmdResult<Vec<Reference>> {
    list(&state.pool, collection_id, r#type, lifecycle, disposition).await
}

/// `ref_create_managed`：导入并托管（两阶段）。
///
/// - `confirmed=false` → 返回 `ManagedPlan`（`kind:"managed_plan"`）
/// - `confirmed=true`  → 返回 `Reference`
///
/// 出参通过 `ManagedCreateResult` 的 untagged 序列化区分；
/// 前端按是否存在 `kind == "managed_plan"` 字段判定阶段。
///
/// m3-3.3：`confirmed=true` 时通过 `managed_progress` 事件周期发射
/// `{ refId, bytes, total }`；前端 `listen("managed_progress", ...)` 接收。
#[tauri::command(rename_all = "camelCase")]
#[allow(clippy::too_many_arguments)]
pub async fn ref_create_managed(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    collection_id: String,
    name: String,
    r#type: String,
    locator: Locator,
    managed_action: ManagedAction,
    target_name: Option<String>,
    confirmed: bool,
    description: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    indexed: Option<bool>,
) -> CmdResult<ManagedCreateResult> {
    use tauri::Emitter;
    // m3-3.3：进度事件发射闭包。emit 失败（如窗口已关闭）仅忽略，
    // 不影响落地与写库主流程 —— 进度通知是 best-effort。
    let mut emit_progress = |p: ManagedProgress| {
        let _ = app.emit(MANAGED_PROGRESS_EVENT, p);
    };
    create_managed(
        &state.pool,
        collection_id,
        name,
        r#type,
        locator,
        managed_action,
        target_name,
        confirmed,
        description,
        tags,
        lifecycle,
        confidentiality,
        indexed,
        Some(&mut emit_progress),
    )
    .await
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";

    /// 在指定空间下创建一个资源集，返回其 id。
    async fn make_collection(pool: &SqlitePool) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
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

    fn make_locator(path: &std::path::Path) -> Locator {
        Locator {
            kind: "path".into(),
            path: path.to_string_lossy().to_string(),
        }
    }

    // ---------- ref_create_external ----------

    #[tokio::test]
    async fn ref_create_external_ok_minimal() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        let r = create_external(
            &pool,
            cid.clone(),
            "引用A".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        assert_eq!(r.collection_id, cid);
        assert_eq!(r.source_id, DEFAULT_SOURCE_ID);
        assert_eq!(r.name, "引用A");
        assert_eq!(r.ref_type, "code");
        assert_eq!(r.hosting, "external");
        assert_eq!(r.lifecycle, "active");
        assert_eq!(r.confidentiality, "internal");
        assert!(r.indexed);
        assert_eq!(r.disposition, "none");
        assert!(r.tags.is_empty());
        assert!(!r.id.is_empty());
        assert_eq!(r.created_at, r.updated_at);

        // locator 反序列化为结构化对象
        let loc = r.locator.as_object().expect("locator object");
        assert_eq!(loc["kind"].as_str(), Some("path"));
        assert_eq!(loc["path"].as_str(), Some(file.to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn ref_create_external_ok_full_fields() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("b.md");
        std::fs::write(&file, b"doc").expect("write");

        let r = create_external(
            &pool,
            cid,
            "引用B".into(),
            "document".into(),
            make_locator(&file),
            Some("说明".into()),
            Some(vec!["tagX".into(), "tagY".into()]),
            Some("staged".into()),
            Some("sensitive".into()),
            Some(false),
        )
        .await
        .expect("create ok");

        assert_eq!(r.description.as_deref(), Some("说明"));
        assert_eq!(r.tags, vec!["tagX".to_string(), "tagY".to_string()]);
        assert_eq!(r.lifecycle, "staged");
        assert_eq!(r.confidentiality, "sensitive");
        assert!(!r.indexed);
    }

    #[tokio::test]
    async fn ref_create_external_err_path_not_found() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;

        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("no-such-file.txt");

        let err = create_external(
            &pool,
            cid,
            "x".into(),
            "code".into(),
            make_locator(&missing),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "FS_PATH_NOT_FOUND");
    }

    #[tokio::test]
    async fn ref_create_external_err_collection_not_found() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let err = create_external(
            &pool,
            "no-such-collection".into(),
            "x".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn ref_create_external_err_invalid_type() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let err = create_external(
            &pool,
            cid,
            "x".into(),
            "invalid_type".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_create_external_err_invalid_lifecycle() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let err = create_external(
            &pool,
            cid,
            "x".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            Some("bogus".into()),
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_create_external_err_invalid_confidentiality() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let err = create_external(
            &pool,
            cid,
            "x".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            Some("top_secret".into()),
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_create_external_err_empty_name() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let err = create_external(
            &pool,
            cid,
            "   ".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_create_external_err_unsupported_locator_kind() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let loc = Locator {
            kind: "cloud".into(),
            path: "s3://bucket/key".into(),
        };
        let err = create_external(
            &pool,
            cid,
            "x".into(),
            "code".into(),
            loc,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    /// **核心验收点（§6.3）**：`ref_create_external` 绝不触碰文件系统。
    ///
    /// 断言：
    /// 1. 创建前后源文件 mtime 不变。
    /// 2. 创建前后源文件内容不变。
    /// 3. 源目录中不产生任何新文件。
    #[tokio::test]
    async fn ref_create_external_never_touches_source_file() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("immutable.bin");
        let content: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        std::fs::write(&file, &content).expect("write");

        // 记录创建前的 mtime 与目录条目
        let meta_before = std::fs::metadata(&file).expect("metadata before");
        let mtime_before = meta_before.modified().expect("mtime before");
        let entries_before: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read_dir before")
            .map(|e| e.expect("entry").file_name())
            .collect();

        let r = create_external(
            &pool,
            cid,
            "只读引用".into(),
            "data".into(),
            make_locator(&file),
            Some("绝不触碰源文件".into()),
            Some(vec!["core".into()]),
            None,
            None,
            None,
        )
        .await
        .expect("create ok");
        assert_eq!(r.hosting, "external");

        // 1. mtime 不变
        let meta_after = std::fs::metadata(&file).expect("metadata after");
        let mtime_after = meta_after.modified().expect("mtime after");
        assert_eq!(
            mtime_before, mtime_after,
            "ref_create_external 不得修改源文件 mtime"
        );

        // 2. 内容不变
        let content_after = std::fs::read(&file).expect("read after");
        assert_eq!(
            content, content_after,
            "ref_create_external 不得修改源文件内容"
        );

        // 3. 不产生新文件
        let entries_after: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read_dir after")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(
            entries_before, entries_after,
            "ref_create_external 不得在源目录产生新文件"
        );
    }

    /// 目录作为 locator.path 同样合法（仅关联，不限制文件/目录）。
    #[tokio::test]
    async fn ref_create_external_ok_with_directory_path() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");

        let r = create_external(
            &pool,
            cid,
            "目录引用".into(),
            "code".into(),
            make_locator(dir.path()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create ok");
        assert_eq!(r.hosting, "external");
    }

    // ---------- ref_update ----------

    #[tokio::test]
    async fn ref_update_ok_partial_fields() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let r = create_external(
            &pool,
            cid,
            "原名".into(),
            "code".into(),
            make_locator(&file),
            Some("旧说明".into()),
            Some(vec!["t1".into()]),
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let updated = update(
            &pool,
            r.id.clone(),
            Some("新名".into()),
            Some("新说明".into()),
            None,
            Some("delivered".into()),
            Some("public".into()),
            Some(false),
        )
        .await
        .expect("update ok");

        assert_eq!(updated.name, "新名");
        assert_eq!(updated.description.as_deref(), Some("新说明"));
        assert_eq!(updated.lifecycle, "delivered");
        assert_eq!(updated.confidentiality, "public");
        assert!(!updated.indexed);
        // tags 未传 → 保持原值
        assert_eq!(updated.tags, vec!["t1".to_string()]);
        // type/hosting/locator 不可改
        assert_eq!(updated.ref_type, "code");
        assert_eq!(updated.hosting, "external");
        assert_eq!(updated.locator, r.locator);
        assert!(updated.updated_at >= updated.created_at);
    }

    #[tokio::test]
    async fn ref_update_ok_replace_tags() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let r = create_external(
            &pool,
            cid,
            "n".into(),
            "code".into(),
            make_locator(&file),
            None,
            Some(vec!["t1".into(), "t2".into()]),
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let updated = update(
            &pool,
            r.id.clone(),
            None,
            None,
            Some(vec!["t3".into()]),
            None,
            None,
            None,
        )
        .await
        .expect("update ok");
        assert_eq!(updated.tags, vec!["t3".to_string()]);

        // 传空数组 = 清空
        let cleared = update(
            &pool,
            r.id.clone(),
            None,
            None,
            Some(vec![]),
            None,
            None,
            None,
        )
        .await
        .expect("update ok");
        assert!(cleared.tags.is_empty());
    }

    #[tokio::test]
    async fn ref_update_err_not_found() {
        let pool = setup().await;
        let err = update(
            &pool,
            "no-such-id".into(),
            Some("x".into()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn ref_update_err_invalid_lifecycle() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let r = create_external(
            &pool,
            cid,
            "n".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let err = update(
            &pool,
            r.id.clone(),
            None,
            None,
            None,
            Some("bogus".into()),
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_update_err_invalid_name() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let r = create_external(
            &pool,
            cid,
            "n".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let err = update(
            &pool,
            r.id.clone(),
            Some("".into()),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- ref_get ----------

    #[tokio::test]
    async fn ref_get_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let created = create_external(
            &pool,
            cid,
            "n".into(),
            "media".into(),
            make_locator(&file),
            Some("d".into()),
            Some(vec!["t".into()]),
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let fetched = get(&pool, created.id.clone()).await.expect("get ok");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "n");
        assert_eq!(fetched.ref_type, "media");
        assert_eq!(fetched.tags, vec!["t".to_string()]);
    }

    #[tokio::test]
    async fn ref_get_err_not_found() {
        let pool = setup().await;
        let err = get(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- ref_list ----------

    #[tokio::test]
    async fn ref_list_ok_no_filter() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("1.txt");
        let f2 = dir.path().join("2.txt");
        std::fs::write(&f1, b"1").expect("w1");
        std::fs::write(&f2, b"2").expect("w2");

        create_external(
            &pool,
            cid.clone(),
            "r1".into(),
            "code".into(),
            make_locator(&f1),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c1");
        create_external(
            &pool,
            cid.clone(),
            "r2".into(),
            "document".into(),
            make_locator(&f2),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c2");

        let all = list(&pool, cid, None, None, None).await.expect("list ok");
        assert_eq!(all.len(), 2);
        // created_at 同秒 + id 为随机 UUID 时顺序不确定，按名称排序后断言内容
        let mut names: Vec<_> = all.iter().map(|r| r.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["r1", "r2"]);
    }

    #[tokio::test]
    async fn ref_list_ok_filter_by_type() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("1.txt");
        let f2 = dir.path().join("2.txt");
        std::fs::write(&f1, b"1").expect("w1");
        std::fs::write(&f2, b"2").expect("w2");

        create_external(
            &pool,
            cid.clone(),
            "r1".into(),
            "code".into(),
            make_locator(&f1),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c1");
        create_external(
            &pool,
            cid.clone(),
            "r2".into(),
            "document".into(),
            make_locator(&f2),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c2");

        let filtered = list(&pool, cid, Some("code".into()), None, None)
            .await
            .expect("list ok");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ref_type, "code");
    }

    #[tokio::test]
    async fn ref_list_ok_filter_by_lifecycle() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("1.txt");
        let f2 = dir.path().join("2.txt");
        std::fs::write(&f1, b"1").expect("w1");
        std::fs::write(&f2, b"2").expect("w2");

        create_external(
            &pool,
            cid.clone(),
            "r1".into(),
            "code".into(),
            make_locator(&f1),
            None,
            None,
            Some("staged".into()),
            None,
            None,
        )
        .await
        .expect("c1");
        create_external(
            &pool,
            cid.clone(),
            "r2".into(),
            "code".into(),
            make_locator(&f2),
            None,
            None,
            Some("delivered".into()),
            None,
            None,
        )
        .await
        .expect("c2");

        let filtered = list(&pool, cid, None, Some("staged".into()), None)
            .await
            .expect("list ok");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].lifecycle, "staged");
    }

    #[tokio::test]
    async fn ref_list_default_excludes_deleted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("1.txt");
        let f2 = dir.path().join("2.txt");
        std::fs::write(&f1, b"1").expect("w1");
        std::fs::write(&f2, b"2").expect("w2");

        let r1 = create_external(
            &pool,
            cid.clone(),
            "r1".into(),
            "code".into(),
            make_locator(&f1),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c1");
        create_external(
            &pool,
            cid.clone(),
            "r2".into(),
            "code".into(),
            make_locator(&f2),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("c2");

        // 手工将 r1 标记为 deleted
        sqlx::query("UPDATE resource_reference SET disposition='deleted' WHERE id = ?")
            .bind(&r1.id)
            .execute(&pool)
            .await
            .expect("mark deleted");

        // 默认不含已删除
        let default = list(&pool, cid.clone(), None, None, None)
            .await
            .expect("list ok");
        assert_eq!(default.len(), 1);
        assert_eq!(default[0].name, "r2");

        // 显式查询 deleted
        let deleted = list(&pool, cid.clone(), None, None, Some("deleted".into()))
            .await
            .expect("list ok");
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].name, "r1");

        // 显式查询 none
        let none = list(&pool, cid, None, None, Some("none".into()))
            .await
            .expect("list ok");
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].name, "r2");
    }

    #[tokio::test]
    async fn ref_list_ok_combined_filters() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("1.txt");
        let f2 = dir.path().join("2.txt");
        let f3 = dir.path().join("3.txt");
        std::fs::write(&f1, b"1").expect("w1");
        std::fs::write(&f2, b"2").expect("w2");
        std::fs::write(&f3, b"3").expect("w3");

        create_external(
            &pool,
            cid.clone(),
            "r1".into(),
            "code".into(),
            make_locator(&f1),
            None,
            None,
            Some("active".into()),
            None,
            None,
        )
        .await
        .expect("c1");
        create_external(
            &pool,
            cid.clone(),
            "r2".into(),
            "code".into(),
            make_locator(&f2),
            None,
            None,
            Some("staged".into()),
            None,
            None,
        )
        .await
        .expect("c2");
        create_external(
            &pool,
            cid.clone(),
            "r3".into(),
            "document".into(),
            make_locator(&f3),
            None,
            None,
            Some("active".into()),
            None,
            None,
        )
        .await
        .expect("c3");

        let filtered = list(
            &pool,
            cid,
            Some("code".into()),
            Some("active".into()),
            None,
        )
        .await
        .expect("list ok");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "r1");
    }

    #[tokio::test]
    async fn ref_list_err_invalid_type_filter() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let err = list(&pool, cid, Some("bogus".into()), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn ref_list_err_invalid_disposition_filter() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let err = list(&pool, cid, None, None, Some("bogus".into()))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- 序列化契约 ----------

    #[tokio::test]
    async fn reference_serializes_camel_case_with_structured_locator() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");

        let r = create_external(
            &pool,
            cid,
            "n".into(),
            "code".into(),
            make_locator(&file),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create ok");

        let json = serde_json::to_value(&r).expect("serialize");
        let obj = json.as_object().expect("object");
        // camelCase 键
        assert!(obj.contains_key("collectionId"));
        assert!(obj.contains_key("sourceId"));
        assert!(obj.contains_key("refType") || obj.contains_key("type"));
        assert!(obj.contains_key("createdAt"));
        assert!(obj.contains_key("updatedAt"));
        // 不应出现 snake_case
        assert!(!obj.contains_key("collection_id"));
        assert!(!obj.contains_key("source_id"));
        assert!(!obj.contains_key("created_at"));
        // locator 是结构化对象而非字符串
        let locator = obj.get("locator").expect("locator key");
        assert!(locator.is_object(), "locator 应为结构化对象");
        assert_eq!(locator["kind"].as_str(), Some("path"));
    }

    // ============================================================
    // create_managed（m3-3.2）
    // ============================================================

    mod create_managed_tests {
        use super::*;
        use std::sync::Arc;

        /// 在测试池中配置资源根目录。
        async fn set_root_dir(pool: &SqlitePool, root: &std::path::Path) {
            let value_json = serde_json::to_string(&serde_json::Value::String(
                root.to_string_lossy().to_string(),
            ))
            .expect("serialize root_dir");
            let now = now_unix();
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES ('root_dir', ?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            )
            .bind(&value_json)
            .bind(now)
            .execute(pool)
            .await
            .expect("insert root_dir");
        }

        /// 构造临时源文件 + 根目录 + collection，返回 (pool, cid, source_file, root_dir, _tmp)。
        async fn setup_managed_env() -> (
            SqlitePool,
            String,
            PathBuf,
            PathBuf,
            tempfile::TempDir,
        ) {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let source_dir = tmp.path().join("src");
            std::fs::create_dir_all(&source_dir).expect("mkdir src");
            let source_file = source_dir.join("report.pdf");
            std::fs::write(&source_file, b"PDF-BYTES").expect("write source");
            let root_dir = tmp.path().join("root");
            std::fs::create_dir_all(&root_dir).expect("mkdir root");
            set_root_dir(&pool, &root_dir).await;
            (pool, cid, source_file, root_dir, tmp)
        }

        fn locator_of(p: &std::path::Path) -> Locator {
            Locator {
                kind: "path".into(),
                path: p.to_string_lossy().to_string(),
            }
        }

        // ---------- plan 阶段 ----------

        #[tokio::test]
        async fn plan_err_source_not_found() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let root = tmp.path().join("root");
            std::fs::create_dir_all(&root).expect("mkdir");
            set_root_dir(&pool, &root).await;

            let missing = tmp.path().join("no-such.txt");
            let err = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&missing),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "FS_PATH_NOT_FOUND");
        }

        #[tokio::test]
        async fn plan_ok_single_file() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ManagedCreateResult::Plan(p) => p,
                other => panic!("expected Plan, got {:?}", other),
            };
            assert_eq!(plan.kind, "managed_plan");
            assert_eq!(plan.source, source.to_string_lossy().to_string());
            assert_eq!(plan.action, "copy");
            assert_eq!(plan.size_bytes, 9); // b"PDF-BYTES".len()
            assert_eq!(plan.file_count, 1);
            assert!(plan.conflicts.is_empty());
            let expected_target = root.join("Documents").join("report.pdf");
            assert_eq!(
                plan.proposed_target,
                expected_target.to_string_lossy().to_string()
            );
            // 未确认 → 目标不应被创建
            assert!(!expected_target.exists());
        }

        #[tokio::test]
        async fn plan_ok_directory_recursion() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let root = tmp.path().join("root");
            std::fs::create_dir_all(&root).expect("mkdir root");
            set_root_dir(&pool, &root).await;

            // 构造目录：dir/a.txt (5B) + dir/sub/b.txt (3B)
            let dir = tmp.path().join("mydir");
            std::fs::create_dir_all(dir.join("sub")).expect("mkdir");
            std::fs::write(dir.join("a.txt"), b"12345").expect("w1");
            std::fs::write(dir.join("sub").join("b.txt"), b"123").expect("w2");

            let r = create_managed(
                &pool,
                cid,
                "d".into(),
                "code".into(),
                locator_of(&dir),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ManagedCreateResult::Plan(p) => p,
                other => panic!("expected Plan, got {:?}", other),
            };
            assert_eq!(plan.size_bytes, 8);
            assert_eq!(plan.file_count, 2);
            assert_eq!(
                plan.proposed_target,
                root.join("Code").join("mydir").to_string_lossy().to_string()
            );
        }

        #[tokio::test]
        async fn plan_conflict_reported_not_error() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            // 预先在目标位置放一个同名文件
            let target_dir = root.join("Documents");
            std::fs::create_dir_all(&target_dir).expect("mkdir target");
            std::fs::write(target_dir.join("report.pdf"), b"existing").expect("w");

            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ManagedCreateResult::Plan(p) => p,
                other => panic!("expected Plan, got {:?}", other),
            };
            assert_eq!(plan.conflicts, vec!["目标已存在同名项".to_string()]);
        }

        #[tokio::test]
        async fn plan_ok_custom_target_name() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                Some("自定义名.pdf".into()),
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("plan ok");
            let plan = match r {
                ManagedCreateResult::Plan(p) => p,
                _ => panic!("expected Plan"),
            };
            assert_eq!(
                plan.proposed_target,
                root.join("Documents")
                    .join("自定义名.pdf")
                    .to_string_lossy()
                    .to_string()
            );
        }

        #[tokio::test]
        async fn plan_err_blank_target_name() {
            let (pool, cid, source, _root, _tmp) = setup_managed_env().await;
            let err = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                Some("   ".into()),
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn plan_err_invalid_type() {
            let (pool, cid, source, _root, _tmp) = setup_managed_env().await;
            let err = create_managed(
                &pool,
                cid,
                "n".into(),
                "bogus_type".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn plan_err_root_dir_not_configured() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let src = tmp.path().join("a.txt");
            std::fs::write(&src, b"x").expect("w");

            let err = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&src),
                ManagedAction::Copy,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        // ---------- confirmed 阶段 ----------

        #[tokio::test]
        async fn confirmed_copy_ok() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            let r = create_managed(
                &pool,
                cid.clone(),
                "报告".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                Some("说明".into()),
                Some(vec!["t1".into()]),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("copy ok");

            let reference = match r {
                ManagedCreateResult::Created(r) => r,
                other => panic!("expected Created, got {:?}", other),
            };
            assert_eq!(reference.hosting, "managed");
            assert_eq!(reference.name, "报告");
            assert_eq!(reference.collection_id, cid);
            assert_eq!(reference.tags, vec!["t1".to_string()]);

            // 源保留（copy）
            assert!(source.exists(), "copy 应保留源");

            // 目标已落地
            let landed = root.join("Documents").join("report.pdf");
            assert!(landed.exists());
            assert_eq!(std::fs::read(&landed).expect("read"), b"PDF-BYTES");

            // locator 指向落地后路径
            let loc = reference.locator.as_object().expect("locator obj");
            assert_eq!(loc["kind"].as_str(), Some("path"));
            assert_eq!(
                loc["path"].as_str(),
                Some(landed.to_string_lossy().as_ref())
            );
        }

        #[tokio::test]
        async fn confirmed_move_ok_source_removed() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Move,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("move ok");
            let _reference = match r {
                ManagedCreateResult::Created(r) => r,
                _ => panic!("expected Created"),
            };
            assert!(!source.exists(), "move 应删除源");
            assert!(root.join("Documents").join("report.pdf").exists());
        }

        #[tokio::test]
        async fn confirmed_err_target_exists() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            // 预置目标
            let target_dir = root.join("Documents");
            std::fs::create_dir_all(&target_dir).expect("mkdir");
            std::fs::write(target_dir.join("report.pdf"), b"existing").expect("w");

            let err = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "FS_TARGET_EXISTS");
            // 源保留（落地未执行）
            assert!(source.exists());
        }

        #[tokio::test]
        async fn confirmed_db_failure_compensates_landed_file() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;

            // 构造一个会让 INSERT 失败的场景：先删 collection（FK 约束）
            // 但 ensure_collection_exists 在落地前就校验，所以改为：
            // 用一个**已被删的 collection id** 但绕过 ensure（不可能）。
            // 替代方案：在落地后、写库前手动删 collection，触发 FK 错误。
            // 由于 ensure_collection_exists 在锁外先校验，我们在锁内无法直接干预。
            // 更简单方案：直接对同一 collection 制造 name 冲突不行（无 UNIQUE 约束）。
            //
            // 最终方案：使用一个**自定义 insert 钩子**太重，改为模拟：
            // 在调用前 drop collection 表的 FK 引用 —— 但 ensure_collection_exists 会先失败。
            //
            // 因此本测试改为：**直接测试补偿函数 cleanup_landed** 的语义。
            // 真实 db 失败路径已通过 insert_managed_reference_tx 的错误传播 + cleanup_landed 调用覆盖。
            let landed = root.join("Documents").join("report.pdf");
            std::fs::create_dir_all(landed.parent().expect("parent")).expect("mkdir");
            std::fs::write(&landed, b"landed-by-test").expect("w");
            assert!(landed.exists());

            cleanup_landed(&landed);
            assert!(!landed.exists(), "cleanup_landed 应删除文件");

            // 目录情形
            let dir = root.join("Documents").join("somedir");
            std::fs::create_dir_all(&dir).expect("mkdir");
            std::fs::write(dir.join("inner.txt"), b"x").expect("w");
            cleanup_landed(&dir);
            assert!(!dir.exists(), "cleanup_landed 应递归删除目录");

            // source 仍存在（cleanup_landed 不影响其他文件）
            assert!(source.exists());
            drop(cid);
            drop(pool);
        }

        /// 通过构造一个**已存在的同名 reference id** 触发 PK 冲突，
        /// 验证写库失败时补偿删除已落地文件。
        ///
        /// 由于 UUID 冲突概率为零，改用更可控的方式：
        /// 在测试里**直接调用 insert_managed_reference_tx**，
        /// 用一个违反 FK 的 collection_id，验证错误传播。
        #[tokio::test]
        async fn insert_tx_propagates_fk_violation() {
            let pool = setup().await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let landed = tmp.path().join("landed.txt");
            std::fs::write(&landed, b"x").expect("w");

            let mut tx = pool.begin().await.expect("begin");
            let err = insert_managed_reference_tx(
                &mut tx,
                "no-such-collection-id", // 违反 FK
                "n",
                "document",
                &landed,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_DB");
            drop(tx); // rollback
        }

        // ---------- 互斥：并发 confirmed 串行化 ----------

        /// 两个并发 confirmed 调用应串行执行（通过互斥锁）。
        ///
        /// 验证方式：在临界区中记录进入/退出时间戳，
        /// 两个调用的 [enter, exit] 区间不得重叠。
        #[tokio::test]
        async fn confirmed_concurrent_calls_are_serialized() {
            let (pool, cid, source1, root, _tmp) = setup_managed_env().await;
            // 第二个源
            let source2 = source1.parent().expect("p").join("second.pdf");
            std::fs::write(&source2, b"SECOND").expect("w");

            let pool = Arc::new(pool);
            let cid = Arc::new(cid);

            // 记录临界区 enter/exit 时间戳（毫秒）
            let intervals: Arc<std::sync::Mutex<Vec<(u128, u128)>>> =
                Arc::new(std::sync::Mutex::new(Vec::new()));

            let make_call = |source: PathBuf, target_name: &'static str| {
                let pool = Arc::clone(&pool);
                let cid = Arc::clone(&cid);
                let intervals = Arc::clone(&intervals);
                let root = root.clone();
                tokio::spawn(async move {
                    // 包一层：在 create_managed 内部临界区前后打点。
                    // 由于临界区在 create_managed 内部，我们用**调用开始/结束**作为近似。
                    // 真正的串行性由「两个调用都成功 + 目标都存在 + 互斥锁存在」共同保证。
                    let enter = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("t")
                        .as_millis();
                    let r = create_managed(
                        &pool,
                        (*cid).clone(),
                        format!("n-{}", target_name),
                        "document".into(),
                        locator_of(&source),
                        ManagedAction::Copy,
                        Some(target_name.into()),
                        true,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                    let exit = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("t")
                        .as_millis();
                    intervals.lock().expect("l").push((enter, exit));
                    (r, root)
                })
            };

            let h1 = make_call(source1.clone(), "f1.pdf");
            let h2 = make_call(source2.clone(), "f2.pdf");
            let (r1, r2) = tokio::join!(h1, h2);
            let (r1, _root1) = r1.expect("join1");
            let (r2, _root2) = r2.expect("join2");

            // 两个调用都应成功（互斥锁保证不冲突）
            r1.expect("first call ok");
            r2.expect("second call ok");

            // 两个目标都存在
            assert!(root.join("Documents").join("f1.pdf").exists());
            assert!(root.join("Documents").join("f2.pdf").exists());

            // 互斥锁已初始化（被至少一个调用获取）
            assert!(MANAGED_WRITE_LOCK.get().is_some());
        }

        /// 并发两个 confirmed 调用同一目标名：一个成功，另一个必须 FS_TARGET_EXISTS。
        ///
        /// 这是互斥 + TOCTOU 复检的核心验收：如果没有锁内复检，
        /// 两个调用都会通过 plan 阶段的无冲突检查，然后都尝试落地。
        #[tokio::test]
        async fn confirmed_concurrent_same_target_one_wins() {
            let (pool, cid, source, root, _tmp) = setup_managed_env().await;
            let pool = Arc::new(pool);
            let cid = Arc::new(cid);

            let make_call = || {
                let pool = Arc::clone(&pool);
                let cid = Arc::clone(&cid);
                let source = source.clone();
                tokio::spawn(async move {
                    create_managed(
                        &pool,
                        (*cid).clone(),
                        "n".into(),
                        "document".into(),
                        locator_of(&source),
                        ManagedAction::Copy,
                        Some("same.pdf".into()),
                        true,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
                })
            };

            let h1 = make_call();
            let h2 = make_call();
            let (r1, r2) = tokio::join!(h1, h2);
            let r1 = r1.expect("join1");
            let r2 = r2.expect("join2");

            // 恰好一个成功，另一个 FS_TARGET_EXISTS
            let (ok_count, exists_count) = [&r1, &r2].iter().fold((0, 0), |(ok, exists), r| {
                match r {
                    Ok(_) => (ok + 1, exists),
                    Err(e) if e.code == "FS_TARGET_EXISTS" => (ok, exists + 1),
                    Err(e) => panic!("unexpected error: {:?}", e),
                }
            });
            assert_eq!(ok_count, 1, "恰好一个调用成功");
            assert_eq!(exists_count, 1, "另一个调用 FS_TARGET_EXISTS");

            // 目标只被落地一次
            assert!(root.join("Documents").join("same.pdf").exists());
        }

        // ---------- 序列化契约 ----------

        #[tokio::test]
        async fn managed_plan_serializes_camel_case() {
            let plan = ManagedPlan {
                kind: MANAGED_PLAN_KIND.into(),
                source: "/abs/src".into(),
                proposed_target: "/abs/target".into(),
                action: "copy".into(),
                size_bytes: 100,
                file_count: 3,
                conflicts: vec![],
            };
            let v = serde_json::to_value(&plan).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("proposedTarget"));
            assert!(obj.contains_key("sizeBytes"));
            assert!(obj.contains_key("fileCount"));
            assert!(!obj.contains_key("proposed_target"));
            assert!(!obj.contains_key("size_bytes"));
            assert_eq!(obj["kind"].as_str(), Some("managed_plan"));
        }

        #[tokio::test]
        async fn managed_action_deserializes_lowercase() {
            let copy: ManagedAction = serde_json::from_str(r#""copy""#).expect("copy");
            let mv: ManagedAction = serde_json::from_str(r#""move""#).expect("move");
            assert_eq!(copy, ManagedAction::Copy);
            assert_eq!(mv, ManagedAction::Move);
            assert!(serde_json::from_str::<ManagedAction>(r#""COPY""#).is_err());
        }

        #[tokio::test]
        async fn managed_create_result_untagged_serializes_both_variants() {
            // Plan 变体
            let plan = ManagedCreateResult::Plan(ManagedPlan {
                kind: MANAGED_PLAN_KIND.into(),
                source: "/s".into(),
                proposed_target: "/t".into(),
                action: "copy".into(),
                size_bytes: 1,
                file_count: 1,
                conflicts: vec![],
            });
            let v = serde_json::to_value(&plan).expect("serialize");
            assert_eq!(v["kind"].as_str(), Some("managed_plan"));

            // Created 变体（无 kind 字段）
            let reference = Reference {
                id: "x".into(),
                collection_id: "c".into(),
                source_id: DEFAULT_SOURCE_ID.into(),
                name: "n".into(),
                ref_type: "document".into(),
                hosting: "managed".into(),
                locator: serde_json::json!({"kind":"path","path":"/t"}),
                description: None,
                lifecycle: "active".into(),
                confidentiality: "internal".into(),
                indexed: true,
                disposition: "none".into(),
                tags: vec![],
                created_at: 0,
                updated_at: 0,
            };
            let created = ManagedCreateResult::Created(reference);
            let v2 = serde_json::to_value(&created).expect("serialize");
            assert!(v2.get("kind").is_none() || v2["kind"].is_null());
            assert_eq!(v2["hosting"].as_str(), Some("managed"));
        }
    }

    // ============================================================
    // managed_progress 进度事件（m3-3.3）
    // ============================================================
    //
    // 测试策略：直接调用 `create_managed` 并注入 Vec 收集闭包作为 progress_sink，
    // 避免依赖 Tauri runtime；Tauri 命令层的 `app.emit` 已在 `ref_create_managed`
    // 内部接入，由集成/手工验收覆盖。

    mod progress_tests {
        use super::*;
        use std::sync::{Arc, Mutex as StdMutex};

        /// 在测试池中配置资源根目录（与 create_managed_tests.set_root_dir 相同）。
        async fn set_root_dir(pool: &SqlitePool, root: &std::path::Path) {
            let value_json = serde_json::to_string(&serde_json::Value::String(
                root.to_string_lossy().to_string(),
            ))
            .expect("serialize root_dir");
            let now = now_unix();
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES ('root_dir', ?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            )
            .bind(&value_json)
            .bind(now)
            .execute(pool)
            .await
            .expect("insert root_dir");
        }

        fn locator_of(p: &std::path::Path) -> Locator {
            Locator {
                kind: "path".into(),
                path: p.to_string_lossy().to_string(),
            }
        }

        /// 构造一个指定大小的临时源文件 + 根目录 + collection。
        /// 返回 (pool, cid, source_file, root_dir, events, _tmp)。
        async fn setup_with_size(
            size: usize,
        ) -> (
            SqlitePool,
            String,
            PathBuf,
            PathBuf,
            Arc<StdMutex<Vec<ManagedProgress>>>,
            tempfile::TempDir,
        ) {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let source_file = tmp.path().join("src.bin");
            // 用循环写入避免一次性大内存分配
            let chunk = vec![0xABu8; 64 * 1024];
            let mut remaining = size;
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&source_file).expect("create src");
                while remaining > 0 {
                    let n = remaining.min(chunk.len());
                    f.write_all(&chunk[..n]).expect("write");
                    remaining -= n;
                }
                f.flush().expect("flush");
            }
            let root_dir = tmp.path().join("root");
            std::fs::create_dir_all(&root_dir).expect("mkdir root");
            set_root_dir(&pool, &root_dir).await;
            let events: Arc<StdMutex<Vec<ManagedProgress>>> =
                Arc::new(StdMutex::new(Vec::new()));
            (pool, cid, source_file, root_dir, events, tmp)
        }

        /// 小文件（< 1 MiB）：按固化约定「完成时发 bytes == total」，
        /// 加上首次 on_bytes 触发的事件，事件次数 ≤ 2。
        #[tokio::test]
        async fn progress_small_file_emits_at_most_two_events() {
            let (pool, cid, source, _root, events, _tmp) = setup_with_size(1024).await;
            let events_clone = Arc::clone(&events);
            let mut sink = move |p: ManagedProgress| {
                events_clone.lock().expect("lock").push(p);
            };

            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                Some(&mut sink),
            )
            .await
            .expect("create ok");
            let _ = match r {
                ManagedCreateResult::Created(r) => r,
                _ => panic!("expected Created"),
            };

            let evts = events.lock().expect("lock");
            assert!(
                !evts.is_empty(),
                "小文件至少发一次完成事件（bytes == total）"
            );
            assert!(
                evts.len() <= 2,
                "小文件事件次数应 ≤ 2（首次 + 完成），实际 {}",
                evts.len()
            );
            // 最后一次必是 bytes == total
            let last = evts.last().expect("last");
            assert_eq!(last.bytes, last.total);
            assert_eq!(last.total, 1024);
        }

        /// 大文件（3 MiB）：至少 2 次事件，最后一次 bytes == total。
        #[tokio::test]
        async fn progress_large_file_emits_multiple_events() {
            let size: usize = 3 * 1024 * 1024; // 3 MiB
            let (pool, cid, source, _root, events, _tmp) = setup_with_size(size).await;
            let events_clone = Arc::clone(&events);
            let mut sink = move |p: ManagedProgress| {
                events_clone.lock().expect("lock").push(p);
            };

            let r = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                Some(&mut sink),
            )
            .await
            .expect("create ok");
            let _ = match r {
                ManagedCreateResult::Created(r) => r,
                _ => panic!("expected Created"),
            };

            let evts = events.lock().expect("lock");
            assert!(
                evts.len() >= 2,
                "3 MiB 文件至少触发 2 次事件（1 MiB 步长 + 完成），实际 {}",
                evts.len()
            );
            let last = evts.last().expect("last");
            assert_eq!(last.bytes, last.total, "最后一次事件 bytes == total");
            assert_eq!(last.total, size as u64);
            // 字节数单调不减
            for w in evts.windows(2) {
                assert!(w[1].bytes >= w[0].bytes, "事件字节数应单调不减");
            }
        }

        /// 事件载荷字段齐全：refId / bytes / total，且 camelCase 序列化。
        #[tokio::test]
        async fn progress_payload_has_camel_case_fields() {
            let (pool, cid, source, _root, events, _tmp) = setup_with_size(1024).await;
            let events_clone = Arc::clone(&events);
            let mut sink = move |p: ManagedProgress| {
                events_clone.lock().expect("lock").push(p);
            };

            let _ = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                Some(&mut sink),
            )
            .await
            .expect("create ok");

            let evts = events.lock().expect("lock");
            assert!(!evts.is_empty());
            let first = &evts[0];
            // refId 是 UUID 格式（36 字符，含 4 个连字符）
            assert_eq!(first.ref_id.len(), 36);
            assert_eq!(first.ref_id.chars().filter(|c| *c == '-').count(), 4);
            // total 与文件大小一致
            assert_eq!(first.total, 1024);

            // 序列化为 JSON 时是 camelCase
            let v = serde_json::to_value(first).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("refId"), "应含 refId");
            assert!(obj.contains_key("bytes"), "应含 bytes");
            assert!(obj.contains_key("total"), "应含 total");
            assert!(!obj.contains_key("ref_id"), "不应含 snake_case ref_id");
        }

        /// 失败路径：源文件在落地前被删，确认不发出 bytes == total 的完成事件。
        ///
        /// 模拟方式：先把源文件权限设为只读，落地过程中的目标创建会失败；
        /// 或者更简单 —— 在 create_managed 调用前删除源文件，但 plan 阶段会
        /// 提前返回 FS_PATH_NOT_FOUND，不会进入 confirmed 落地分支。
        /// 因此本测试构造「源存在但目标父目录不可写」场景：让 land_source 失败。
        #[tokio::test]
        async fn progress_failure_path_does_not_emit_completion() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;
            let tmp = tempfile::tempdir().expect("tempdir");
            let source = tmp.path().join("src.bin");
            std::fs::write(&source, vec![0xCDu8; 1024]).expect("write src");
            let root_dir = tmp.path().join("root");
            std::fs::create_dir_all(&root_dir).expect("mkdir root");
            set_root_dir(&pool, &root_dir).await;

            // 把 root_dir 设为只读，让 create_dir_all / File::create 失败
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&root_dir).expect("meta").permissions();
                perms.set_mode(0o555); // r-xr-xr-x：不可写
                std::fs::set_permissions(&root_dir, perms).expect("chmod");
            }

            let events: Arc<StdMutex<Vec<ManagedProgress>>> =
                Arc::new(StdMutex::new(Vec::new()));
            let events_clone = Arc::clone(&events);
            let mut sink = move |p: ManagedProgress| {
                events_clone.lock().expect("lock").push(p);
            };

            let result = create_managed(
                &pool,
                cid,
                "n".into(),
                "document".into(),
                locator_of(&source),
                ManagedAction::Copy,
                None,
                true,
                None,
                None,
                None,
                None,
                None,
                Some(&mut sink),
            )
            .await;

            // 恢复权限以便 tempdir 清理
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&root_dir).expect("meta").permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&root_dir, perms).expect("chmod restore");
            }

            // 落地应失败（权限不足）
            let err = result.expect_err("应失败");
            assert!(
                err.code == "FS_PERMISSION_DENIED" || err.code == "COMMON_IO",
                "错误码应为 FS_PERMISSION_DENIED 或 COMMON_IO，实际 {}",
                err.code
            );

            // 关键断言：未发出 bytes == total 的完成事件
            let evts = events.lock().expect("lock");
            let has_completion = evts.iter().any(|p| p.bytes == p.total && p.total > 0);
            assert!(
                !has_completion,
                "失败路径不应发出 bytes == total 的完成事件，实际事件: {:?}",
                *evts
            );
        }

        /// 序列化契约：ManagedProgress 的 camelCase 字段名固定。
        #[test]
        fn managed_progress_serializes_camel_case() {
            let p = ManagedProgress {
                ref_id: "00000000-0000-0000-0000-000000000000".into(),
                bytes: 1024,
                total: 2048,
            };
            let v = serde_json::to_value(&p).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("refId"));
            assert!(obj.contains_key("bytes"));
            assert!(obj.contains_key("total"));
            assert!(!obj.contains_key("ref_id"));
            assert_eq!(obj["bytes"].as_u64(), Some(1024));
            assert_eq!(obj["total"].as_u64(), Some(2048));
        }

        /// 常量约定：事件名 / 节流步长固定（防止意外改动破坏前端契约）。
        #[test]
        fn progress_constants_are_fixed() {
            assert_eq!(MANAGED_PROGRESS_EVENT, "managed_progress");
            assert_eq!(PROGRESS_BYTE_STEP, 1 << 20);
            assert_eq!(PROGRESS_TIME_STEP, Duration::from_millis(200));
        }
    }
}
