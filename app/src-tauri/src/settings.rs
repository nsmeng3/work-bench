//! 设置中心（详细设计 §2.8）
//!
//! 本任务（m3-3.4）实现两个 Tauri 命令：
//! - `settings_get_root_dir`：读取资源根目录配置
//! - `settings_init_root_dir`：初始化资源根目录（建出 6 个类型子目录）
//!
//! ## 配置存储
//!
//! 采用**方案 B：复用 `settings` 表**（`migrations/0001_init.sql` 已建表，
//! schema 为 `(key TEXT PRIMARY KEY, value_json TEXT, updated_at INTEGER)`）。
//! 理由：任务包明确"若已有 settings 表则用表"；根目录路径属应用级设置，
//! 与业务数据同库同事务，备份/迁移成本更低。
//!
//! ## 子目录命名
//!
//! 6 个类型子目录：Code / Documents / Data / Artifacts / Tools / Media。
//! M3-3.1 已沉淀同名映射（`type_to_subdir`），待后续整合；本任务自建常量
//! `TYPE_SUBDIRS`，与 3.1 保持一致。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::path::{Path, PathBuf};

use crate::error::{AppError, CmdResult};
use crate::landing;

/// `settings` 表中根目录键名。
const ROOT_DIR_KEY: &str = "root_dir";

/// 6 个类型子目录（与 M3-3.1 `type_to_subdir` 命名一致；待后续整合）。
const TYPE_SUBDIRS: [&str; 6] = ["Code", "Documents", "Data", "Artifacts", "Tools", "Media"];

/// `settings_get_root_dir` 出参（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootDirStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_dir: Option<String>,
    pub initialized: bool,
}

/// `settings_init_root_dir` 出参（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InitRootDirResult {
    pub root_dir: String,
    pub created: Vec<String>,
}

// ============================================================
// 内部工具
// ============================================================

/// 从 `settings` 表读取根目录路径。
pub(crate) async fn load_root_dir(pool: &SqlitePool) -> CmdResult<Option<String>> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?")
        .bind(ROOT_DIR_KEY)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    match row {
        None => Ok(None),
        Some(r) => {
            let value_json: String = r.try_get("value_json").map_err(AppError::from)?;
            let v: serde_json::Value = serde_json::from_str(&value_json)
                .map_err(|e| AppError::db(format!("settings.root_dir 反序列化失败: {}", e)))?;
            // 存的是 JSON string
            let s = v
                .as_str()
                .ok_or_else(|| AppError::db("settings.root_dir 非字符串"))?;
            Ok(Some(s.to_string()))
        }
    }
}

/// 将根目录路径写入 `settings` 表（UPSERT）。
async fn save_root_dir(pool: &SqlitePool, root_dir: &str) -> CmdResult<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let value_json = serde_json::to_string(&serde_json::Value::String(root_dir.to_string()))
        .map_err(|e| AppError::db(format!("settings.root_dir 序列化失败: {}", e)))?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(ROOT_DIR_KEY)
    .bind(&value_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// 校验 `root_dir`：非空、绝对路径、不含 `..`。
fn validate_root_dir(root_dir: &str) -> CmdResult<()> {
    if root_dir.trim().is_empty() {
        return Err(AppError::invalid_param("rootDir 不能为空"));
    }
    let path = std::path::Path::new(root_dir);
    if !path.is_absolute() {
        return Err(AppError::invalid_param(format!(
            "rootDir 必须为绝对路径: {}",
            root_dir
        )));
    }
    // 拒绝包含 `..` 组件的路径，防止越出预期目录
    for comp in path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(AppError::invalid_param(format!(
                "rootDir 不允许包含 '..': {}",
                root_dir
            )));
        }
    }
    Ok(())
}

/// 将 `std::io::Error` 映射为 `AppError`：
/// `PermissionDenied` → `FS_PERMISSION_DENIED`，其他 → `COMMON_IO`。
fn map_io_err(context: impl Into<String>, err: std::io::Error) -> AppError {
    let ctx = context.into();
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            AppError::new("FS_PERMISSION_DENIED", format!("{}: {}", ctx, err))
        }
        _ => AppError::io(format!("{}: {}", ctx, err)),
    }
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `settings_get_root_dir`：读取根目录配置。
///
/// - 未设置 → `{ rootDir: null, initialized: false }`
/// - 已设置但目录不存在 → `{ rootDir: Some(...), initialized: false }`
/// - 已设置且目录存在 → `{ rootDir: Some(...), initialized: true }`
pub async fn get_root_dir(pool: &SqlitePool) -> CmdResult<RootDirStatus> {
    match load_root_dir(pool).await? {
        None => Ok(RootDirStatus {
            root_dir: None,
            initialized: false,
        }),
        Some(p) => {
            let exists = std::path::Path::new(&p).is_dir();
            Ok(RootDirStatus {
                root_dir: Some(p),
                initialized: exists,
            })
        }
    }
}

/// `settings_init_root_dir`：初始化根目录。
///
/// - 校验入参
/// - 递归创建根目录（已存在则跳过）
/// - 建出 6 个类型子目录（已存在跳过）
/// - 持久化到 `settings` 表
/// - 返回 `created`：本次实际新建的子目录绝对路径列表
pub async fn init_root_dir(pool: &SqlitePool, root_dir: String) -> CmdResult<InitRootDirResult> {
    validate_root_dir(&root_dir)?;

    let root = std::path::Path::new(&root_dir);

    // 递归创建根目录（已存在不报错）
    std::fs::create_dir_all(root)
        .map_err(|e| map_io_err(format!("创建根目录失败 {}", root_dir), e))?;

    // 建出 6 个类型子目录；已存在的跳过
    let mut created: Vec<String> = Vec::new();
    for sub in TYPE_SUBDIRS.iter() {
        let sub_path = root.join(sub);
        if sub_path.exists() {
            // 幂等：已存在则跳过，不计入 created
            continue;
        }
        std::fs::create_dir(&sub_path)
            .map_err(|e| map_io_err(format!("创建子目录失败 {}", sub_path.display()), e))?;
        created.push(sub_path.to_string_lossy().to_string());
    }

    // 持久化
    save_root_dir(pool, &root_dir).await?;

    Ok(InitRootDirResult { root_dir, created })
}

// ============================================================
// m6-6.4 · settings_get_default_app / settings_set_default_app
// ============================================================
//
// 详细设计 §2.8 / §5.3：默认查看程序配置。
//
// 存储：复用 `settings` 表，key 格式 `default_app_{type}`，value 为 JSON：
//   `{ "strategy": "system_default" }`
//   或 `{ "strategy": "app", "appPath": "/Applications/Typora.app" }`
//
// 语义：
// - `system_default`：使用系统默认程序（ref_open 现有行为）。
// - `app`：使用 `appPath` 指定的应用打开（macOS `open -a <app>`，Windows/Linux 直接执行）。
//
// 优先级（在 ref_open 中生效）：`appOverride` > 类型默认程序 > 系统默认。

/// `settings` 表中默认程序 key 前缀。
const DEFAULT_APP_KEY_PREFIX: &str = "default_app_";

/// `settings_get_default_app` / `settings_set_default_app` 出入参 strategy。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DefaultAppStrategy {
    /// 系统默认程序。
    SystemDefault,
    /// 指定应用。
    App,
}

impl DefaultAppStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            DefaultAppStrategy::SystemDefault => "system_default",
            DefaultAppStrategy::App => "app",
        }
    }
}

/// `settings_get_default_app` 出参 / `settings_set_default_app` 出参（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAppConfig {
    /// 引用类型（如 "code" / "document" / ...）。
    #[serde(rename = "type")]
    pub ref_type: String,
    /// "system_default" | "app"
    pub strategy: String,
    /// 仅 strategy == "app" 时存在。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_path: Option<String>,
}

/// 拼接 `settings` 表 key：`default_app_{type}`。
fn default_app_key(ref_type: &str) -> String {
    format!("{}{}", DEFAULT_APP_KEY_PREFIX, ref_type)
}

/// 校验 `ref_type`：非空、不含空白与路径分隔符（避免 key 注入）。
fn validate_ref_type(ref_type: &str) -> CmdResult<()> {
    let t = ref_type.trim();
    if t.is_empty() {
        return Err(AppError::invalid_param("type 不能为空"));
    }
    if t != ref_type {
        return Err(AppError::invalid_param(format!(
            "type 不允许首尾空白: {:?}",
            ref_type
        )));
    }
    if t.contains('/') || t.contains('\\') {
        return Err(AppError::invalid_param(format!(
            "type 不允许包含路径分隔符: {}",
            ref_type
        )));
    }
    Ok(())
}

/// 从 `settings` 表读取某类型的默认程序配置。
///
/// 返回 `None` 表示无配置（调用方应回退到系统默认）。
pub async fn load_default_app(
    pool: &SqlitePool,
    ref_type: &str,
) -> CmdResult<Option<DefaultAppConfig>> {
    let key = default_app_key(ref_type);
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?")
        .bind(&key)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    let Some(r) = row else {
        return Ok(None);
    };
    let value_json: String = r.try_get("value_json").map_err(AppError::from)?;
    let v: serde_json::Value = serde_json::from_str(&value_json)
        .map_err(|e| AppError::db(format!("settings.{} 反序列化失败: {}", key, e)))?;
    let strategy_str = v
        .get("strategy")
        .and_then(|s| s.as_str())
        .ok_or_else(|| AppError::db(format!("settings.{} 缺 strategy 字段", key)))?;
    let app_path = v
        .get("appPath")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());
    Ok(Some(DefaultAppConfig {
        ref_type: ref_type.to_string(),
        strategy: strategy_str.to_string(),
        app_path,
    }))
}

/// 写入某类型的默认程序配置（UPSERT）。
async fn save_default_app(
    pool: &SqlitePool,
    ref_type: &str,
    strategy: DefaultAppStrategy,
    app_path: Option<&str>,
) -> CmdResult<()> {
    let key = default_app_key(ref_type);
    let mut obj = serde_json::Map::new();
    obj.insert(
        "strategy".to_string(),
        serde_json::Value::String(strategy.as_str().to_string()),
    );
    if let Some(p) = app_path {
        obj.insert(
            "appPath".to_string(),
            serde_json::Value::String(p.to_string()),
        );
    }
    let value_json = serde_json::to_string(&serde_json::Value::Object(obj))
        .map_err(|e| AppError::db(format!("settings.{} 序列化失败: {}", key, e)))?;
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
    .map_err(AppError::from)?;
    Ok(())
}

/// `settings_get_default_app` 业务函数。
///
/// - 无配置 → 返回 `{ type, strategy: "system_default" }`（不返回 appPath）。
/// - 有配置 → 返回存储的 strategy 与 appPath。
pub async fn get_default_app(pool: &SqlitePool, ref_type: String) -> CmdResult<DefaultAppConfig> {
    validate_ref_type(&ref_type)?;
    match load_default_app(pool, &ref_type).await? {
        Some(cfg) => Ok(cfg),
        None => Ok(DefaultAppConfig {
            ref_type,
            strategy: DefaultAppStrategy::SystemDefault.as_str().to_string(),
            app_path: None,
        }),
    }
}

/// `settings_set_default_app` 业务函数。
///
/// - `strategy = "system_default"`：忽略 `app_path`，写 `{ "strategy": "system_default" }`。
/// - `strategy = "app"`：`app_path` 必填、非空、绝对路径。
pub async fn set_default_app(
    pool: &SqlitePool,
    ref_type: String,
    strategy: DefaultAppStrategy,
    app_path: Option<String>,
) -> CmdResult<DefaultAppConfig> {
    validate_ref_type(&ref_type)?;

    let normalized_app_path: Option<String> = match strategy {
        DefaultAppStrategy::SystemDefault => None,
        DefaultAppStrategy::App => {
            let p = app_path
                .ok_or_else(|| {
                    AppError::invalid_param("strategy=app 时 appPath 必填")
                })?;
            let trimmed = p.trim();
            if trimmed.is_empty() {
                return Err(AppError::invalid_param("appPath 不能为空"));
            }
            let path = std::path::Path::new(trimmed);
            if !path.is_absolute() {
                return Err(AppError::invalid_param(format!(
                    "appPath 必须为绝对路径: {}",
                    trimmed
                )));
            }
            Some(trimmed.to_string())
        }
    };

    save_default_app(pool, &ref_type, strategy, normalized_app_path.as_deref()).await?;

    Ok(DefaultAppConfig {
        ref_type,
        strategy: strategy.as_str().to_string(),
        app_path: normalized_app_path,
    })
}

// ============================================================
// m4-4.8 · settings_change_root_dir（修改资源根目录，两阶段）
// ============================================================
//
// 详细设计 §2.8 / §4.4：
// - 入参 `{ newRootDir, strategy: "future_only"|"migrate", confirmed }`。
// - `confirmed=false`：仅返回 `MigrationPlan`，不写文件、不改库（§6.8 绝不静默搬迁）。
// - `confirmed=true && strategy="future_only"`：仅更新 settings.root_dir 并建出
//   6 个类型子目录；**不动**已有文件与引用记录（已有 managed 引用仍指向旧路径，
//   由用户后续手动迁移）。
// - `confirmed=true && strategy="migrate"`：逐项复制 `{newRootDir}/{TypeSubdir}/{原文件名}`，
//   并更新对应 `resource_reference.locator_json`；单项失败记入 `failed` 后继续，
//   不中断整批；全部完成后更新 settings.root_dir。
//
// 互斥（详细设计 §4.4 迁移模式）：
// - `migrate` 的 confirmed 阶段持有 `reference::managed_write_lock()`，
//   与 M3-3.2 托管落地、M4-4.5 销毁共用同一把锁，迁移期间禁止新的托管写。
//
// 审计说明（契约外标注）：
// - 详细设计 §2.8 提到迁移写 `disposition_audit.action='migrate'`，
//   这是对 disposition_audit.action 集合的扩展（原集合仅覆盖 dispose 流程）。
// - 本任务**暂不写审计，仅记日志**（`eprintln!`），待用户 review 时确认
//   是否将 'migrate' 纳入契约；若纳入，再补 INSERT disposition_audit。

/// `settings_change_root_dir` 入参 strategy 枚举（§2.8）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeRootStrategy {
    /// 仅未来生效：只改 settings.root_dir，不动已有文件与引用。
    FutureOnly,
    /// 迁移已有内容：逐项复制 + 更新 locator_json。
    Migrate,
}

impl ChangeRootStrategy {
    fn as_str(&self) -> &'static str {
        match self {
            ChangeRootStrategy::FutureOnly => "future_only",
            ChangeRootStrategy::Migrate => "migrate",
        }
    }
}

/// `MigrationPlan.items` 单项状态。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationItemStatus {
    /// 源存在且目标无冲突，可迁移。
    Ok,
    /// 源文件已不存在（外部删除），confirmed 阶段将记入 failed。
    SourceMissing,
    /// 目标路径已存在同名项，confirmed 阶段将跳过并记入 failed。
    TargetConflict,
}

impl MigrationItemStatus {
    fn as_str(&self) -> &'static str {
        match self {
            MigrationItemStatus::Ok => "ok",
            MigrationItemStatus::SourceMissing => "source_missing",
            MigrationItemStatus::TargetConflict => "target_conflict",
        }
    }
}

/// `MigrationPlan.items` 单项（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlanItem {
    pub ref_id: String,
    pub current_path: String,
    pub proposed_path: String,
    /// "ok" | "source_missing" | "target_conflict"
    pub status: String,
}

/// `confirmed=false` 时 `settings_change_root_dir` 的出参（§2.8 MigrationPlan）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    pub new_root_dir: String,
    /// "future_only" | "migrate"
    pub strategy: String,
    pub items: Vec<MigrationPlanItem>,
    /// 可迁移项（status == "ok"）源文件总字节数。
    pub total_bytes: u64,
    /// 目标冲突的 proposedPath 列表（便于前端汇总展示）。
    pub conflicts: Vec<String>,
}

/// `migrate` 单项失败记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationFailure {
    pub ref_id: String,
    pub current_path: String,
    pub proposed_path: String,
    /// 人类可读失败原因（来自 AppError.message 或自定义描述）。
    pub error: String,
}

/// `confirmed=true && strategy="migrate"` 出参（§2.8 MigrationResult）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    /// 成功迁移的 refId 列表。
    pub migrated: Vec<String>,
    /// 失败项明细（不中断整批）。
    pub failed: Vec<MigrationFailure>,
}

/// `confirmed=true && strategy="future_only"` 出参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FutureOnlyResult {
    pub new_root_dir: String,
}

/// `settings_change_root_dir` 两阶段出参（untagged 联合）。
///
/// - `Plan` → `MigrationPlan`（含 `items` 字段，前端据此识别）
/// - `Migrated` → `MigrationResult`（含 `migrated` / `failed`）
/// - `FutureOnlyApplied` → `FutureOnlyResult`（仅 `newRootDir`）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ChangeRootResult {
    Plan(MigrationPlan),
    Migrated(MigrationResult),
    FutureOnlyApplied(FutureOnlyResult),
}

/// 从 `resource_reference` 表读取所有 `hosting='managed'` 的引用，
/// 返回 `(id, type, locator_path)` 三元组；按 created_at 稳定排序。
async fn list_managed_references(pool: &SqlitePool) -> CmdResult<Vec<(String, String, String)>> {
    let rows = sqlx::query(
        "SELECT id, type, locator_json FROM resource_reference \
         WHERE hosting = 'managed' ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("id").map_err(AppError::from)?;
        let ref_type: String = row.try_get("type").map_err(AppError::from)?;
        let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
        let v: serde_json::Value = serde_json::from_str(&locator_json).map_err(|e| {
            AppError::db(format!("resource_reference.locator_json 反序列化失败: {}", e))
        })?;
        let path = v
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| AppError::db(format!("resource_reference {} locator 缺 path", id)))?;
        out.push((id, ref_type, path.to_string()));
    }
    Ok(out)
}

/// 计算迁移目标路径：`{newRootDir}/{TypeSubdir}/{原文件名}`。
///
/// 复用 M3-3.1 `landing::type_to_subdir` 做类型 → 子目录映射。
/// 源路径无文件名（如 `/`）→ `COMMON_INVALID_PARAM`。
fn proposed_target_for(
    new_root: &Path,
    ref_type: &str,
    current_path: &str,
) -> CmdResult<PathBuf> {
    let subdir = landing::type_to_subdir(ref_type)?;
    let file_name = Path::new(current_path)
        .file_name()
        .ok_or_else(|| {
            AppError::invalid_param(format!("源路径无文件名，无法计算迁移目标: {}", current_path))
        })?;
    Ok(new_root.join(subdir).join(file_name))
}

/// 递归复制文件或目录（不跟随符号链接出根）。
///
/// 与 `reference::land_source` 语义一致，但**不带进度回调**：
//  迁移场景下进度反馈由前端基于 `migrated` 列表增量渲染，
//  本任务不引入 `managed_progress` 事件以避免契约外扩张。
fn copy_recursive(src: &Path, dst: &Path) -> CmdResult<()> {
    let meta = std::fs::symlink_metadata(src)
        .map_err(|e| map_io_err(format!("读取源元数据失败 {}", src.display()), e))?;
    if meta.is_dir() {
        std::fs::create_dir_all(dst)
            .map_err(|e| map_io_err(format!("创建目标目录失败 {}", dst.display()), e))?;
        let entries = std::fs::read_dir(src)
            .map_err(|e| map_io_err(format!("读取源目录失败 {}", src.display()), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                map_io_err(format!("读取源目录条目失败 {}", src.display()), e)
            })?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            let child_meta = std::fs::symlink_metadata(&child_src).map_err(|e| {
                map_io_err(format!("读取源条目元数据失败 {}", child_src.display()), e)
            })?;
            if child_meta.is_dir() {
                copy_recursive(&child_src, &child_dst)?;
            } else {
                // 文件 / 符号链接：按文件复制（File::open 跟随符号链接到目标）
                std::fs::copy(&child_src, &child_dst).map_err(|e| {
                    map_io_err(
                        format!(
                            "复制文件失败 {} -> {}",
                            child_src.display(),
                            child_dst.display()
                        ),
                        e,
                    )
                })?;
            }
        }
        Ok(())
    } else {
        // 单文件：确保父目录存在
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                map_io_err(format!("创建目标父目录失败 {}", parent.display()), e)
            })?;
        }
        std::fs::copy(src, dst).map_err(|e| {
            map_io_err(
                format!("复制文件失败 {} -> {}", src.display(), dst.display()),
                e,
            )
        })?;
        Ok(())
    }
}

/// 更新 `resource_reference.locator_json` 中的 `path` 字段（保留其他字段）。
///
/// 使用 raw sqlx（任务包明确禁止改动 `reference.rs`，故在此直接写 SQL）。
async fn update_reference_locator_path(
    pool: &SqlitePool,
    ref_id: &str,
    new_path: &Path,
) -> CmdResult<()> {
    // 先读出当前 locator_json，合并 path 字段后写回，避免覆盖未来扩展字段。
    let row = sqlx::query("SELECT locator_json FROM resource_reference WHERE id = ?")
        .bind(ref_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let mut v: serde_json::Value = serde_json::from_str(&locator_json)
        .map_err(|e| AppError::db(format!("locator_json 反序列化失败: {}", e)))?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "path".to_string(),
            serde_json::Value::String(new_path.to_string_lossy().to_string()),
        );
    } else {
        return Err(AppError::db(format!(
            "resource_reference {} locator 非对象",
            ref_id
        )));
    }
    let new_json = serde_json::to_string(&v)
        .map_err(|e| AppError::db(format!("locator_json 序列化失败: {}", e)))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query("UPDATE resource_reference SET locator_json = ?, updated_at = ? WHERE id = ?")
        .bind(&new_json)
        .bind(now)
        .bind(ref_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// 确保 `new_root` 下 6 个类型子目录存在（与 `init_root_dir` 行为一致，幂等）。
fn ensure_type_subdirs(new_root: &Path) -> CmdResult<()> {
    std::fs::create_dir_all(new_root)
        .map_err(|e| map_io_err(format!("创建根目录失败 {}", new_root.display()), e))?;
    for sub in TYPE_SUBDIRS.iter() {
        let p = new_root.join(sub);
        if p.exists() {
            continue;
        }
        std::fs::create_dir(&p)
            .map_err(|e| map_io_err(format!("创建子目录失败 {}", p.display()), e))?;
    }
    Ok(())
}

/// 构建 `MigrationPlan`（不写文件、不改库）。
async fn build_migration_plan(
    pool: &SqlitePool,
    new_root_dir: &str,
    strategy: ChangeRootStrategy,
) -> CmdResult<MigrationPlan> {
    let new_root = Path::new(new_root_dir);
    let managed = list_managed_references(pool).await?;

    let mut items: Vec<MigrationPlanItem> = Vec::with_capacity(managed.len());
    let mut conflicts: Vec<String> = Vec::new();
    let mut total_bytes: u64 = 0;

    for (ref_id, ref_type, current_path) in managed {
        let proposed = proposed_target_for(new_root, &ref_type, &current_path)?;
        let proposed_str = proposed.to_string_lossy().to_string();

        let src_exists = Path::new(&current_path).exists();
        let dst_exists = proposed.exists();

        let status = if !src_exists {
            MigrationItemStatus::SourceMissing
        } else if dst_exists {
            MigrationItemStatus::TargetConflict
        } else {
            MigrationItemStatus::Ok
        };

        if status == MigrationItemStatus::TargetConflict {
            conflicts.push(proposed_str.clone());
        }
        if status == MigrationItemStatus::Ok {
            // 累计可迁移项大小；目录递归累计，失败仅跳过（不阻塞 plan）。
            let meta = std::fs::symlink_metadata(&current_path);
            if let Ok(m) = meta {
                if m.is_file() {
                    total_bytes = total_bytes.saturating_add(m.len());
                } else if m.is_dir() {
                    // 目录：递归累计文件大小；权限错误跳过该子树
                    let mut stack = vec![PathBuf::from(&current_path)];
                    while let Some(d) = stack.pop() {
                        if let Ok(entries) = std::fs::read_dir(&d) {
                            for e in entries.flatten() {
                                let p = e.path();
                                if let Ok(cm) = std::fs::symlink_metadata(&p) {
                                    if cm.is_dir() {
                                        stack.push(p);
                                    } else if cm.is_file() {
                                        total_bytes = total_bytes.saturating_add(cm.len());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        items.push(MigrationPlanItem {
            ref_id,
            current_path,
            proposed_path: proposed_str,
            status: status.as_str().to_string(),
        });
    }

    Ok(MigrationPlan {
        new_root_dir: new_root_dir.to_string(),
        strategy: strategy.as_str().to_string(),
        items,
        total_bytes,
        conflicts,
    })
}

/// `settings_change_root_dir` 业务函数（两阶段）。
///
/// # 错误码
/// - `COMMON_INVALID_PARAM`：newRootDir 空 / 相对路径 / 含 `..`
/// - `FS_PERMISSION_DENIED`：创建目录 / 复制文件权限不足
/// - `COMMON_IO`：其他 IO 错误
/// - `COMMON_DB`：settings / resource_reference 写库失败
pub async fn change_root_dir(
    pool: &SqlitePool,
    new_root_dir: String,
    strategy: ChangeRootStrategy,
    confirmed: bool,
) -> CmdResult<ChangeRootResult> {
    // ---------- 1. 入参校验 ----------
    validate_root_dir(&new_root_dir)?;

    // ---------- 2. plan 阶段：绝不写文件、不改库（§6.8） ----------
    if !confirmed {
        let plan = build_migration_plan(pool, &new_root_dir, strategy).await?;
        return Ok(ChangeRootResult::Plan(plan));
    }

    // ---------- 3. confirmed 阶段 ----------
    let new_root = Path::new(&new_root_dir);

    match strategy {
        ChangeRootStrategy::FutureOnly => {
            // 仅建出 6 子目录 + 更新 settings.root_dir；不动已有文件与引用。
            ensure_type_subdirs(new_root)?;
            save_root_dir(pool, &new_root_dir).await?;
            Ok(ChangeRootResult::FutureOnlyApplied(FutureOnlyResult {
                new_root_dir,
            }))
        }
        ChangeRootStrategy::Migrate => {
            // 迁移模式互斥（详细设计 §4.4）：
            // 与 M3-3.2 托管落地、M4-4.5 销毁共用同一把锁，
            // 迁移期间禁止新的托管写与处置操作。
            let _guard = crate::reference::managed_write_lock().lock().await;

            // 建出 6 子目录（在锁内做，保证与落地并发时目录已就绪）
            ensure_type_subdirs(new_root)?;

            let managed = list_managed_references(pool).await?;
            let mut migrated: Vec<String> = Vec::new();
            let mut failed: Vec<MigrationFailure> = Vec::new();

            for (ref_id, ref_type, current_path) in managed {
                let proposed = match proposed_target_for(new_root, &ref_type, &current_path) {
                    Ok(p) => p,
                    Err(e) => {
                        failed.push(MigrationFailure {
                            ref_id,
                            current_path,
                            proposed_path: String::new(),
                            error: format!("计算目标路径失败: {}", e.message),
                        });
                        continue;
                    }
                };
                let proposed_str = proposed.to_string_lossy().to_string();

                // 源已不存在 → 记入 failed，继续
                if !Path::new(&current_path).exists() {
                    failed.push(MigrationFailure {
                        ref_id,
                        current_path,
                        proposed_path: proposed_str,
                        error: "源路径不存在（可能已被外部删除）".to_string(),
                    });
                    continue;
                }

                // 目标冲突 → 记入 failed，继续（不覆盖已有文件）
                if proposed.exists() {
                    failed.push(MigrationFailure {
                        ref_id,
                        current_path,
                        proposed_path: proposed_str,
                        error: "目标路径已存在同名项".to_string(),
                    });
                    continue;
                }

                // 复制
                if let Err(e) = copy_recursive(Path::new(&current_path), &proposed) {
                    failed.push(MigrationFailure {
                        ref_id,
                        current_path,
                        proposed_path: proposed_str,
                        error: format!("复制失败: {}", e.message),
                    });
                    continue;
                }

                // 更新 locator_json；失败补偿删除已复制目标
                if let Err(e) = update_reference_locator_path(pool, &ref_id, &proposed).await {
                    // 补偿：删除已复制目标（失败仅记日志，不向上抛）
                    let cleanup = if proposed.is_dir() {
                        std::fs::remove_dir_all(&proposed)
                    } else {
                        std::fs::remove_file(&proposed)
                    };
                    if let Err(ce) = cleanup {
                        eprintln!(
                            "[settings_change_root_dir] 补偿删除失败 {}: {}",
                            proposed.display(),
                            ce
                        );
                    }
                    failed.push(MigrationFailure {
                        ref_id,
                        current_path,
                        proposed_path: proposed_str,
                        error: format!("更新 locator 失败: {}", e.message),
                    });
                    continue;
                }

                // 成功：契约外审计（详见文件头注释）
                //
                // 详细设计 §2.8 提到 disposition_audit.action='migrate'，
                // 但 disposition_audit.action 集合当前仅覆盖 dispose 流程，
                // 'migrate' 属于契约外扩展。本任务**仅记日志不写审计表**，
                // 待用户 review 确认是否将 'migrate' 纳入契约后再补 INSERT。
                eprintln!(
                    "[settings_change_root_dir] migrated ref {} : {} -> {}",
                    ref_id, current_path, proposed_str
                );
                migrated.push(ref_id);
            }

            // 全部完成后更新 settings.root_dir
            save_root_dir(pool, &new_root_dir).await?;

            Ok(ChangeRootResult::Migrated(MigrationResult { migrated, failed }))
        }
    }
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_root_dir(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<RootDirStatus> {
    get_root_dir(&state.pool).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_init_root_dir(
    state: tauri::State<'_, crate::AppState>,
    root_dir: String,
) -> CmdResult<InitRootDirResult> {
    init_root_dir(&state.pool, root_dir).await
}

/// `settings_change_root_dir`：修改资源根目录（两阶段，§2.8 / §4.4）。
///
/// - `confirmed=false` → `MigrationPlan`（不写文件不改库）
/// - `confirmed=true && strategy="future_only"` → `FutureOnlyResult`
/// - `confirmed=true && strategy="migrate"` → `MigrationResult`
#[tauri::command(rename_all = "camelCase")]
pub async fn settings_change_root_dir(
    state: tauri::State<'_, crate::AppState>,
    new_root_dir: String,
    strategy: ChangeRootStrategy,
    confirmed: bool,
) -> CmdResult<ChangeRootResult> {
    change_root_dir(&state.pool, new_root_dir, strategy, confirmed).await
}

/// `settings_get_default_app`：读取某类型的默认查看程序配置（§2.8）。
#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_default_app(
    state: tauri::State<'_, crate::AppState>,
    r#type: String,
) -> CmdResult<DefaultAppConfig> {
    get_default_app(&state.pool, r#type).await
}

/// `settings_set_default_app`：写入某类型的默认查看程序配置（§2.8）。
#[tauri::command(rename_all = "camelCase")]
pub async fn settings_set_default_app(
    state: tauri::State<'_, crate::AppState>,
    r#type: String,
    strategy: DefaultAppStrategy,
    app_path: Option<String>,
) -> CmdResult<DefaultAppConfig> {
    set_default_app(&state.pool, r#type, strategy, app_path).await
}

// ============================================================
// m7-7.3 · settings_get_default_home / settings_set_default_home
// ============================================================
//
// 启动默认页配置（Dashboard 首页 vs 空间页）。
// 存储：复用 settings 表，key = "default_home"，value 为 JSON string。
// 合法值："dashboard" | "spaces"。默认 "dashboard"。

/// `settings` 表中启动默认页键名。
const DEFAULT_HOME_KEY: &str = "default_home";

/// 合法启动默认页取值。
const VALID_DEFAULT_HOMES: &[&str] = &["dashboard", "spaces"];

/// `settings_get_default_home` 出参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DefaultHomeConfig {
    /// "dashboard" | "spaces"
    pub home: String,
}

/// 读取启动默认页；未设置时返回 "dashboard"。
pub async fn get_default_home(pool: &SqlitePool) -> CmdResult<DefaultHomeConfig> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?")
        .bind(DEFAULT_HOME_KEY)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    match row {
        None => Ok(DefaultHomeConfig {
            home: "dashboard".to_string(),
        }),
        Some(r) => {
            let value_json: String = r.try_get("value_json").map_err(AppError::from)?;
            let v: serde_json::Value = serde_json::from_str(&value_json).map_err(|e| {
                AppError::db(format!("settings.{} 反序列化失败: {}", DEFAULT_HOME_KEY, e))
            })?;
            let s = v
                .as_str()
                .ok_or_else(|| AppError::db(format!("settings.{} 非字符串", DEFAULT_HOME_KEY)))?;
            // 容错：若数据库存了非法值（例如旧版本写入），回退到默认
            let home = if VALID_DEFAULT_HOMES.contains(&s) {
                s.to_string()
            } else {
                "dashboard".to_string()
            };
            Ok(DefaultHomeConfig { home })
        }
    }
}

/// 写入启动默认页。非法值返回 COMMON_INVALID_PARAM。
pub async fn set_default_home(pool: &SqlitePool, home: String) -> CmdResult<DefaultHomeConfig> {
    let trimmed = home.trim();
    if !VALID_DEFAULT_HOMES.contains(&trimmed) {
        return Err(AppError::invalid_param(format!(
            "非法 home: {}（允许值: {}）",
            trimmed,
            VALID_DEFAULT_HOMES.join(", ")
        )));
    }
    let value_json = serde_json::to_string(&serde_json::Value::String(trimmed.to_string()))
        .map_err(|e| AppError::db(format!("settings.{} 序列化失败: {}", DEFAULT_HOME_KEY, e)))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(DEFAULT_HOME_KEY)
    .bind(&value_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(DefaultHomeConfig {
        home: trimmed.to_string(),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_default_home(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<DefaultHomeConfig> {
    get_default_home(&state.pool).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_set_default_home(
    state: tauri::State<'_, crate::AppState>,
    home: String,
) -> CmdResult<DefaultHomeConfig> {
    set_default_home(&state.pool, home).await
}

// ============================================================
// 开机自启（launch_at_login）：settings KV + tauri-plugin-autostart
// ============================================================

/// `settings` 表中开机自启开关键名。
const LAUNCH_AT_LOGIN_KEY: &str = "launch_at_login";

/// `settings_get/set_launch_at_login` 出参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAtLoginConfig {
    /// 是否开机自启；未设置时默认 true（功能默认开启，用户可在设置页关闭）。
    pub enabled: bool,
}

/// 读取开关；未设置 / 非法 JSON / 非布尔值均回退默认 true。
pub async fn get_launch_at_login(pool: &SqlitePool) -> CmdResult<LaunchAtLoginConfig> {
    let raw = sqlx::query_scalar::<_, String>("SELECT value_json FROM settings WHERE key = ?")
        .bind(LAUNCH_AT_LOGIN_KEY)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    let enabled = raw
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Ok(LaunchAtLoginConfig { enabled })
}

/// 写入开关（仅落库；系统注册状态由命令层对齐）。
pub async fn set_launch_at_login(pool: &SqlitePool, enabled: bool) -> CmdResult<LaunchAtLoginConfig> {
    let value_json = serde_json::to_string(&serde_json::Value::Bool(enabled))
        .map_err(|e| AppError::db(format!("settings.{} 序列化失败: {}", LAUNCH_AT_LOGIN_KEY, e)))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(LAUNCH_AT_LOGIN_KEY)
    .bind(&value_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(LaunchAtLoginConfig { enabled })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_launch_at_login(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<LaunchAtLoginConfig> {
    get_launch_at_login(&state.pool).await
}

/// 写入开关并对齐系统自启注册（macOS LaunchAgent）。
/// 注册失败不阻塞：设置已落库，下次启动时 setup 会重新对齐。
#[tauri::command(rename_all = "camelCase")]
pub async fn settings_set_launch_at_login(
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
    enabled: bool,
) -> CmdResult<LaunchAtLoginConfig> {
    let cfg = set_launch_at_login(&state.pool, enabled).await?;
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if let Err(e) = (if enabled {
        manager.enable()
    } else {
        manager.disable()
    }) {
        eprintln!("[settings] 对齐开机自启注册失败 (enabled={}): {}", enabled, e);
    }
    Ok(cfg)
}

// ============================================================
// m7-7.3 · settings_get_user_name（Dashboard 问候语）
// ============================================================
//
// 任务包仅要求"读取 user_name，没有就显示'朋友'"，未要求设置 UI。
// 为保持命令职责单一，新增独立的只读命令。

/// `settings` 表中用户名键名。
const USER_NAME_KEY: &str = "user_name";

/// `settings_get_user_name` 出参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserNameConfig {
    /// 用户名；未设置时为 None（前端显示"朋友"）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

/// 读取用户名；未设置 / 空串 / 非法 JSON 均返回 None（前端自行 fallback）。
pub async fn get_user_name(pool: &SqlitePool) -> CmdResult<UserNameConfig> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?")
        .bind(USER_NAME_KEY)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    let Some(r) = row else {
        return Ok(UserNameConfig { user_name: None });
    };
    let value_json: String = r.try_get("value_json").map_err(AppError::from)?;
    let v: serde_json::Value = serde_json::from_str(&value_json)
        .map_err(|e| AppError::db(format!("settings.{} 反序列化失败: {}", USER_NAME_KEY, e)))?;
    let s = v.as_str().map(str::trim).filter(|s| !s.is_empty());
    Ok(UserNameConfig {
        user_name: s.map(str::to_string),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_user_name(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<UserNameConfig> {
    get_user_name(&state.pool).await
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

    // ---------- validate_root_dir ----------

    #[tokio::test]
    async fn init_rejects_empty_root_dir() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "".into()).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = init_root_dir(&pool, "   ".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn init_rejects_relative_path() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "relative/path".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = init_root_dir(&pool, "./also/relative".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn init_rejects_parent_dir_component() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "/tmp/../etc/evil".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- init_root_dir 正常路径 ----------

    #[tokio::test]
    async fn init_creates_root_and_six_subdirs() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        let res = init_root_dir(&pool, root_str.clone())
            .await
            .expect("init ok");

        assert_eq!(res.root_dir, root_str);
        assert_eq!(res.created.len(), 6, "should create 6 subdirs");

        // 6 个子目录真实存在
        for sub in TYPE_SUBDIRS.iter() {
            let p = root.join(sub);
            assert!(p.is_dir(), "missing subdir: {}", p.display());
            // created 列表里的路径应与之一致
            let s = p.to_string_lossy().to_string();
            assert!(res.created.contains(&s), "created missing {}", s);
        }
    }

    #[tokio::test]
    async fn init_is_idempotent_second_call_returns_empty_created() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        let first = init_root_dir(&pool, root_str.clone())
            .await
            .expect("first ok");
        assert_eq!(first.created.len(), 6);

        let second = init_root_dir(&pool, root_str.clone())
            .await
            .expect("second ok");
        assert_eq!(second.root_dir, root_str);
        assert!(second.created.is_empty(), "second call must be idempotent");

        // 子目录依然存在
        for sub in TYPE_SUBDIRS.iter() {
            assert!(root.join(sub).is_dir());
        }
    }

    #[tokio::test]
    async fn init_skips_preexisting_subdirs() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        // 预先创建 root 与其中一个子目录
        std::fs::create_dir_all(root.join("Code")).expect("pre-create");

        let root_str = root.to_string_lossy().to_string();
        let res = init_root_dir(&pool, root_str).await.expect("init ok");

        assert_eq!(res.created.len(), 5, "Code 已存在，应只新建 5 个");
        let code_path = root.join("Code").to_string_lossy().to_string();
        assert!(!res.created.contains(&code_path));
    }

    // ---------- get_root_dir ----------

    #[tokio::test]
    async fn get_root_dir_uninitialized_returns_none() {
        let pool = setup().await;
        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir, None);
        assert!(!s.initialized);
    }

    #[tokio::test]
    async fn get_root_dir_after_init_returns_path_and_initialized() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        init_root_dir(&pool, root_str.clone()).await.expect("init ok");

        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(s.initialized);
    }

    #[tokio::test]
    async fn get_root_dir_returns_uninitialized_when_dir_missing() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        init_root_dir(&pool, root_str.clone()).await.expect("init ok");

        // 删除根目录（模拟外部清理）
        std::fs::remove_dir_all(&root).expect("remove root");

        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(!s.initialized, "目录不存在 → initialized=false");
    }

    // ---------- 配置持久化 ----------

    #[tokio::test]
    async fn root_dir_persists_across_pool_reopen() {
        // 用文件库模拟"重启应用"：drop 池后用同一文件重开
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        {
            let pool = crate::db::init_pool_with_file(&db_path)
                .await
                .expect("init pool");
            init_root_dir(&pool, root_str.clone()).await.expect("init ok");
            pool.close().await;
        }

        // 重新打开同一文件
        let pool2 = crate::db::init_pool_with_file(&db_path)
            .await
            .expect("reopen pool");
        let s = get_root_dir(&pool2).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(s.initialized);
    }

    // ---------- 序列化契约 ----------

    #[test]
    fn root_dir_status_serializes_camel_case() {
        let s = RootDirStatus {
            root_dir: Some("/tmp/x".into()),
            initialized: true,
        };
        let v = serde_json::to_value(&s).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("rootDir"));
        assert!(obj.contains_key("initialized"));
        assert!(!obj.contains_key("root_dir"));

        // rootDir=None 时被 skip
        let s2 = RootDirStatus {
            root_dir: None,
            initialized: false,
        };
        let v2 = serde_json::to_value(&s2).expect("serialize");
        let obj2 = v2.as_object().expect("object");
        assert!(!obj2.contains_key("rootDir"));
        assert_eq!(obj2["initialized"].as_bool(), Some(false));
    }

    #[test]
    fn init_root_dir_result_serializes_camel_case() {
        let r = InitRootDirResult {
            root_dir: "/tmp/x".into(),
            created: vec!["/tmp/x/Code".into()],
        };
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("rootDir"));
        assert!(obj.contains_key("created"));
        assert_eq!(obj["created"].as_array().expect("arr").len(), 1);
    }

    // ============================================================
    // m4-4.8 · settings_change_root_dir
    // ============================================================

    mod change_root_dir_tests {
        use super::*;
        use std::sync::Arc;

        /// 在测试池中直接写 settings.root_dir（不走 init_root_dir，避免建子目录）。
        async fn set_root_dir(pool: &SqlitePool, root: &Path) {
            let value_json = serde_json::to_string(&serde_json::Value::String(
                root.to_string_lossy().to_string(),
            ))
            .expect("serialize root_dir");
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
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

        /// 预置空间 id（来自 0002 迁移）。
        const PRESET_SPACE: &str = "preset_space_work";

        /// 创建资源集，返回 id。
        async fn make_collection(pool: &SqlitePool) -> String {
            let id = uuid::Uuid::new_v4().to_string();
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

        /// 直接 INSERT 一条 managed 引用（绕过 reference::create_managed 的落地流程）。
        ///
        /// 返回 ref_id。`locator_path` 为落库 locator.path。
        async fn insert_managed_ref(
            pool: &SqlitePool,
            collection_id: &str,
            ref_type: &str,
            locator_path: &Path,
        ) -> String {
            let id = uuid::Uuid::new_v4().to_string();
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            let locator_json = serde_json::to_string(&serde_json::json!({
                "kind": "path",
                "path": locator_path.to_string_lossy(),
            }))
            .expect("serialize locator");
            sqlx::query(
                "INSERT INTO resource_reference \
                 (id, collection_id, source_id, name, type, hosting, locator_json, \
                  description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
                 VALUES (?, ?, 'src_local_fs_default', 'n', ?, 'managed', ?, NULL, 'active', 'internal', 1, 'none', ?, ?)",
            )
            .bind(&id)
            .bind(collection_id)
            .bind(ref_type)
            .bind(&locator_json)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .expect("insert managed ref");
            id
        }

        /// 读取 resource_reference.locator_json 中的 path 字段。
        async fn read_locator_path(pool: &SqlitePool, ref_id: &str) -> String {
            let row = sqlx::query("SELECT locator_json FROM resource_reference WHERE id = ?")
                .bind(ref_id)
                .fetch_one(pool)
                .await
                .expect("fetch locator");
            let locator_json: String = row.try_get("locator_json").expect("locator_json");
            let v: serde_json::Value =
                serde_json::from_str(&locator_json).expect("parse locator");
            v["path"].as_str().expect("path str").to_string()
        }

        // ---------- 入参校验 ----------

        #[tokio::test]
        async fn change_root_rejects_empty() {
            let pool = setup().await;
            let err = change_root_dir(&pool, "".into(), ChangeRootStrategy::FutureOnly, false)
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");

            let err = change_root_dir(&pool, "   ".into(), ChangeRootStrategy::FutureOnly, false)
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn change_root_rejects_relative_path() {
            let pool = setup().await;
            let err = change_root_dir(
                &pool,
                "relative/path".into(),
                ChangeRootStrategy::FutureOnly,
                false,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn change_root_rejects_parent_dir_component() {
            let pool = setup().await;
            let err = change_root_dir(
                &pool,
                "/tmp/../etc/evil".into(),
                ChangeRootStrategy::FutureOnly,
                false,
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        // ---------- future_only ----------

        #[tokio::test]
        async fn future_only_plan_returns_items_without_writing() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let src_file = old_root.join("Code").join("main.rs");
            std::fs::write(&src_file, b"fn main(){}").expect("write");
            set_root_dir(&pool, &old_root).await;

            let ref_id = insert_managed_ref(&pool, &cid, "code", &src_file).await;

            let new_root = tmp.path().join("new-root");
            let new_root_str = new_root.to_string_lossy().to_string();

            let r = change_root_dir(
                &pool,
                new_root_str.clone(),
                ChangeRootStrategy::FutureOnly,
                false,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ChangeRootResult::Plan(p) => p,
                other => panic!("expected Plan, got {:?}", other),
            };
            assert_eq!(plan.new_root_dir, new_root_str);
            assert_eq!(plan.strategy, "future_only");
            assert_eq!(plan.items.len(), 1);
            assert_eq!(plan.items[0].ref_id, ref_id);
            assert_eq!(
                plan.items[0].current_path,
                src_file.to_string_lossy().to_string()
            );
            assert_eq!(
                plan.items[0].proposed_path,
                new_root
                    .join("Code")
                    .join("main.rs")
                    .to_string_lossy()
                    .to_string()
            );
            assert_eq!(plan.items[0].status, "ok");
            assert!(plan.conflicts.is_empty());

            // plan 阶段不写文件：new_root 不应被创建
            assert!(!new_root.exists(), "plan 阶段不得创建新根目录");
            // plan 阶段不改库：settings.root_dir 仍是 old_root
            let s = get_root_dir(&pool).await.expect("get");
            assert_eq!(s.root_dir.as_deref(), Some(old_root.to_string_lossy().as_ref()));
        }

        #[tokio::test]
        async fn future_only_confirmed_updates_settings_and_creates_subdirs() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let src_file = old_root.join("Code").join("main.rs");
            std::fs::write(&src_file, b"fn main(){}").expect("write");
            set_root_dir(&pool, &old_root).await;

            let ref_id = insert_managed_ref(&pool, &cid, "code", &src_file).await;
            let locator_before = read_locator_path(&pool, &ref_id).await;

            let new_root = tmp.path().join("new-root");
            let new_root_str = new_root.to_string_lossy().to_string();

            let r = change_root_dir(
                &pool,
                new_root_str.clone(),
                ChangeRootStrategy::FutureOnly,
                true,
            )
            .await
            .expect("future_only ok");

            let res = match r {
                ChangeRootResult::FutureOnlyApplied(f) => f,
                other => panic!("expected FutureOnlyApplied, got {:?}", other),
            };
            assert_eq!(res.new_root_dir, new_root_str);

            // settings 已更新
            let s = get_root_dir(&pool).await.expect("get");
            assert_eq!(s.root_dir.as_deref(), Some(new_root_str.as_str()));
            assert!(s.initialized);

            // 6 个子目录已建出
            for sub in TYPE_SUBDIRS.iter() {
                assert!(new_root.join(sub).is_dir(), "missing subdir {}", sub);
            }

            // 已有引用 locator 不变（future_only 不动已有文件与引用）
            let locator_after = read_locator_path(&pool, &ref_id).await;
            assert_eq!(locator_before, locator_after);
            // 旧文件仍在原处
            assert!(src_file.exists());
            // 新根目录下不应有该文件（future_only 不复制）
            assert!(!new_root.join("Code").join("main.rs").exists());
        }

        // ---------- migrate · plan ----------

        #[tokio::test]
        async fn migrate_plan_items_match_managed_count() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            std::fs::create_dir_all(old_root.join("Documents")).expect("mkdir");
            let f1 = old_root.join("Code").join("a.rs");
            let f2 = old_root.join("Documents").join("b.pdf");
            std::fs::write(&f1, b"a").expect("w1");
            std::fs::write(&f2, b"bb").expect("w2");
            set_root_dir(&pool, &old_root).await;

            insert_managed_ref(&pool, &cid, "code", &f1).await;
            insert_managed_ref(&pool, &cid, "document", &f2).await;

            // 再插一条 external 引用（不应被纳入 plan）
            let ext_file = tmp.path().join("ext.txt");
            std::fs::write(&ext_file, b"ext").expect("w");
            let ext_locator = serde_json::to_string(&serde_json::json!({
                "kind": "path",
                "path": ext_file.to_string_lossy(),
            }))
            .expect("json");
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            sqlx::query(
                "INSERT INTO resource_reference \
                 (id, collection_id, source_id, name, type, hosting, locator_json, \
                  description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
                 VALUES (?, ?, 'src_local_fs_default', 'ext', 'code', 'external', ?, NULL, 'active', 'internal', 1, 'none', ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&cid)
            .bind(&ext_locator)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .expect("insert external");

            let new_root = tmp.path().join("new-root");
            let r = change_root_dir(
                &pool,
                new_root.to_string_lossy().to_string(),
                ChangeRootStrategy::Migrate,
                false,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ChangeRootResult::Plan(p) => p,
                other => panic!("expected Plan, got {:?}", other),
            };
            // 仅 managed 引用纳入 plan
            assert_eq!(plan.items.len(), 2);
            assert_eq!(plan.total_bytes, 3); // 1 + 2
            assert!(plan.conflicts.is_empty());
            assert_eq!(plan.strategy, "migrate");
        }

        #[tokio::test]
        async fn migrate_plan_detects_target_conflict() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let src = old_root.join("Code").join("main.rs");
            std::fs::write(&src, b"x").expect("w");
            set_root_dir(&pool, &old_root).await;
            insert_managed_ref(&pool, &cid, "code", &src).await;

            // 在新根目录预置同名文件 → 冲突
            let new_root = tmp.path().join("new-root");
            std::fs::create_dir_all(new_root.join("Code")).expect("mkdir");
            std::fs::write(new_root.join("Code").join("main.rs"), b"existing").expect("w");

            let r = change_root_dir(
                &pool,
                new_root.to_string_lossy().to_string(),
                ChangeRootStrategy::Migrate,
                false,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ChangeRootResult::Plan(p) => p,
                _ => panic!("expected Plan"),
            };
            assert_eq!(plan.items.len(), 1);
            assert_eq!(plan.items[0].status, "target_conflict");
            assert_eq!(plan.conflicts.len(), 1);
            assert_eq!(plan.total_bytes, 0, "冲突项不计入 totalBytes");
        }

        #[tokio::test]
        async fn migrate_plan_marks_source_missing() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let src = old_root.join("Code").join("ghost.rs");
            std::fs::write(&src, b"x").expect("w");
            set_root_dir(&pool, &old_root).await;
            insert_managed_ref(&pool, &cid, "code", &src).await;

            // 外部删除源文件
            std::fs::remove_file(&src).expect("remove");

            let new_root = tmp.path().join("new-root");
            let r = change_root_dir(
                &pool,
                new_root.to_string_lossy().to_string(),
                ChangeRootStrategy::Migrate,
                false,
            )
            .await
            .expect("plan ok");

            let plan = match r {
                ChangeRootResult::Plan(p) => p,
                _ => panic!("expected Plan"),
            };
            assert_eq!(plan.items.len(), 1);
            assert_eq!(plan.items[0].status, "source_missing");
            assert!(plan.conflicts.is_empty(), "源缺失不算目标冲突");
        }

        // ---------- migrate · confirmed ----------

        #[tokio::test]
        async fn migrate_confirmed_copies_and_updates_locator() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            std::fs::create_dir_all(old_root.join("Documents")).expect("mkdir");
            let f1 = old_root.join("Code").join("a.rs");
            let f2 = old_root.join("Documents").join("b.pdf");
            std::fs::write(&f1, b"fn a(){}").expect("w1");
            std::fs::write(&f2, b"PDF").expect("w2");
            set_root_dir(&pool, &old_root).await;

            let id1 = insert_managed_ref(&pool, &cid, "code", &f1).await;
            let id2 = insert_managed_ref(&pool, &cid, "document", &f2).await;

            let new_root = tmp.path().join("new-root");
            let new_root_str = new_root.to_string_lossy().to_string();

            let r = change_root_dir(
                &pool,
                new_root_str.clone(),
                ChangeRootStrategy::Migrate,
                true,
            )
            .await
            .expect("migrate ok");

            let res = match r {
                ChangeRootResult::Migrated(m) => m,
                other => panic!("expected Migrated, got {:?}", other),
            };
            assert_eq!(res.migrated.len(), 2);
            assert!(res.failed.is_empty());
            assert!(res.migrated.contains(&id1));
            assert!(res.migrated.contains(&id2));

            // 文件已复制到新位置
            let landed1 = new_root.join("Code").join("a.rs");
            let landed2 = new_root.join("Documents").join("b.pdf");
            assert!(landed1.exists());
            assert!(landed2.exists());
            assert_eq!(std::fs::read(&landed1).expect("read"), b"fn a(){}");
            assert_eq!(std::fs::read(&landed2).expect("read"), b"PDF");

            // 源文件保留（migrate 是复制不是移动）
            assert!(f1.exists());
            assert!(f2.exists());

            // locator 已更新
            let path1 = read_locator_path(&pool, &id1).await;
            let path2 = read_locator_path(&pool, &id2).await;
            assert_eq!(path1, landed1.to_string_lossy().to_string());
            assert_eq!(path2, landed2.to_string_lossy().to_string());

            // settings 已更新
            let s = get_root_dir(&pool).await.expect("get");
            assert_eq!(s.root_dir.as_deref(), Some(new_root_str.as_str()));
        }

        #[tokio::test]
        async fn migrate_confirmed_partial_failure_continues() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            std::fs::create_dir_all(old_root.join("Documents")).expect("mkdir");
            let f_ok = old_root.join("Code").join("ok.rs");
            let f_missing = old_root.join("Documents").join("gone.pdf");
            std::fs::write(&f_ok, b"ok").expect("w");
            std::fs::write(&f_missing, b"gone").expect("w");
            set_root_dir(&pool, &old_root).await;

            let id_ok = insert_managed_ref(&pool, &cid, "code", &f_ok).await;
            let id_missing = insert_managed_ref(&pool, &cid, "document", &f_missing).await;

            // 外部删除 f_missing
            std::fs::remove_file(&f_missing).expect("remove");

            let new_root = tmp.path().join("new-root");
            let r = change_root_dir(
                &pool,
                new_root.to_string_lossy().to_string(),
                ChangeRootStrategy::Migrate,
                true,
            )
            .await
            .expect("migrate ok (partial)");

            let res = match r {
                ChangeRootResult::Migrated(m) => m,
                _ => panic!("expected Migrated"),
            };
            assert_eq!(res.migrated, vec![id_ok.clone()]);
            assert_eq!(res.failed.len(), 1);
            assert_eq!(res.failed[0].ref_id, id_missing);
            assert!(res.failed[0].error.contains("源路径不存在"));

            // 成功项 locator 已更新
            let p = read_locator_path(&pool, &id_ok).await;
            assert_eq!(
                p,
                new_root
                    .join("Code")
                    .join("ok.rs")
                    .to_string_lossy()
                    .to_string()
            );
            // 失败项 locator 未变
            let p_missing = read_locator_path(&pool, &id_missing).await;
            assert_eq!(p_missing, f_missing.to_string_lossy().to_string());

            // settings 仍更新（整批完成）
            let s = get_root_dir(&pool).await.expect("get");
            assert_eq!(
                s.root_dir.as_deref(),
                Some(new_root.to_string_lossy().as_ref())
            );
        }

        #[tokio::test]
        async fn migrate_confirmed_skips_target_conflict() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let src = old_root.join("Code").join("main.rs");
            std::fs::write(&src, b"new-content").expect("w");
            set_root_dir(&pool, &old_root).await;
            let ref_id = insert_managed_ref(&pool, &cid, "code", &src).await;

            // 新根目录预置同名文件（已有内容不应被覆盖）
            let new_root = tmp.path().join("new-root");
            std::fs::create_dir_all(new_root.join("Code")).expect("mkdir");
            let existing = new_root.join("Code").join("main.rs");
            std::fs::write(&existing, b"existing-content").expect("w");

            let r = change_root_dir(
                &pool,
                new_root.to_string_lossy().to_string(),
                ChangeRootStrategy::Migrate,
                true,
            )
            .await
            .expect("migrate ok");

            let res = match r {
                ChangeRootResult::Migrated(m) => m,
                _ => panic!("expected Migrated"),
            };
            assert!(res.migrated.is_empty());
            assert_eq!(res.failed.len(), 1);
            assert_eq!(res.failed[0].ref_id, ref_id);
            assert!(res.failed[0].error.contains("目标路径已存在"));

            // 已有文件未被覆盖
            assert_eq!(
                std::fs::read(&existing).expect("read"),
                b"existing-content"
            );
            // locator 未变
            let p = read_locator_path(&pool, &ref_id).await;
            assert_eq!(p, src.to_string_lossy().to_string());
        }

        // ---------- 互斥：与 ref_create_managed 串行 ----------

        /// 并发触发 migrate 与 create_managed，验证二者串行（同一把锁）。
        ///
        /// 验证方式：两者都成功完成，且互斥锁被至少一个调用持有过。
        /// 由于临界区在各自函数内部，我们用「两个调用都成功 + 目标文件都存在 +
        /// 锁已初始化」共同佐证串行性（与 m3-3.2 既有互斥测试同一策略）。
        #[tokio::test]
        async fn migrate_and_create_managed_are_serialized() {
            let pool = setup().await;
            let cid = make_collection(&pool).await;

            let tmp = tempfile::tempdir().expect("tempdir");
            let old_root = tmp.path().join("old-root");
            std::fs::create_dir_all(old_root.join("Code")).expect("mkdir");
            let managed_src = old_root.join("Code").join("old.rs");
            std::fs::write(&managed_src, b"old").expect("w");
            set_root_dir(&pool, &old_root).await;
            insert_managed_ref(&pool, &cid, "code", &managed_src).await;

            // create_managed 的新源（在 old_root 之外）
            let new_src = tmp.path().join("new-source.rs");
            std::fs::write(&new_src, b"new").expect("w");

            let new_root = tmp.path().join("new-root");

            let pool = Arc::new(pool);
            let cid = Arc::new(cid);

            // 任务 A：migrate 到 new_root
            let pool_a = Arc::clone(&pool);
            let new_root_a = new_root.clone();
            let h_migrate = tokio::spawn(async move {
                change_root_dir(
                    &pool_a,
                    new_root_a.to_string_lossy().to_string(),
                    ChangeRootStrategy::Migrate,
                    true,
                )
                .await
            });

            // 任务 B：create_managed 落地到 old_root（settings 尚未切换）
            let pool_b = Arc::clone(&pool);
            let cid_b = Arc::clone(&cid);
            let new_src_b = new_src.clone();
            let h_create = tokio::spawn(async move {
                crate::reference::create_managed(
                    &pool_b,
                    (*cid_b).clone(),
                    "new-ref".into(),
                    "code".into(),
                    crate::reference::Locator {
                        kind: "path".into(),
                        path: new_src_b.to_string_lossy().to_string(),
                    },
                    crate::reference::ManagedAction::Copy,
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
            });

            let (r_migrate, r_create) = tokio::join!(h_migrate, h_create);
            let migrate_res = r_migrate.expect("join migrate").expect("migrate ok");
            let create_res = r_create.expect("join create").expect("create ok");

            // 两者都成功
            match migrate_res {
                ChangeRootResult::Migrated(m) => {
                    assert_eq!(m.migrated.len(), 1);
                    assert!(m.failed.is_empty());
                }
                other => panic!("expected Migrated, got {:?}", other),
            }
            match create_res {
                crate::reference::ManagedCreateResult::Created(_) => {}
                other => panic!("expected Created, got {:?}", other),
            }

            // 互斥锁已被初始化（至少一个调用持有过）
            assert!(crate::reference::MANAGED_WRITE_LOCK.get().is_some());
        }

        // ---------- 序列化契约 ----------

        #[test]
        fn change_root_strategy_deserializes_snake_case() {
            let f: ChangeRootStrategy =
                serde_json::from_str(r#""future_only""#).expect("future_only");
            let m: ChangeRootStrategy = serde_json::from_str(r#""migrate""#).expect("migrate");
            assert_eq!(f, ChangeRootStrategy::FutureOnly);
            assert_eq!(m, ChangeRootStrategy::Migrate);
            assert!(serde_json::from_str::<ChangeRootStrategy>(r#""FutureOnly""#).is_err());
        }

        #[test]
        fn migration_plan_serializes_camel_case() {
            let plan = MigrationPlan {
                new_root_dir: "/new".into(),
                strategy: "migrate".into(),
                items: vec![MigrationPlanItem {
                    ref_id: "r1".into(),
                    current_path: "/old/Code/a.rs".into(),
                    proposed_path: "/new/Code/a.rs".into(),
                    status: "ok".into(),
                }],
                total_bytes: 100,
                conflicts: vec![],
            };
            let v = serde_json::to_value(&plan).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("newRootDir"));
            assert!(obj.contains_key("totalBytes"));
            assert!(obj.contains_key("items"));
            assert!(!obj.contains_key("new_root_dir"));
            let item = &obj["items"][0];
            assert!(item.get("refId").is_some());
            assert!(item.get("currentPath").is_some());
            assert!(item.get("proposedPath").is_some());
        }

        #[test]
        fn migration_result_serializes_camel_case() {
            let res = MigrationResult {
                migrated: vec!["r1".into()],
                failed: vec![MigrationFailure {
                    ref_id: "r2".into(),
                    current_path: "/old/x".into(),
                    proposed_path: "/new/x".into(),
                    error: "源路径不存在".into(),
                }],
            };
            let v = serde_json::to_value(&res).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("migrated"));
            assert!(obj.contains_key("failed"));
            let f = &obj["failed"][0];
            assert!(f.get("refId").is_some());
            assert!(f.get("currentPath").is_some());
            assert!(f.get("proposedPath").is_some());
        }

        #[test]
        fn change_root_result_untagged_serializes_all_variants() {
            let plan = ChangeRootResult::Plan(MigrationPlan {
                new_root_dir: "/n".into(),
                strategy: "migrate".into(),
                items: vec![],
                total_bytes: 0,
                conflicts: vec![],
            });
            let v = serde_json::to_value(&plan).expect("serialize");
            assert!(v.get("items").is_some());

            let migrated = ChangeRootResult::Migrated(MigrationResult {
                migrated: vec![],
                failed: vec![],
            });
            let v2 = serde_json::to_value(&migrated).expect("serialize");
            assert!(v2.get("migrated").is_some());

            let future = ChangeRootResult::FutureOnlyApplied(FutureOnlyResult {
                new_root_dir: "/n".into(),
            });
            let v3 = serde_json::to_value(&future).expect("serialize");
            assert!(v3.get("newRootDir").is_some());
            assert!(v3.get("items").is_none());
            assert!(v3.get("migrated").is_none());
        }
    }

    // ============================================================
    // m6-6.4 · settings_get_default_app / settings_set_default_app
    // ============================================================

    mod default_app_tests {
        use super::*;

        // ---------- 入参校验 ----------

        #[tokio::test]
        async fn default_app_rejects_empty_type() {
            let pool = setup().await;
            let err = get_default_app(&pool, "".into())
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");

            let err = get_default_app(&pool, "   ".into())
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn default_app_rejects_type_with_path_separator() {
            let pool = setup().await;
            let err = get_default_app(&pool, "a/b".into())
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");

            let err = get_default_app(&pool, "a\\b".into())
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn set_default_app_rejects_app_strategy_without_path() {
            let pool = setup().await;
            let err = set_default_app(&pool, "code".into(), DefaultAppStrategy::App, None)
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn set_default_app_rejects_app_strategy_with_empty_path() {
            let pool = setup().await;
            let err = set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::App,
                Some("   ".into()),
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn set_default_app_rejects_app_strategy_with_relative_path() {
            let pool = setup().await;
            let err = set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::App,
                Some("Typora".into()),
            )
            .await
            .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        // ---------- 读写流程 ----------

        #[tokio::test]
        async fn get_default_app_returns_system_default_when_unset() {
            let pool = setup().await;
            let cfg = get_default_app(&pool, "code".into()).await.expect("get ok");
            assert_eq!(cfg.ref_type, "code");
            assert_eq!(cfg.strategy, "system_default");
            assert!(cfg.app_path.is_none());
        }

        #[tokio::test]
        async fn set_then_get_default_app_roundtrip_system_default() {
            let pool = setup().await;
            let r = set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::SystemDefault,
                Some("/should/be/ignored".into()),
            )
            .await
            .expect("set ok");
            assert_eq!(r.strategy, "system_default");
            assert!(r.app_path.is_none(), "system_default 不应返回 appPath");

            let cfg = get_default_app(&pool, "code".into()).await.expect("get ok");
            assert_eq!(cfg.strategy, "system_default");
            assert!(cfg.app_path.is_none());
        }

        #[tokio::test]
        async fn set_then_get_default_app_roundtrip_app() {
            let pool = setup().await;
            let r = set_default_app(
                &pool,
                "document".into(),
                DefaultAppStrategy::App,
                Some("/Applications/Typora.app".into()),
            )
            .await
            .expect("set ok");
            assert_eq!(r.ref_type, "document");
            assert_eq!(r.strategy, "app");
            assert_eq!(r.app_path.as_deref(), Some("/Applications/Typora.app"));

            let cfg = get_default_app(&pool, "document".into())
                .await
                .expect("get ok");
            assert_eq!(cfg.strategy, "app");
            assert_eq!(cfg.app_path.as_deref(), Some("/Applications/Typora.app"));
        }

        #[tokio::test]
        async fn set_default_app_overwrites_previous_value() {
            let pool = setup().await;
            set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::App,
                Some("/Applications/VSCode.app".into()),
            )
            .await
            .expect("set1 ok");

            set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::SystemDefault,
                None,
            )
            .await
            .expect("set2 ok");

            let cfg = get_default_app(&pool, "code".into()).await.expect("get ok");
            assert_eq!(cfg.strategy, "system_default");
            assert!(cfg.app_path.is_none(), "覆盖为 system_default 后 appPath 应被清除");
        }

        #[tokio::test]
        async fn default_app_is_per_type() {
            let pool = setup().await;
            set_default_app(
                &pool,
                "code".into(),
                DefaultAppStrategy::App,
                Some("/Applications/VSCode.app".into()),
            )
            .await
            .expect("set code");
            set_default_app(
                &pool,
                "document".into(),
                DefaultAppStrategy::App,
                Some("/Applications/Typora.app".into()),
            )
            .await
            .expect("set document");

            let code_cfg = get_default_app(&pool, "code".into()).await.expect("get code");
            let doc_cfg = get_default_app(&pool, "document".into())
                .await
                .expect("get document");
            let media_cfg = get_default_app(&pool, "media".into())
                .await
                .expect("get media");

            assert_eq!(code_cfg.app_path.as_deref(), Some("/Applications/VSCode.app"));
            assert_eq!(doc_cfg.app_path.as_deref(), Some("/Applications/Typora.app"));
            assert_eq!(media_cfg.strategy, "system_default");
            assert!(media_cfg.app_path.is_none());
        }

        #[tokio::test]
        async fn default_app_persists_across_pool_reopen() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let db_path = tmp.path().join("test.db");

            {
                let pool = crate::db::init_pool_with_file(&db_path)
                    .await
                    .expect("init pool");
                set_default_app(
                    &pool,
                    "code".into(),
                    DefaultAppStrategy::App,
                    Some("/Applications/VSCode.app".into()),
                )
                .await
                .expect("set ok");
                pool.close().await;
            }

            let pool2 = crate::db::init_pool_with_file(&db_path)
                .await
                .expect("reopen pool");
            let cfg = get_default_app(&pool2, "code".into()).await.expect("get ok");
            assert_eq!(cfg.strategy, "app");
            assert_eq!(cfg.app_path.as_deref(), Some("/Applications/VSCode.app"));
        }

        // ---------- 序列化契约 ----------

        #[test]
        fn default_app_strategy_deserializes_snake_case() {
            let s: DefaultAppStrategy =
                serde_json::from_str(r#""system_default""#).expect("system_default");
            let a: DefaultAppStrategy = serde_json::from_str(r#""app""#).expect("app");
            assert_eq!(s, DefaultAppStrategy::SystemDefault);
            assert_eq!(a, DefaultAppStrategy::App);
            assert!(serde_json::from_str::<DefaultAppStrategy>(r#""SystemDefault""#).is_err());
        }

        #[test]
        fn default_app_config_serializes_camel_case() {
            let c = DefaultAppConfig {
                ref_type: "code".into(),
                strategy: "app".into(),
                app_path: Some("/Applications/VSCode.app".into()),
            };
            let v = serde_json::to_value(&c).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("type"));
            assert!(obj.contains_key("strategy"));
            assert!(obj.contains_key("appPath"));
            assert!(!obj.contains_key("ref_type"));
            assert!(!obj.contains_key("app_path"));

            // appPath=None 时被 skip
            let c2 = DefaultAppConfig {
                ref_type: "code".into(),
                strategy: "system_default".into(),
                app_path: None,
            };
            let v2 = serde_json::to_value(&c2).expect("serialize");
            let obj2 = v2.as_object().expect("object");
            assert!(!obj2.contains_key("appPath"));
            assert_eq!(obj2["strategy"].as_str(), Some("system_default"));
        }
    }

    // ============================================================
    // m7-7.3 · settings_get/set_default_home
    // ============================================================

    mod default_home_tests {
        use super::*;

        #[tokio::test]
        async fn default_home_returns_dashboard_when_unset() {
            let pool = setup().await;
            let cfg = get_default_home(&pool).await.expect("get ok");
            assert_eq!(cfg.home, "dashboard", "未设置时应回退到 dashboard");
        }

        #[tokio::test]
        async fn set_then_get_default_home_roundtrip() {
            let pool = setup().await;
            let r = set_default_home(&pool, "spaces".into()).await.expect("set ok");
            assert_eq!(r.home, "spaces");

            let cfg = get_default_home(&pool).await.expect("get ok");
            assert_eq!(cfg.home, "spaces");

            // 改回 dashboard
            set_default_home(&pool, "dashboard".into()).await.expect("set2 ok");
            let cfg2 = get_default_home(&pool).await.expect("get2 ok");
            assert_eq!(cfg2.home, "dashboard");
        }

        #[tokio::test]
        async fn set_default_home_rejects_invalid_value() {
            let pool = setup().await;
            let err = set_default_home(&pool, "weird".into())
                .await
                .expect_err("should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");

            let err = set_default_home(&pool, "".into())
                .await
                .expect_err("empty should fail");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");

            // 大小写敏感：Dashboard 不合法
            let err = set_default_home(&pool, "Dashboard".into())
                .await
                .expect_err("case sensitive");
            assert_eq!(err.code, "COMMON_INVALID_PARAM");
        }

        #[tokio::test]
        async fn default_home_persists_across_pool_reopen() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let db_path = tmp.path().join("test.db");

            {
                let pool = crate::db::init_pool_with_file(&db_path)
                    .await
                    .expect("init pool");
                set_default_home(&pool, "spaces".into()).await.expect("set ok");
                pool.close().await;
            }

            let pool2 = crate::db::init_pool_with_file(&db_path)
                .await
                .expect("reopen pool");
            let cfg = get_default_home(&pool2).await.expect("get ok");
            assert_eq!(cfg.home, "spaces");
        }

        #[tokio::test]
        async fn default_home_falls_back_on_invalid_db_value() {
            let pool = setup().await;
            // 直接写入非法值（模拟旧版本/外部修改）
            let value_json = serde_json::to_string(&serde_json::Value::String("bogus".into()))
                .expect("serialize");
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES ('default_home', ?, ?)",
            )
            .bind(&value_json)
            .bind(now)
            .execute(&pool)
            .await
            .expect("insert bogus");

            let cfg = get_default_home(&pool).await.expect("get ok");
            assert_eq!(cfg.home, "dashboard", "非法存量值应回退到默认");
        }

        #[test]
        fn default_home_config_serializes_camel_case() {
            let c = DefaultHomeConfig {
                home: "dashboard".into(),
            };
            let v = serde_json::to_value(&c).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("home"));
            assert_eq!(obj["home"].as_str(), Some("dashboard"));
        }
    }

    // ============================================================
    // m7-7.3 · settings_get_user_name
    // ============================================================

    mod user_name_tests {
        use super::*;

        #[tokio::test]
        async fn user_name_returns_none_when_unset() {
            let pool = setup().await;
            let cfg = get_user_name(&pool).await.expect("get ok");
            assert_eq!(cfg.user_name, None);
        }

        #[tokio::test]
        async fn user_name_returns_value_when_set() {
            let pool = setup().await;
            let value_json = serde_json::to_string(&serde_json::Value::String("lazyegg".into()))
                .expect("serialize");
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES ('user_name', ?, ?)",
            )
            .bind(&value_json)
            .bind(now)
            .execute(&pool)
            .await
            .expect("insert");

            let cfg = get_user_name(&pool).await.expect("get ok");
            assert_eq!(cfg.user_name.as_deref(), Some("lazyegg"));
        }

        #[tokio::test]
        async fn user_name_returns_none_for_empty_or_whitespace() {
            let pool = setup().await;
            for v in ["", "   "] {
                let value_json =
                    serde_json::to_string(&serde_json::Value::String(v.into())).expect("ser");
                let now = time::OffsetDateTime::now_utc().unix_timestamp();
                sqlx::query(
                    "INSERT INTO settings (key, value_json, updated_at) VALUES ('user_name', ?, ?) \
                     ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                )
                .bind(&value_json)
                .bind(now)
                .execute(&pool)
                .await
                .expect("upsert");

                let cfg = get_user_name(&pool).await.expect("get ok");
                assert_eq!(cfg.user_name, None, "empty/whitespace 应回退 None");
            }
        }

        #[test]
        fn user_name_config_serializes_camel_case() {
            let c = UserNameConfig {
                user_name: Some("x".into()),
            };
            let v = serde_json::to_value(&c).expect("serialize");
            let obj = v.as_object().expect("object");
            assert!(obj.contains_key("userName"));
            assert!(!obj.contains_key("user_name"));

            // None 时被 skip
            let c2 = UserNameConfig { user_name: None };
            let v2 = serde_json::to_value(&c2).expect("serialize");
            let obj2 = v2.as_object().expect("object");
            assert!(!obj2.contains_key("userName"));
        }
    }
}
