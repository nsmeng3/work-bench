//! 文件系统操作（详细设计 §2.5）
//!
//! 本任务（m2-2.6）实现 3 个命令：
//! `ref_check_health` / `ref_open` / `ref_reveal_in_finder`。
//!
//! 关键约束：
//! - `ref_check_health` 仅做只读元数据检测（`std::fs::metadata`），
//!   不打开文件内容、不修改任何数据。
//! - `ref_open` / `ref_reveal_in_finder` 起子进程必须使用
//!   `std::process::Command`，路径作为独立 argv 元素传递，
//!   **绝不拼接 shell 字符串**（防注入）。
//! - 跨平台分支用 `#[cfg(target_os = ...)]`；当前验收在 macOS，
//!   其他平台仅需编译通过。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::error::{AppError, CmdResult};

/// 引用健康度（§2.5 ref_check_health.health）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Health {
    Ok,
    Missing,
    Unknown,
}

/// `ref_check_health` 出参条目（§2.5）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub ref_id: String,
    pub health: Health,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// `ref_open` 出参（§2.5）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    pub opened: bool,
    /// system_default | custom
    pub strategy: String,
}

// ============================================================
// 内部工具
// ============================================================

/// 从 `resource_reference` 读取 `(id, locator_json)`。
///
/// 仅供 fs_ops 内部使用；不复用 reference.rs 的 `fetch_reference`，
/// 因为健康检查只需要 locator，无需 tags 等额外字段。
async fn fetch_locator(pool: &SqlitePool, id: &str) -> CmdResult<serde_json::Value> {
    let row = sqlx::query("SELECT locator_json FROM resource_reference WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", id)))?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let locator: serde_json::Value = serde_json::from_str(&locator_json)
        .map_err(|e| AppError::db(format!("locator_json 反序列化失败: {}", e)))?;
    Ok(locator)
}

/// 从 `resource_reference` 读取 `(locator_json, type)`。
///
/// m6-6.4 引入：`ref_open` 需要按引用类型读取默认程序配置（§5.3）。
async fn fetch_locator_and_type(
    pool: &SqlitePool,
    id: &str,
) -> CmdResult<(serde_json::Value, String)> {
    let row = sqlx::query("SELECT locator_json, type FROM resource_reference WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", id)))?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let ref_type: String = row.try_get("type").map_err(AppError::from)?;
    let locator: serde_json::Value = serde_json::from_str(&locator_json)
        .map_err(|e| AppError::db(format!("locator_json 反序列化失败: {}", e)))?;
    Ok((locator, ref_type))
}

/// 列出某 collection 下所有引用 id（不过滤 disposition：
/// 健康检查应覆盖全部，UI 自行决定是否展示已删除项）。
async fn list_ids_by_collection(pool: &SqlitePool, collection_id: &str) -> CmdResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT id FROM resource_reference WHERE collection_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    Ok(rows
        .iter()
        .map(|r| r.get::<String, _>("id"))
        .collect())
}

/// 从 locator JSON 提取 `path`（仅支持 `kind == "path"`）。
///
/// 其他 kind（repo/cloud 等预留形态）返回 `None`，由调用方标记 `unknown`。
fn extract_path(locator: &serde_json::Value) -> Option<String> {
    let kind = locator.get("kind")?.as_str()?;
    if kind != "path" {
        return None;
    }
    let path = locator.get("path")?.as_str()?;
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

/// 单条健康检查：只读 `std::fs::metadata`，不打开文件内容。
async fn check_one(pool: &SqlitePool, ref_id: &str) -> CmdResult<HealthReport> {
    let locator = fetch_locator(pool, ref_id).await?;
    match extract_path(&locator) {
        None => Ok(HealthReport {
            ref_id: ref_id.to_string(),
            health: Health::Unknown,
            detail: Some("locator 非 path 形态或字段缺失".into()),
        }),
        Some(path) => {
            // 仅 metadata 探测；不打开文件、不读内容。
            match std::fs::metadata(&path) {
                Ok(_) => Ok(HealthReport {
                    ref_id: ref_id.to_string(),
                    health: Health::Ok,
                    detail: None,
                }),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HealthReport {
                    ref_id: ref_id.to_string(),
                    health: Health::Missing,
                    detail: Some(format!("路径不存在: {}", path)),
                }),
                Err(e) => Ok(HealthReport {
                    ref_id: ref_id.to_string(),
                    health: Health::Unknown,
                    detail: Some(format!("metadata 读取失败: {}", e)),
                }),
            }
        }
    }
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `ref_check_health`：入参二选一（`id` 或 `collection_id`）。
///
/// 只检测不修改数据；路径存在 → `ok`，不存在 → `missing`，
/// 其他情况（非 path locator / 元数据读取失败）→ `unknown`。
pub async fn check_health(
    pool: &SqlitePool,
    id: Option<String>,
    collection_id: Option<String>,
) -> CmdResult<Vec<HealthReport>> {
    match (id, collection_id) {
        (Some(id), None) => Ok(vec![check_one(pool, &id).await?]),
        (None, Some(cid)) => {
            let ids = list_ids_by_collection(pool, &cid).await?;
            let mut out = Vec::with_capacity(ids.len());
            for rid in &ids {
                out.push(check_one(pool, rid).await?);
            }
            Ok(out)
        }
        (Some(_), Some(_)) => Err(AppError::invalid_param(
            "id 与 collectionId 二选一，不可同时提供",
        )),
        (None, None) => Err(AppError::invalid_param(
            "id 与 collectionId 必须提供其一",
        )),
    }
}

/// `ref_open` 策略选择结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenStrategy {
    /// 系统默认程序。
    SystemDefault,
    /// 类型默认程序（来自 settings 表 `default_app_{type}`）。
    TypedDefault(String),
    /// 本次调用显式指定（appOverride）。
    Custom(String),
}

impl OpenStrategy {
    /// 序列化为 `OpenResult.strategy` 字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            OpenStrategy::SystemDefault => "system_default",
            OpenStrategy::TypedDefault(_) => "app",
            OpenStrategy::Custom(_) => "custom",
        }
    }

    /// 提取实际用于 spawn 的 app 路径（SystemDefault 时返回 None）。
    pub fn app_path(&self) -> Option<&str> {
        match self {
            OpenStrategy::SystemDefault => None,
            OpenStrategy::TypedDefault(p) => Some(p.as_str()),
            OpenStrategy::Custom(p) => Some(p.as_str()),
        }
    }
}

/// 选择 `ref_open` 的打开策略（纯函数，便于测试）。
///
/// 优先级：`appOverride` > 类型默认程序 > 系统默认。
///
/// - `app_override` 非空白 → `Custom(app)`。
/// - 否则查 `settings` 表 `default_app_{ref_type}`：
///   - `strategy = "app"` 且 `appPath` 非空白 → `TypedDefault(appPath)`。
///   - 其他（`system_default` / 无配置 / appPath 缺失或空白）→ `SystemDefault`。
pub async fn choose_open_strategy(
    pool: &SqlitePool,
    ref_type: &str,
    app_override: Option<String>,
) -> CmdResult<OpenStrategy> {
    if let Some(app) = app_override {
        let trimmed = app.trim();
        if !trimmed.is_empty() {
            return Ok(OpenStrategy::Custom(trimmed.to_string()));
        }
    }
    match crate::settings::load_default_app(pool, ref_type).await? {
        Some(cfg) if cfg.strategy == "app" => {
            if let Some(p) = cfg.app_path {
                let trimmed = p.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok(OpenStrategy::TypedDefault(trimmed));
                }
            }
            Ok(OpenStrategy::SystemDefault)
        }
        _ => Ok(OpenStrategy::SystemDefault),
    }
}

/// `ref_open`：按优先级选择打开策略（§2.5 / §5.3）。
///
/// 优先级：`appOverride` > 类型默认程序 > 系统默认。
///
/// - `app_override` 非空 → 用指定程序打开（strategy = "custom"）。
/// - `app_override` 为空 → 按引用类型读取 `settings` 表 `default_app_{type}`：
///   - 配置 `strategy = "app"` 且 `appPath` 非空 → 用配置的 appPath 打开（strategy = "app"）。
///   - 配置 `strategy = "system_default"` 或无配置 → 系统默认（strategy = "system_default"）。
///
/// 平台分支：
/// - macOS：默认 `open <path>`；自定义 `open -a <app> <path>`。
/// - Windows：默认 `explorer <path>`；自定义直接 `<app> <path>`。
/// - Linux：默认 `xdg-open <path>`；自定义直接 `<app> <path>`。
///
/// 安全：路径作为独立 argv 元素传递，绝不拼接 shell 字符串。
pub async fn open(
    pool: &SqlitePool,
    id: String,
    app_override: Option<String>,
) -> CmdResult<OpenResult> {
    let (locator, ref_type) = fetch_locator_and_type(pool, &id).await?;
    let path = extract_path(&locator).ok_or_else(|| {
        AppError::invalid_param(format!("引用 {} 的 locator 非 path 形态，无法打开", id))
    })?;

    // 路径存在性 / 权限校验（仅 metadata，不读内容）。
    match std::fs::metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::new(
                "FS_PATH_NOT_FOUND",
                format!("目标路径不存在: {}", path),
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(AppError::new(
                "FS_PERMISSION_DENIED",
                format!("无权限访问: {}", path),
            ));
        }
        Err(e) => {
            return Err(AppError::io(format!("读取路径元数据失败 {}: {}", path, e)));
        }
    }

    let strategy = choose_open_strategy(pool, &ref_type, app_override).await?;
    let strategy_label = strategy.as_str().to_string();
    spawn_open(&path, strategy.app_path())?;

    Ok(OpenResult {
        opened: true,
        strategy: strategy_label,
    })
}

/// `ref_reveal_in_finder`：在系统文件管理器中显示。
///
/// - macOS：`open -R <path>`
/// - Windows：`explorer /select,<path>`
/// - Linux：退化为 `xdg-open <parent_dir>`
pub async fn reveal_in_finder(pool: &SqlitePool, id: String) -> CmdResult<()> {
    let locator = fetch_locator(pool, &id).await?;
    let path = extract_path(&locator).ok_or_else(|| {
        AppError::invalid_param(format!("引用 {} 的 locator 非 path 形态，无法定位", id))
    })?;

    if !std::path::Path::new(&path).exists() {
        return Err(AppError::new(
            "FS_PATH_NOT_FOUND",
            format!("目标路径不存在: {}", path),
        ));
    }

    spawn_reveal(&path)
}

// ============================================================
// m7-7.1 · ref_log_access（访问埋点）
// ============================================================

/// `ref_log_access` 允许的 action 取值。
/// 与 `0006_ref_access_log.sql` 的 CHECK 约束保持一致。
const VALID_ACCESS_ACTIONS: &[&str] = &["open", "reveal", "copy_path", "open_with"];

/// `ref_log_access`：写入一条 `ref_access_log` 记录。
///
/// 校验：
/// - `action` 必须在白名单内，否则 `COMMON_INVALID_PARAM`；
/// - `ref_id` 必须存在，否则 `COMMON_NOT_FOUND`（外键约束兜底，但提前查更友好）。
///
/// 失败语义：调用方（前端）应 `await` 但捕获错误仅 `console.warn`，
/// 不影响主流程 —— 埋点丢失可接受，用户体验优先。
pub async fn log_access(pool: &SqlitePool, ref_id: String, action: String) -> CmdResult<()> {
    let action_trimmed = action.trim();
    if !VALID_ACCESS_ACTIONS.contains(&action_trimmed) {
        return Err(AppError::invalid_param(format!(
            "非法 action: {}（允许值: {}）",
            action_trimmed,
            VALID_ACCESS_ACTIONS.join(", ")
        )));
    }

    // 提前校验 ref_id 存在，给出友好错误码（而不是 FOREIGN KEY constraint failed）。
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM resource_reference WHERE id = ?")
            .bind(&ref_id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::from)?;
    if exists.is_none() {
        return Err(AppError::not_found(format!(
            "资源引用不存在: {}",
            ref_id
        )));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let at = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query("INSERT INTO ref_access_log (id, ref_id, action, at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&ref_id)
        .bind(action_trimmed)
        .bind(at)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

// ============================================================
// 平台分支：起子进程（绝不拼接 shell 字符串）
// ============================================================

#[cfg(target_os = "macos")]
fn spawn_open(path: &str, app_override: Option<&str>) -> CmdResult<()> {
    let mut cmd = std::process::Command::new("open");
    if let Some(app) = app_override {
        cmd.arg("-a").arg(app);
    }
    cmd.arg(path);
    cmd.spawn()
        .map_err(|e| AppError::io(format!("启动 open 失败: {}", e)))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_open(path: &str, app_override: Option<&str>) -> CmdResult<()> {
    match app_override {
        None => {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| AppError::io(format!("启动 explorer 失败: {}", e)))?;
        }
        Some(app) => {
            std::process::Command::new(app)
                .arg(path)
                .spawn()
                .map_err(|e| AppError::io(format!("启动 {} 失败: {}", app, e)))?;
        }
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_open(path: &str, app_override: Option<&str>) -> CmdResult<()> {
    match app_override {
        None => {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| AppError::io(format!("启动 xdg-open 失败: {}", e)))?;
        }
        Some(app) => {
            std::process::Command::new(app)
                .arg(path)
                .spawn()
                .map_err(|e| AppError::io(format!("启动 {} 失败: {}", app, e)))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_reveal(path: &str) -> CmdResult<()> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| AppError::io(format!("启动 open -R 失败: {}", e)))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_reveal(path: &str) -> CmdResult<()> {
    // `explorer /select,<path>`：`/select,<path>` 是 explorer 的单个参数，
    // 不经过 shell，逗号后路径作为同一 argv 元素的一部分，无注入风险。
    let arg = format!("/select,{}", path);
    std::process::Command::new("explorer")
        .arg(arg)
        .spawn()
        .map_err(|e| AppError::io(format!("启动 explorer 失败: {}", e)))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_reveal(path: &str) -> CmdResult<()> {
    // Linux 无统一 "reveal" 概念，退化为打开所在目录。
    let parent = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    std::process::Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| AppError::io(format!("启动 xdg-open 失败: {}", e)))?;
    Ok(())
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_check_health(
    state: tauri::State<'_, crate::AppState>,
    id: Option<String>,
    collection_id: Option<String>,
) -> CmdResult<Vec<HealthReport>> {
    check_health(&state.pool, id, collection_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_open(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    app_override: Option<String>,
) -> CmdResult<OpenResult> {
    open(&state.pool, id, app_override).await
}

#[tauri::command]
pub async fn ref_reveal_in_finder(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<()> {
    reveal_in_finder(&state.pool, id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_log_access(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
    action: String,
) -> CmdResult<()> {
    log_access(&state.pool, ref_id, action).await
}

// ============================================================
// m7-7.3 · ref_recent_access（Dashboard 最近资源）
// ============================================================

/// `ref_recent_access` 出参条目（任务包 m7-7.3 §后端新增命令）。
///
/// 按 ref_id 去重，取每个 ref 最近一次 access_log，按 at DESC 排序。
/// 过滤 `disposition='deleted'` 的引用（已删除资源不应出现在最近列表）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecentRef {
    pub ref_id: String,
    pub ref_name: String,
    /// 引用类型：'code' | 'document' | ...
    pub ref_type: String,
    /// 最近一次动作：'open' | 'reveal' | 'copy_path' | 'open_with'
    pub last_action: String,
    /// 最近一次访问时间（Unix 秒）
    pub last_at: i64,
    /// 透传 resource_reference.locator_json（前端展示路径用）
    pub locator_json: String,
}

/// `ref_recent_access`：返回最近访问的资源列表（按 ref 去重）。
///
/// SQL 思路：
/// 1. 子查询对每个 ref_id 取 MAX(at)；
/// 2. JOIN resource_reference 取名称/类型/locator；
/// 3. 过滤 disposition='deleted'；
/// 4. 按 at DESC 排序，LIMIT ?。
///
/// `limit` 缺省 10，上限 100（防止前端误传超大值拖慢查询）。
pub async fn recent_access(pool: &SqlitePool, limit: Option<i64>) -> CmdResult<Vec<RecentRef>> {
    let limit = limit.unwrap_or(10).clamp(1, 100);
    let rows = sqlx::query(
        "SELECT r.id AS ref_id, r.name AS ref_name, r.type AS ref_type, \
                r.locator_json AS locator_json, l.action AS last_action, l.at AS last_at \
         FROM ref_access_log l \
         JOIN resource_reference r ON r.id = l.ref_id \
         WHERE l.at = (SELECT MAX(at) FROM ref_access_log WHERE ref_id = l.ref_id) \
           AND r.disposition != 'deleted' \
         GROUP BY l.ref_id \
         ORDER BY l.at DESC \
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(RecentRef {
            ref_id: row.try_get("ref_id").map_err(AppError::from)?,
            ref_name: row.try_get("ref_name").map_err(AppError::from)?,
            ref_type: row.try_get("ref_type").map_err(AppError::from)?,
            last_action: row.try_get("last_action").map_err(AppError::from)?,
            last_at: row.try_get("last_at").map_err(AppError::from)?,
            locator_json: row.try_get("locator_json").map_err(AppError::from)?,
        });
    }
    Ok(out)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ref_recent_access(
    state: tauri::State<'_, crate::AppState>,
    limit: Option<i64>,
) -> CmdResult<Vec<RecentRef>> {
    recent_access(&state.pool, limit).await
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_in_memory;
    use uuid::Uuid;

    async fn setup() -> SqlitePool {
        init_pool_in_memory().await.expect("migrate ok")
    }

    const PRESET_SPACE: &str = "preset_space_work";

    async fn make_collection(pool: &SqlitePool) -> String {
        let id = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
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

    /// 直接插入一条 reference（不走 create_external，便于构造 missing 路径）。
    async fn insert_reference(pool: &SqlitePool, collection_id: &str, path: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let locator_json = serde_json::to_string(&serde_json::json!({
            "kind": "path",
            "path": path,
        }))
        .expect("serialize locator");
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, 'src_local_fs_default', 'r', 'code', 'external', ?, NULL, \
                     'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(&locator_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference");
        id
    }

    /// 插入一条非 path locator 的 reference（用于 unknown 分支）。
    async fn insert_reference_with_locator(
        pool: &SqlitePool,
        collection_id: &str,
        locator: serde_json::Value,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let locator_json = serde_json::to_string(&locator).expect("serialize");
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, 'src_local_fs_default', 'r', 'code', 'external', ?, NULL, \
                     'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(&locator_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference");
        id
    }

    // ---------- ref_check_health ----------

    #[tokio::test]
    async fn check_health_ok_for_existing_path() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = insert_reference(&pool, &cid, &file.to_string_lossy()).await;

        let reports = check_health(&pool, Some(rid.clone()), None)
            .await
            .expect("check ok");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].ref_id, rid);
        assert_eq!(reports[0].health, Health::Ok);
        assert!(reports[0].detail.is_none());
    }

    #[tokio::test]
    async fn check_health_missing_for_nonexistent_path() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("no-such-file.txt");
        let rid = insert_reference(&pool, &cid, &missing.to_string_lossy()).await;

        let reports = check_health(&pool, Some(rid.clone()), None)
            .await
            .expect("check ok");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].health, Health::Missing);
        assert!(reports[0].detail.is_some());
    }

    #[tokio::test]
    async fn check_health_unknown_for_non_path_locator() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference_with_locator(
            &pool,
            &cid,
            serde_json::json!({ "kind": "cloud", "path": "s3://bucket/key" }),
        )
        .await;

        let reports = check_health(&pool, Some(rid.clone()), None)
            .await
            .expect("check ok");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].health, Health::Unknown);
        assert!(reports[0].detail.is_some());
    }

    #[tokio::test]
    async fn check_health_err_when_id_not_found() {
        let pool = setup().await;
        let err = check_health(&pool, Some("no-such-id".into()), None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn check_health_err_when_both_params_provided() {
        let pool = setup().await;
        let err = check_health(&pool, Some("a".into()), Some("b".into()))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn check_health_err_when_neither_param_provided() {
        let pool = setup().await;
        let err = check_health(&pool, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn check_health_by_collection_returns_all() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;

        let dir = tempfile::tempdir().expect("tempdir");
        let f_ok = dir.path().join("ok.txt");
        std::fs::write(&f_ok, b"x").expect("write");
        let f_missing = dir.path().join("missing.txt");

        let rid_ok = insert_reference(&pool, &cid, &f_ok.to_string_lossy()).await;
        let rid_missing = insert_reference(&pool, &cid, &f_missing.to_string_lossy()).await;
        let rid_unknown = insert_reference_with_locator(
            &pool,
            &cid,
            serde_json::json!({ "kind": "repo", "url": "https://x" }),
        )
        .await;

        let reports = check_health(&pool, None, Some(cid.clone()))
            .await
            .expect("check ok");
        assert_eq!(reports.len(), 3);

        let by_id: std::collections::HashMap<_, _> = reports
            .iter()
            .map(|r| (r.ref_id.as_str(), r.health))
            .collect();
        assert_eq!(by_id[rid_ok.as_str()], Health::Ok);
        assert_eq!(by_id[rid_missing.as_str()], Health::Missing);
        assert_eq!(by_id[rid_unknown.as_str()], Health::Unknown);
    }

    #[tokio::test]
    async fn check_health_by_collection_empty_when_no_refs() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let reports = check_health(&pool, None, Some(cid))
            .await
            .expect("check ok");
        assert!(reports.is_empty());
    }

    /// 健康检查不得修改源文件（mtime/内容不变）。
    #[tokio::test]
    async fn check_health_is_readonly() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("immutable.txt");
        let content = b"readonly";
        std::fs::write(&file, content).expect("write");
        let mtime_before = std::fs::metadata(&file)
            .expect("meta")
            .modified()
            .expect("mtime");

        let rid = insert_reference(&pool, &cid, &file.to_string_lossy()).await;
        check_health(&pool, Some(rid), None).await.expect("check ok");

        let mtime_after = std::fs::metadata(&file)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert_eq!(mtime_before, mtime_after);
        let content_after = std::fs::read(&file).expect("read");
        assert_eq!(content, &content_after[..]);
    }

    // ---------- ref_open ----------

    #[tokio::test]
    async fn open_err_path_not_found() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("no-such.txt");
        let rid = insert_reference(&pool, &cid, &missing.to_string_lossy()).await;

        let err = open(&pool, rid, None).await.expect_err("should fail");
        assert_eq!(err.code, "FS_PATH_NOT_FOUND");
    }

    #[tokio::test]
    async fn open_err_when_id_not_found() {
        let pool = setup().await;
        let err = open(&pool, "no-such-id".into(), None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn open_err_when_locator_not_path() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference_with_locator(
            &pool,
            &cid,
            serde_json::json!({ "kind": "cloud", "path": "s3://x" }),
        )
        .await;
        let err = open(&pool, rid, None).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- ref_reveal_in_finder ----------

    #[tokio::test]
    async fn reveal_err_path_not_found() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("no-such.txt");
        let rid = insert_reference(&pool, &cid, &missing.to_string_lossy()).await;

        let err = reveal_in_finder(&pool, rid).await.expect_err("should fail");
        assert_eq!(err.code, "FS_PATH_NOT_FOUND");
    }

    #[tokio::test]
    async fn reveal_err_when_id_not_found() {
        let pool = setup().await;
        let err = reveal_in_finder(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- 序列化契约 ----------

    #[test]
    fn health_report_serializes_camel_case() {
        let r = HealthReport {
            ref_id: "x".into(),
            health: Health::Ok,
            detail: None,
        };
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("refId"));
        assert!(obj.contains_key("health"));
        assert!(!obj.contains_key("ref_id"));
        // detail=None 时被跳过
        assert!(!obj.contains_key("detail"));
        // health 序列化为小写
        assert_eq!(obj["health"].as_str(), Some("ok"));
    }

    #[test]
    fn open_result_serializes_camel_case() {
        let r = OpenResult {
            opened: true,
            strategy: "system_default".into(),
        };
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        assert_eq!(obj["opened"].as_bool(), Some(true));
        assert_eq!(obj["strategy"].as_str(), Some("system_default"));
    }

    // ---------- m6-6.4 · choose_open_strategy（ref_open 策略选择） ----------

    /// 直接写 settings.default_app_{type}（绕过 set_default_app 命令层）。
    async fn seed_default_app(pool: &SqlitePool, ref_type: &str, value: serde_json::Value) {
        let key = format!("default_app_{}", ref_type);
        let value_json = serde_json::to_string(&value).expect("serialize");
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(&key)
        .bind(&value_json)
        .bind(now)
        .execute(pool)
        .await
        .expect("seed default_app");
    }

    #[tokio::test]
    async fn strategy_override_wins_over_typed_default() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "app", "appPath": "/Applications/VSCode.app" }),
        )
        .await;

        let s = choose_open_strategy(&pool, "code", Some("/Applications/Sublime.app".into()))
            .await
            .expect("choose ok");
        assert_eq!(
            s,
            OpenStrategy::Custom("/Applications/Sublime.app".into())
        );
        assert_eq!(s.as_str(), "custom");
        assert_eq!(s.app_path(), Some("/Applications/Sublime.app"));
    }

    #[tokio::test]
    async fn strategy_typed_default_when_no_override() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "document",
            serde_json::json!({ "strategy": "app", "appPath": "/Applications/Typora.app" }),
        )
        .await;

        let s = choose_open_strategy(&pool, "document", None)
            .await
            .expect("choose ok");
        assert_eq!(
            s,
            OpenStrategy::TypedDefault("/Applications/Typora.app".into())
        );
        assert_eq!(s.as_str(), "app");
        assert_eq!(s.app_path(), Some("/Applications/Typora.app"));
    }

    #[tokio::test]
    async fn strategy_system_default_when_no_config() {
        let pool = setup().await;
        let s = choose_open_strategy(&pool, "media", None)
            .await
            .expect("choose ok");
        assert_eq!(s, OpenStrategy::SystemDefault);
        assert_eq!(s.as_str(), "system_default");
        assert_eq!(s.app_path(), None);
    }

    #[tokio::test]
    async fn strategy_system_default_when_config_is_system_default() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "system_default" }),
        )
        .await;

        let s = choose_open_strategy(&pool, "code", None)
            .await
            .expect("choose ok");
        assert_eq!(s, OpenStrategy::SystemDefault);
    }

    #[tokio::test]
    async fn strategy_system_default_when_app_path_missing() {
        let pool = setup().await;
        // strategy=app 但缺 appPath → 回退 system_default（防御性）
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "app" }),
        )
        .await;

        let s = choose_open_strategy(&pool, "code", None)
            .await
            .expect("choose ok");
        assert_eq!(s, OpenStrategy::SystemDefault);
    }

    #[tokio::test]
    async fn strategy_system_default_when_app_path_blank() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "app", "appPath": "   " }),
        )
        .await;

        let s = choose_open_strategy(&pool, "code", None)
            .await
            .expect("choose ok");
        assert_eq!(s, OpenStrategy::SystemDefault);
    }

    #[tokio::test]
    async fn strategy_blank_override_falls_back_to_typed_default() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "app", "appPath": "/Applications/VSCode.app" }),
        )
        .await;

        // 空白 override 应视为未提供
        let s = choose_open_strategy(&pool, "code", Some("   ".into()))
            .await
            .expect("choose ok");
        assert_eq!(
            s,
            OpenStrategy::TypedDefault("/Applications/VSCode.app".into())
        );
    }

    #[tokio::test]
    async fn strategy_per_type_isolation() {
        let pool = setup().await;
        seed_default_app(
            &pool,
            "code",
            serde_json::json!({ "strategy": "app", "appPath": "/Applications/VSCode.app" }),
        )
        .await;
        seed_default_app(
            &pool,
            "document",
            serde_json::json!({ "strategy": "app", "appPath": "/Applications/Typora.app" }),
        )
        .await;

        let s_code = choose_open_strategy(&pool, "code", None)
            .await
            .expect("code");
        let s_doc = choose_open_strategy(&pool, "document", None)
            .await
            .expect("doc");
        let s_media = choose_open_strategy(&pool, "media", None)
            .await
            .expect("media");

        assert_eq!(s_code.app_path(), Some("/Applications/VSCode.app"));
        assert_eq!(s_doc.app_path(), Some("/Applications/Typora.app"));
        assert_eq!(s_media, OpenStrategy::SystemDefault);
    }

    // ---------- m7-7.1 · ref_log_access ----------

    #[tokio::test]
    async fn log_access_writes_row() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference(&pool, &cid, "/tmp/x").await;

        log_access(&pool, rid.clone(), "open".into())
            .await
            .expect("log ok");

        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT ref_id, action, at FROM ref_access_log WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_all(&pool)
        .await
        .expect("select");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, rid);
        assert_eq!(rows[0].1, "open");
        assert!(rows[0].2 > 0, "at 应为 unix 秒");
    }

    #[tokio::test]
    async fn log_access_rejects_invalid_action() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference(&pool, &cid, "/tmp/x").await;

        let err = log_access(&pool, rid, "delete".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn log_access_rejects_unknown_ref() {
        let pool = setup().await;
        let err = log_access(&pool, "no-such-ref".into(), "open".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn log_access_accepts_all_valid_actions() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference(&pool, &cid, "/tmp/x").await;

        for action in ["open", "reveal", "copy_path", "open_with"] {
            log_access(&pool, rid.clone(), action.into())
                .await
                .unwrap_or_else(|e| panic!("action {} 应成功: {}", action, e));
        }

        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ref_access_log WHERE ref_id = ?")
                .bind(&rid)
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(n, 4);
    }

    // ---------- m7-7.3 · ref_recent_access ----------

    /// 直接 INSERT 一条 ref_access_log（便于控制 at 时间戳）。
    async fn insert_access_log(pool: &SqlitePool, ref_id: &str, action: &str, at: i64) {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ref_access_log (id, ref_id, action, at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(ref_id)
            .bind(action)
            .bind(at)
            .execute(pool)
            .await
            .expect("insert access_log");
    }

    #[tokio::test]
    async fn recent_access_empty_when_no_logs() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let _rid = insert_reference(&pool, &cid, "/tmp/x").await;

        let list = recent_access(&pool, None).await.expect("recent ok");
        assert!(list.is_empty(), "无 access_log 时应返回空");
    }

    #[tokio::test]
    async fn recent_access_dedupes_by_ref_and_returns_latest() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference(&pool, &cid, "/tmp/x").await;

        // 同一 ref 三条日志，时间递增
        insert_access_log(&pool, &rid, "open", 1000).await;
        insert_access_log(&pool, &rid, "reveal", 2000).await;
        insert_access_log(&pool, &rid, "copy_path", 3000).await;

        let list = recent_access(&pool, None).await.expect("recent ok");
        assert_eq!(list.len(), 1, "同一 ref 应去重为 1 条");
        assert_eq!(list[0].ref_id, rid);
        assert_eq!(list[0].last_action, "copy_path", "应返回最近一次动作");
        assert_eq!(list[0].last_at, 3000);
    }

    #[tokio::test]
    async fn recent_access_orders_by_last_at_desc() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid_a = insert_reference(&pool, &cid, "/tmp/a").await;
        let rid_b = insert_reference(&pool, &cid, "/tmp/b").await;
        let rid_c = insert_reference(&pool, &cid, "/tmp/c").await;

        // a 最新，b 次之，c 最旧
        insert_access_log(&pool, &rid_c, "open", 1000).await;
        insert_access_log(&pool, &rid_a, "open", 3000).await;
        insert_access_log(&pool, &rid_b, "open", 2000).await;

        let list = recent_access(&pool, None).await.expect("recent ok");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].ref_id, rid_a, "最新访问的应排第一");
        assert_eq!(list[1].ref_id, rid_b);
        assert_eq!(list[2].ref_id, rid_c);
    }

    #[tokio::test]
    async fn recent_access_respects_limit() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let mut rids = Vec::new();
        for i in 0..5 {
            let rid = insert_reference(&pool, &cid, &format!("/tmp/r{}", i)).await;
            insert_access_log(&pool, &rid, "open", 1000 + i as i64).await;
            rids.push(rid);
        }

        let list = recent_access(&pool, Some(3)).await.expect("recent ok");
        assert_eq!(list.len(), 3, "limit=3 应只返回 3 条");
        // 最近 3 条应是 rids[4], rids[3], rids[2]
        assert_eq!(list[0].ref_id, rids[4]);
        assert_eq!(list[1].ref_id, rids[3]);
        assert_eq!(list[2].ref_id, rids[2]);
    }

    #[tokio::test]
    async fn recent_access_excludes_deleted_disposition() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid_alive = insert_reference(&pool, &cid, "/tmp/alive").await;
        let rid_deleted = insert_reference(&pool, &cid, "/tmp/deleted").await;

        insert_access_log(&pool, &rid_alive, "open", 1000).await;
        insert_access_log(&pool, &rid_deleted, "open", 2000).await;

        // 把 deleted 的 disposition 置为 deleted
        sqlx::query("UPDATE resource_reference SET disposition = 'deleted' WHERE id = ?")
            .bind(&rid_deleted)
            .execute(&pool)
            .await
            .expect("update disposition");

        let list = recent_access(&pool, None).await.expect("recent ok");
        assert_eq!(list.len(), 1, "deleted 资源不应出现在最近列表");
        assert_eq!(list[0].ref_id, rid_alive);
    }

    #[tokio::test]
    async fn recent_access_returns_full_fields() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = insert_reference(&pool, &cid, "/tmp/full").await;
        insert_access_log(&pool, &rid, "reveal", 1234).await;

        let list = recent_access(&pool, None).await.expect("recent ok");
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item.ref_id, rid);
        assert_eq!(item.ref_name, "r", "insert_reference 默认 name='r'");
        assert_eq!(item.ref_type, "code");
        assert_eq!(item.last_action, "reveal");
        assert_eq!(item.last_at, 1234);
        assert!(item.locator_json.contains("/tmp/full"));
    }

    #[tokio::test]
    async fn recent_access_serializes_camel_case() {
        let item = RecentRef {
            ref_id: "r1".into(),
            ref_name: "name".into(),
            ref_type: "code".into(),
            last_action: "open".into(),
            last_at: 1000,
            locator_json: "{}".into(),
        };
        let v = serde_json::to_value(&item).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("refId"));
        assert!(obj.contains_key("refName"));
        assert!(obj.contains_key("refType"));
        assert!(obj.contains_key("lastAction"));
        assert!(obj.contains_key("lastAt"));
        assert!(obj.contains_key("locatorJson"));
        assert!(!obj.contains_key("ref_id"));
    }
}
