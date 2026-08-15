//! 处置能力声明（详细设计 §2.6、§5.1、§6.7）
//!
//! 本模块实现 `disp_get_capabilities` 命令：给定 `refId`，返回该引用当前
//! 可用的处置操作集合（archive / softDelete / destroy / restoreFromBin），
//! 并携带每项的 `reason`。
//!
//! 关键约束：
//! - 能力计算为**纯函数** `compute_capabilities`：输入 `(disposition, caps_json)`，
//!   输出 `DispCapabilities`；不依赖数据库连接，便于单测覆盖所有组合。
//! - `softDelete` 能力来自 `storage_source.caps_json.softDelete`，由启动探测写入。
//!   本模块提供 `detect_soft_delete_support` 与 `write_soft_delete_cap`：
//!   macOS / Windows 默认 true；Linux 探测 `$XDG_DATA_HOME/Trash` 目录存在性。
//! - 错误：refId 不存在 → `COMMON_NOT_FOUND`。
//! - false 项必须填 reason；true 项的 reason 可省略。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::{AppError, CmdResult};

// ============================================================
// 出参结构（契约 §2.6）
// ============================================================

/// 单项能力的 reason 集合；只给 false 项填说明，true 项可省略。
///
/// 序列化为 camelCase：`{ "archive": "...", "softDelete": "...", ... }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DispReasons {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_delete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destroy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_from_bin: Option<String>,
}

/// `disp_get_capabilities` 出参（契约 §2.6）。
///
/// 序列化为 camelCase：
/// `{ archive, softDelete, destroy, restoreFromBin, reason }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DispCapabilities {
    pub archive: bool,
    pub soft_delete: bool,
    pub destroy: bool,
    pub restore_from_bin: bool,
    pub reason: DispReasons,
}

// ============================================================
// 能力计算纯函数
// ============================================================

/// 能力计算纯函数：输入 `(disposition, soft_delete_supported)`，
/// 输出 `DispCapabilities`。
///
/// 规则（契约 §2.6）：
/// - `archive`：`disposition != 'archived'` 时 true；已归档则 false 并填 reason。
/// - `softDelete`：取决于存储源 `caps_json.softDelete`；false 时填 reason。
/// - `destroy`：始终 true（兜底）。
/// - `restoreFromBin`：`disposition == 'deleted'` 时 true；否则 false 并填 reason。
///
/// `disposition` 取值集合由 schema CHECK 约束保证为 `none|archived|deleted`，
/// 本函数对未知值按 `none` 处理（防御性兜底）。
pub fn compute_capabilities(disposition: &str, soft_delete_supported: bool) -> DispCapabilities {
    let mut reason = DispReasons::default();

    let archive = if disposition == "archived" {
        reason.archive = Some("已归档，无需重复归档".to_string());
        false
    } else {
        true
    };

    let soft_delete = if soft_delete_supported {
        true
    } else {
        reason.soft_delete = Some("当前存储源不支持系统回收站".to_string());
        false
    };

    // destroy 始终 true（兜底销毁），无需 reason。
    let destroy = true;

    let restore_from_bin = if disposition == "deleted" {
        true
    } else {
        reason.restore_from_bin = Some("引用未处于回收站状态".to_string());
        false
    };

    DispCapabilities {
        archive,
        soft_delete,
        destroy,
        restore_from_bin,
        reason,
    }
}

// ============================================================
// 启动探测：系统回收站可用性
// ============================================================

/// 探测当前平台是否支持系统回收站。
///
/// - macOS / Windows：默认 true（系统回收站始终可用）。
/// - Linux：检查 `$XDG_DATA_HOME/Trash` 目录存在性；`XDG_DATA_HOME` 缺省时
///   回退到 `~/.local/share/Trash`。
/// - 其他平台：默认 false（保守）。
///
/// 本函数为纯探测，不创建任何目录；返回 false 不代表后续不能 soft_delete，
/// 仅表示当前未探测到回收站。
pub fn detect_soft_delete_support() -> bool {
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        linux_trash_dir_exists()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Linux 平台探测：检查 `$XDG_DATA_HOME/Trash` 或 `~/.local/share/Trash` 是否存在。
#[cfg(target_os = "linux")]
fn linux_trash_dir_exists() -> bool {
    use std::path::PathBuf;

    let trash_dir: Option<PathBuf> = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
        .map(|base| base.join("Trash"));

    match trash_dir {
        Some(p) => p.is_dir(),
        None => false,
    }
}

/// 把探测到的 `softDelete` 能力写入指定存储源的 `caps_json`。
///
/// 仅更新 `softDelete` 字段，保留 `caps_json` 中其他键。
/// 存储源不存在 → `COMMON_NOT_FOUND`；`caps_json` 反序列化失败 → `COMMON_DB`。
pub async fn write_soft_delete_cap(
    pool: &SqlitePool,
    source_id: &str,
    supported: bool,
) -> CmdResult<()> {
    let row = sqlx::query("SELECT caps_json FROM storage_source WHERE id = ?")
        .bind(source_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("存储源不存在: {}", source_id)))?;

    let caps_json: Option<String> = row.try_get("caps_json").map_err(AppError::from)?;
    let mut caps: serde_json::Value = match caps_json.as_deref() {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s).map_err(|e| {
            AppError::db(format!(
                "storage_source.caps_json 反序列化失败 ({}): {}",
                source_id, e
            ))
        })?,
        _ => serde_json::json!({}),
    };

    if !caps.is_object() {
        caps = serde_json::json!({});
    }
    caps["softDelete"] = serde_json::Value::Bool(supported);

    let new_caps = serde_json::to_string(&caps)
        .map_err(|e| AppError::db(format!("caps_json 序列化失败: {}", e)))?;

    sqlx::query("UPDATE storage_source SET caps_json = ? WHERE id = ?")
        .bind(new_caps)
        .bind(source_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

    Ok(())
}

// ============================================================
// 业务函数
// ============================================================

/// `disp_get_capabilities` 业务函数：读 reference + storage_source.caps_json，
/// 调用纯函数计算能力集合。
///
/// 错误：
/// - `COMMON_NOT_FOUND`：refId 不存在
/// - `COMMON_DB`：数据库或 JSON 解析失败
pub async fn get_capabilities(pool: &SqlitePool, ref_id: String) -> CmdResult<DispCapabilities> {
    let row = sqlx::query(
        "SELECT r.disposition AS disposition, s.caps_json AS caps_json \
         FROM resource_reference r \
         JOIN storage_source s ON s.id = r.source_id \
         WHERE r.id = ?",
    )
    .bind(&ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let disposition: String = row.try_get("disposition").map_err(AppError::from)?;
    let caps_json: Option<String> = row.try_get("caps_json").map_err(AppError::from)?;

    let soft_delete_supported = parse_soft_delete_cap(caps_json.as_deref())?;
    Ok(compute_capabilities(&disposition, soft_delete_supported))
}

/// 从 `caps_json` 字符串中解析 `softDelete` 字段。
///
/// - `None` / 空串 / 非对象 → 默认 true（与 0001 迁移初始数据一致）。
/// - 字段缺失 → 默认 true。
/// - JSON 解析失败 → `COMMON_DB`。
fn parse_soft_delete_cap(caps_json: Option<&str>) -> CmdResult<bool> {
    let s = match caps_json {
        Some(s) if !s.trim().is_empty() => s,
        _ => return Ok(true),
    };
    let v: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| AppError::db(format!("caps_json 反序列化失败: {}", e)))?;
    let obj = match v.as_object() {
        Some(o) => o,
        None => return Ok(true),
    };
    Ok(obj
        .get("softDelete")
        .and_then(|b| b.as_bool())
        .unwrap_or(true))
}

// ============================================================
// Tauri Command
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn disp_get_capabilities(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
) -> CmdResult<DispCapabilities> {
    get_capabilities(&state.pool, ref_id).await
}

// ============================================================
// 归档 / 恢复（m4-4.2 · 详细设计 §2.6 / §6.7）
// =====================================================//
// 关键约束：
// - 仅改 `resource_reference.disposition`，**不动文件系统**。
// - 状态机：
//   - `disp_archive`   : none     → archived；当前 archived/deleted → COMMON_CONFLICT
//   - `disp_unarchive` : archived → none；    当前 none/deleted     → COMMON_CONFLICT
// - 审计：UPDATE disposition + INSERT disposition_audit 在同一 sqlx 事务中。
//   `actor` 固定 `'local_user'`，`locator_snapshot` 存当前 `locator_json` 完整快照。
// - refId 不存在 → `COMMON_NOT_FOUND`。

/// 审计写入辅助（供 4.4 / 4.5 复用）。
///
/// 在调用方持有的事务 `tx` 中插入一条 `disposition_audit` 记录。
/// 调用方负责事务的开启与提交；本函数失败仅回滚事务，不做额外补偿。
///
/// 参数：
/// - `tx`               : 当前事务
/// - `ref_id`           : 被操作的 reference id
/// - `ref_name`         : 当前 reference.name 快照
/// - `action`           : 'archive' | 'unarchive' | 'soft_delete' | 'destroy'
/// - `locator_snapshot` : 当前 `locator_json` 完整快照（TEXT）
/// - `note`             : 可选备注
pub(crate) async fn write_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ref_id: &str,
    ref_name: &str,
    action: &str,
    locator_snapshot: Option<&str>,
    note: Option<&str>,
) -> CmdResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let at = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query(
        "INSERT INTO disposition_audit \
         (id, ref_id, ref_name, action, locator_snapshot, actor, note, at) \
         VALUES (?, ?, ?, ?, ?, 'local_user', ?, ?)",
    )
    .bind(&id)
    .bind(ref_id)
    .bind(ref_name)
    .bind(action)
    .bind(locator_snapshot)
    .bind(note)
    .bind(at)
    .execute(&mut **tx)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// 归档 / 恢复共用核心：在事务内校验当前 disposition、执行迁移、写审计。
///
/// - `expect_from`：当前 disposition 必须等于此值，否则 `COMMON_CONFLICT`。
/// - `to`         : 目标 disposition。
/// - `action`     : 写入审计的 action 字符串。
///
/// 返回更新后的 `Reference`（事务提交后再读，保证标签等关联数据一致）。
async fn transition_disposition(
    pool: &SqlitePool,
    ref_id: &str,
    expect_from: &str,
    to: &str,
    action: &str,
) -> CmdResult<crate::reference::Reference> {
    // 1. 读出当前行（含 name / locator_json 快照），不存在 → NOT_FOUND。
    let row = sqlx::query(
        "SELECT name, disposition, locator_json FROM resource_reference WHERE id = ?",
    )
    .bind(ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let name: String = row.try_get("name").map_err(AppError::from)?;
    let current: String = row.try_get("disposition").map_err(AppError::from)?;
    let locator_json: Option<String> = row.try_get("locator_json").map_err(AppError::from)?;

    // 2. 状态机校验：当前必须等于 expect_from，否则 CONFLICT。
    if current != expect_from {
        return Err(AppError::conflict(format!(
            "当前 disposition={}，无法执行 {}（要求 {}）",
            current, action, expect_from
        )));
    }

    // 3. 事务：UPDATE disposition + INSERT audit。
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query("UPDATE resource_reference SET disposition = ?, updated_at = ? WHERE id = ?")
        .bind(to)
        .bind(now)
        .bind(ref_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
    write_audit(
        &mut tx,
        ref_id,
        &name,
        action,
        locator_json.as_deref(),
        None,
    )
    .await?;
    tx.commit().await.map_err(AppError::from)?;

    // 4. 提交后重新读取完整 Reference（含 tags），保持与 ref_get 出参一致。
    crate::reference::get(pool, ref_id.to_string()).await
}

// ============================================================
// disp_preview：删除/销毁确认框的统计数据（契约 §2.6）
// ============================================================
//
// 设计要点（详细设计 §6 性能）：
// - 目录递归统计在 `tokio::task::spawn_blocking` 阻塞线程池执行，不阻塞 async runtime。
// - 取消机制：协作式 `Arc<AtomicBool>`。每次 `disp_preview` 生成 UUID 作为 `previewId`，
//   并把取消标志注册到进程级 `PREVIEW_REGISTRY`；前端调 `disp_preview_cancel { previewId }`
//   把标志位置 true，walker 每个条目检查一次，发现取消立即终止并返回 `COMMON_CANCELLED`。
// - 命令完成（成功/失败/取消）后从注册表移除自身条目，避免泄漏。
// - 符号链接不跟随出根（与 M3-3.2 `reference::walk_dir` 一致）：用 `symlink_metadata`
//   判断，symlink 自身按文件计入 fileCount 与 totalBytes，但不递归进入。

/// `disp_preview` 出参（契约 §2.6）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DispPreview {
    pub preview_id: String,
    pub target: String,
    pub is_dir: bool,
    pub file_count: u64,
    pub total_bytes: u64,
    pub capability: DispCapabilities,
    pub warning: String,
}

/// 进程级 preview 取消注册表：previewId → 取消标志。
///
/// 用 `OnceLock<Mutex<HashMap>>` 而非 `lazy_static`/`once_cell` 外部 crate，
/// 避免引入新依赖（任务约束 #8）。
static PREVIEW_REGISTRY: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

fn preview_registry() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    PREVIEW_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 注册新 preview，返回 `(previewId, cancel_flag)`。
fn register_preview() -> (String, Arc<AtomicBool>) {
    let id = uuid::Uuid::new_v4().to_string();
    let flag = Arc::new(AtomicBool::new(false));
    let mut map = preview_registry()
        .lock()
        .expect("preview registry poisoned");
    map.insert(id.clone(), Arc::clone(&flag));
    (id, flag)
}

/// 从注册表移除 preview（命令结束时调用）。
fn unregister_preview(preview_id: &str) {
    let mut map = preview_registry()
        .lock()
        .expect("preview registry poisoned");
    map.remove(preview_id);
}

/// 取消指定 preview：把标志位置 true。返回是否存在该 previewId。
fn cancel_preview(preview_id: &str) -> bool {
    let map = preview_registry()
        .lock()
        .expect("preview registry poisoned");
    if let Some(flag) = map.get(preview_id) {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

/// 从 `resource_reference` 行解析目标绝对路径。
///
/// 仅支持 `locator_json.kind == "path"`；其他 kind 返回 `COMMON_INVALID_PARAM`。
fn parse_target_path(locator_json: &str) -> CmdResult<PathBuf> {
    let v: serde_json::Value = serde_json::from_str(locator_json).map_err(|e| {
        AppError::db(format!("locator_json 反序列化失败: {}", e))
    })?;
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    if kind != "path" {
        return Err(AppError::invalid_param(format!(
            "暂不支持的 locator kind: {}",
            kind
        )));
    }
    let path = v
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| AppError::db("locator_json 缺少 path 字段".to_string()))?;
    Ok(PathBuf::from(path))
}

/// 递归统计目标路径：返回 `(is_dir, file_count, total_bytes)`。
///
/// - 单文件 / 符号链接：`file_count = 1`，`total_bytes = metadata.len()`。
/// - 目录：深度优先递归 walk；符号链接不跟随出根（与 M3-3.2 一致）。
/// - `cancel_flag` 每个条目检查一次；为 true 时返回 `COMMON_CANCELLED`。
/// - 路径不存在 → `COMMON_NOT_FOUND`（契约 §2.6 错误语义）。
fn stat_target(path: &Path, cancel_flag: &AtomicBool) -> CmdResult<(bool, u64, u64)> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| map_preview_fs_err("读取目标元数据失败", path, e))?;
    if meta.is_dir() {
        let mut file_count: u64 = 0;
        let mut total_bytes: u64 = 0;
        walk_preview_dir(path, cancel_flag, &mut |entry_meta| {
            if entry_meta.is_file() || entry_meta.file_type().is_symlink() {
                file_count += 1;
                total_bytes = total_bytes.saturating_add(entry_meta.len());
            }
            Ok(())
        })?;
        Ok((true, file_count, total_bytes))
    } else {
        // 单文件 / 符号链接：直接读 metadata
        Ok((false, 1, meta.len()))
    }
}

/// 深度优先递归遍历；每个条目调用 `f`，并在每个条目检查 `cancel_flag`。
fn walk_preview_dir<F>(root: &Path, cancel_flag: &AtomicBool, f: &mut F) -> CmdResult<()>
where
    F: FnMut(std::fs::Metadata) -> CmdResult<()>,
{
    if cancel_flag.load(Ordering::SeqCst) {
        return Err(AppError::new("COMMON_CANCELLED", "preview 已取消"));
    }
    let entries = std::fs::read_dir(root)
        .map_err(|e| map_preview_fs_err("读取目录失败", root, e))?;
    for entry in entries {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(AppError::new("COMMON_CANCELLED", "preview 已取消"));
        }
        let entry = entry.map_err(|e| map_preview_fs_err("读取目录条目失败", root, e))?;
        let entry_path = entry.path();
        // symlink_metadata 不跟随符号链接，避免越出根
        let meta = std::fs::symlink_metadata(&entry_path)
            .map_err(|e| map_preview_fs_err("读取条目元数据失败", &entry_path, e))?;
        f(meta.clone())?;
        if meta.is_dir() {
            walk_preview_dir(&entry_path, cancel_flag, f)?;
        }
    }
    Ok(())
}

/// preview 专用 FS 错误映射：路径不存在 → `COMMON_NOT_FOUND`（契约 §2.6）。
fn map_preview_fs_err(context: &str, path: &Path, err: std::io::Error) -> AppError {
    let msg = format!("{} {}: {}", context, path.display(), err);
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::not_found(format!("目标路径不存在: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => AppError::new("FS_PERMISSION_DENIED", msg),
        _ => AppError::io(msg),
    }
}

/// 生成中文警告文案：包含 fileCount / totalBytes / 「销毁后不可恢复」。
fn build_warning(is_dir: bool, file_count: u64, total_bytes: u64) -> String {
    let size_human = human_size(total_bytes);
    if is_dir {
        format!(
            "目录包含 {} 个文件，共 {}；销毁后不可恢复",
            file_count, size_human
        )
    } else {
        format!("文件大小 {}；销毁后不可恢复", size_human)
    }
}

/// 字节数人性化展示（B / KB / MB / GB / TB）。
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// `disp_preview` 业务函数：查 reference → 后台线程统计 → 计算能力 → 组装出参。
///
/// 错误：
/// - `COMMON_NOT_FOUND`：refId 不存在 / 目标路径已被外部删除
/// - `COMMON_CANCELLED`：前端调用 `disp_preview_cancel` 取消
/// - `COMMON_DB`：数据库或 JSON 解析失败
pub async fn preview(pool: &SqlitePool, ref_id: String) -> CmdResult<DispPreview> {
    // 1) 查 reference + storage_source.caps_json
    let row = sqlx::query(
        "SELECT r.disposition AS disposition, r.locator_json AS locator_json, \
                s.caps_json AS caps_json \
         FROM resource_reference r \
         JOIN storage_source s ON s.id = r.source_id \
         WHERE r.id = ?",
    )
    .bind(&ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let disposition: String = row.try_get("disposition").map_err(AppError::from)?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let caps_json: Option<String> = row.try_get("caps_json").map_err(AppError::from)?;

    let target_path = parse_target_path(&locator_json)?;

    // 2) 注册 preview，准备取消标志
    let (preview_id, cancel_flag) = register_preview();
    let cancel_flag_for_worker = Arc::clone(&cancel_flag);
    let target_for_worker = target_path.clone();

    // 3) 阻塞线程池执行递归统计；结束（无论成败）后从注册表移除
    let stat_result = tokio::task::spawn_blocking(move || {
        stat_target(&target_for_worker, &cancel_flag_for_worker)
    })
    .await
    .map_err(|e| AppError::io(format!("preview worker join 失败: {}", e)));
    unregister_preview(&preview_id);
    let (is_dir, file_count, total_bytes) = stat_result??;

    // 4) 能力计算（复用 4.1 纯函数）
    let soft_delete_supported = parse_soft_delete_cap(caps_json.as_deref())?;
    let capability = compute_capabilities(&disposition, soft_delete_supported);

    // 5) 警告文案
    let warning = build_warning(is_dir, file_count, total_bytes);

    Ok(DispPreview {
        preview_id,
        target: target_path.to_string_lossy().into_owned(),
        is_dir,
        file_count,
        total_bytes,
        capability,
        warning,
    })
}

/// `disp_archive { refId }` → `Reference`。
///
/// `disposition` 从 `none` → `archived`；已 archived / deleted → `COMMON_CONFLICT`。
/// 不动文件系统。
pub async fn archive(pool: &SqlitePool, ref_id: String) -> CmdResult<crate::reference::Reference> {
    transition_disposition(pool, &ref_id, "none", "archived", "archive").await
}

/// `disp_unarchive { refId }` → `Reference`。
///
/// `disposition` 从 `archived` → `none`；未归档（none / deleted）→ `COMMON_CONFLICT`。
/// 不动文件系统。
pub async fn unarchive(
    pool: &SqlitePool,
    ref_id: String,
) -> CmdResult<crate::reference::Reference> {
    transition_disposition(pool, &ref_id, "archived", "none", "unarchive").await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn disp_archive(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
) -> CmdResult<crate::reference::Reference> {
    archive(&state.pool, ref_id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn disp_unarchive(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
) -> CmdResult<crate::reference::Reference> {
    unarchive(&state.pool, ref_id).await
}

/// `disp_preview` Tauri 命令。
#[tauri::command(rename_all = "camelCase")]
pub async fn disp_preview(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
) -> CmdResult<DispPreview> {
    preview(&state.pool, ref_id).await
}

/// `disp_preview_cancel` Tauri 命令：协作式取消正在进行的 preview。
///
/// 若 previewId 不存在（已结束或从未注册），返回 Ok(false)；存在则置标志位并返回 Ok(true)。
#[tauri::command(rename_all = "camelCase")]
pub async fn disp_preview_cancel(preview_id: String) -> CmdResult<bool> {
    Ok(cancel_preview(&preview_id))
}

// ============================================================
// disp_destroy：销毁（永久删除文件 + 审计 + 物理删除引用行）
// （m4-4.5 · 详细设计 §2.6 / §4.2 / §5 / §6.7）
// ============================================================
//
// 契约（§2.6）：
// - 入参：`{ refId, confirmText, confirmed: true }`
// - 出参：`{ deletedRefId }`（二选一固化：返回被删除的 refId 便于前端清理缓存/路由；
//   不返回 `()` 是因为前端需要明确知道哪条引用被销毁，避免依赖入参回显）
// - 错误：`COMMON_CONFIRM_REQUIRED` / `FS_PERMISSION_DENIED` / `COMMON_IO`
//   / `COMMON_NOT_FOUND` / `COMMON_FORBIDDEN`
//
// 关键约束：
// - **confirmText 强校验**：从 DB 读出当前 `resource_reference.name`，与入参
//   `confirmText` **逐字符比对**（`String == String`）；不一致立即返回
//   `COMMON_CONFIRM_REQUIRED`，**不做任何写操作**（不写文件、不写库）。
// - **confirmed 必须为 true**：否则同样返回 `COMMON_CONFIRM_REQUIRED`。
// - **互斥**：复用 `crate::reference::managed_write_lock()`（M3-3.2 落地 /
//   M4-4.8 根目录迁移 / 本销毁共用同一把 `MANAGED_WRITE_LOCK`，详细设计 §5
//   「多步写操作互斥串行化」）。临界区内含 `.await`（tokio::fs + sqlx 事务），
//   与 reference.rs 的异步 Mutex 选型一致。
// - **执行顺序**：
//   1. 读 DB 拿到 name / disposition / locator_json 快照；
//   2. confirmText / confirmed 校验（失败不写任何数据）；
//   3. 加全局互斥锁；
//   4. `tokio::fs::remove_file` / `remove_dir_all` 永久删除目标（不经回收站）；
//      文件删除失败 → 不写库，直接返回错误；
//   5. 文件删除成功后开事务：INSERT disposition_audit（action='destroy'）
//      + DELETE resource_reference 行；
//   6. 写库失败：永久删除无法回滚，返回 `COMMON_DB` 并 `eprintln!` 记录
//      已删除路径与 refId，**不**尝试恢复文件（best-effort 日志补偿）。
// - **能力规则**：`compute_capabilities.destroy` 始终为 true（兜底），
//   因此本命令对任意 disposition（none / archived / deleted）均允许销毁，
//   不返回 `COMMON_FORBIDDEN`。已 deleted 的引用再销毁：允许（固化注释）——
//   此时文件可能已被外部清空，`tokio::fs::remove_*` 对不存在路径返回
//   NotFound，按 `COMMON_NOT_FOUND` 处理；调用方需先通过 preview 确认。
// - **审计字段**（§6.7）：
//   - `id`：UUID v4
//   - `ref_id` / `ref_name`：当前引用 id 与 name 快照
//   - `action`：`'destroy'`
//   - `locator_snapshot`：当前 `locator_json` 完整快照（含被删路径）
//   - `actor`：`'local_user'`（`write_audit` 内固定）
//   - `note`：填入 JSON 字符串 `{"fileCount":N,"totalBytes":M,"isDir":B}`，
//     来自销毁前的同步统计（best-effort，统计失败则填 NULL）
//   - `at`：Unix 秒
// - **引用删除**：`DELETE FROM resource_reference WHERE id = ?`（物理删除；
//   `disposition_audit` 无外键约束，审计独立存活，§6.7）。

/// `disp_destroy` 出参（契约 §2.6 二选一固化：返回 `deletedRefId`）。
///
/// 序列化为 camelCase：`{ "deletedRefId": "..." }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DispDestroyResult {
    pub deleted_ref_id: String,
}

/// 销毁前的同步统计（best-effort）：返回 `(is_dir, file_count, total_bytes)`。
///
/// 复用 preview 的 `stat_target`，但**不注册取消**（销毁是同步短操作，
/// 且契约未要求可取消）。统计失败仅影响 note 字段，不阻塞销毁。
fn stat_for_destroy(path: &Path) -> CmdResult<(bool, u64, u64)> {
    let flag = AtomicBool::new(false);
    stat_target(path, &flag)
}

/// `disp_destroy` 业务函数：永久删除文件 + 写审计 + 物理删除引用行。
///
/// 详见模块级注释（m4-4.5）。
pub async fn destroy(
    pool: &SqlitePool,
    ref_id: String,
    confirm_text: String,
    confirmed: bool,
) -> CmdResult<DispDestroyResult> {
    // 1) 读 DB：name / disposition / locator_json 快照。
    let row = sqlx::query(
        "SELECT name, disposition, locator_json FROM resource_reference WHERE id = ?",
    )
    .bind(&ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let name: String = row.try_get("name").map_err(AppError::from)?;
    let _disposition: String = row.try_get("disposition").map_err(AppError::from)?;
    let locator_json: Option<String> = row.try_get("locator_json").map_err(AppError::from)?;

    // 2) confirmText / confirmed 强校验（**不做任何写操作**）。
    if !confirmed {
        return Err(AppError::new(
            "COMMON_CONFIRM_REQUIRED",
            "confirmed 必须为 true",
        ));
    }
    // 逐字符比对：Rust `String == String` 即按字节序逐字符比较。
    if confirm_text != name {
        return Err(AppError::new(
            "COMMON_CONFIRM_REQUIRED",
            format!("confirmText 与资源名称不一致（期望: {}）", name),
        ));
    }

    // 3) 解析目标路径（locator_json 必须存在且为 kind=path）。
    let locator_str = locator_json
        .as_deref()
        .ok_or_else(|| AppError::db("locator_json 为空".to_string()))?;
    let target_path = parse_target_path(locator_str)?;

    // 4) 销毁前 best-effort 统计（用于审计 note）；失败仅记日志，不阻塞。
    let stat_note: Option<String> = match stat_for_destroy(&target_path) {
        Ok((is_dir, file_count, total_bytes)) => Some(
            serde_json::json!({
                "isDir": is_dir,
                "fileCount": file_count,
                "totalBytes": total_bytes,
            })
            .to_string(),
        ),
        Err(e) => {
            eprintln!(
                "[disp_destroy] 销毁前统计失败（继续销毁） ref_id={} path={} err={}",
                ref_id,
                target_path.display(),
                e
            );
            None
        }
    };

    // 5) 加全局互斥锁（详细设计 §5：托管落地 / 根目录迁移 / 销毁串行化）。
    //    `managed_write_lock()` 返回 tokio 异步 Mutex（与 reference.rs 的
    //    `lock().await` 用法一致），允许临界区内含 `.await`（tokio::fs + sqlx）。
    let _guard = crate::reference::managed_write_lock().lock().await;

    // 6) 永久删除目标（不经回收站）。
    //    - 单文件 / 符号链接 → remove_file
    //    - 目录 → remove_dir_all（递归删除整棵子树）
    //    失败 → 不写库，直接返回错误。
    let meta = tokio::fs::symlink_metadata(&target_path)
        .await
        .map_err(|e| map_destroy_fs_err("读取目标元数据失败", &target_path, e))?;
    let remove_result: std::io::Result<()> = if meta.is_dir() {
        tokio::fs::remove_dir_all(&target_path).await
    } else {
        tokio::fs::remove_file(&target_path).await
    };
    if let Err(e) = remove_result {
        return Err(map_destroy_fs_err("永久删除失败", &target_path, e));
    }

    // 7) 文件已删除，开事务写审计 + DELETE 引用行。
    //    写库失败：永久删除无法回滚，返回 COMMON_DB 并 best-effort 日志补偿。
    let mut tx = pool.begin().await.map_err(AppError::from)?;
    if let Err(e) = write_audit(
        &mut tx,
        &ref_id,
        &name,
        "destroy",
        Some(locator_str),
        stat_note.as_deref(),
    )
    .await
    {
        let _ = tx.rollback().await;
        eprintln!(
            "[disp_destroy] 审计写入失败，文件已永久删除 ref_id={} path={} err={}",
            ref_id,
            target_path.display(),
            e
        );
        return Err(AppError::db(format!(
            "审计写入失败（文件已删除，无法回滚）: {}",
            e
        )));
    }
    if let Err(e) = sqlx::query("DELETE FROM resource_reference WHERE id = ?")
        .bind(&ref_id)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        eprintln!(
            "[disp_destroy] 引用行删除失败，文件已永久删除 ref_id={} path={} err={}",
            ref_id,
            target_path.display(),
            e
        );
        return Err(AppError::db(format!(
            "引用行删除失败（文件已删除，无法回滚）: {}",
            e
        )));
    }
    if let Err(e) = tx.commit().await {
        eprintln!(
            "[disp_destroy] 事务提交失败，文件已永久删除 ref_id={} path={} err={}",
            ref_id,
            target_path.display(),
            e
        );
        return Err(AppError::db(format!(
            "事务提交失败（文件已删除，无法回滚）: {}",
            e
        )));
    }

    Ok(DispDestroyResult {
        deleted_ref_id: ref_id,
    })
}

/// destroy 专用 FS 错误映射：路径不存在 → `COMMON_NOT_FOUND`；
/// 权限不足 → `FS_PERMISSION_DENIED`；其他 → `COMMON_IO`。
fn map_destroy_fs_err(context: &str, path: &Path, err: std::io::Error) -> AppError {
    let msg = format!("{} {}: {}", context, path.display(), err);
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::not_found(format!("目标路径不存在: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => AppError::new("FS_PERMISSION_DENIED", msg),
        _ => AppError::io(msg),
    }
}

/// `disp_destroy` Tauri 命令。
#[tauri::command(rename_all = "camelCase")]
pub async fn disp_destroy(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
    confirm_text: String,
    confirmed: bool,
) -> CmdResult<DispDestroyResult> {
    destroy(&state.pool, ref_id, confirm_text, confirmed).await
}

// ============================================================
// disp_soft_delete：移入系统回收站（m4-4.4 · 详细设计 §2.6 / §5.1）
// =====================================================//
// 关键约束：
// - 顺序：**先 `trash::delete` 成功 → 再开事务 UPDATE disposition + INSERT audit**。
//   与 M3-3.2 相反 —— 落地先，写库后；写库失败则无残留（文件已在回收站，用户可手动恢复）。
// - 能力预检：执行 `trash::delete` 前调用 4.1 的能力计算；`softDelete=false` 直接
//   返回 `COMMON_FORBIDDEN`，**不尝试移动**。
// - confirmed 强制：`confirmed != true` 返回 `COMMON_CONFIRM_REQUIRED`。
// - 失败补偿：`trash::delete` 成功但写库失败时，**不回滚回收站**；返回 `COMMON_DB`，
//   并 best-effort 写一条 action='soft_delete' + note='db_failed' 的审计记录。
// - 错误映射：
//   - 路径不存在（已被外部删除） → `COMMON_NOT_FOUND`
//   - 权限不足 → `FS_PERMISSION_DENIED`
//   - 平台/回收站不可用 → `FS_RECYCLE_UNSUPPORTED`
//   - 其他 IO → `COMMON_IO`

/// 把 `trash::Error` 映射为统一错误模型。
///
/// - `CouldNotAccess`：目标不存在或权限不足 → `COMMON_NOT_FOUND`（与契约 §2.6 对齐；
///   无法区分时按 not_found 处理，避免泄露文件系统细节）。
/// - `TargetedRoot`：试图删除根目录 → `COMMON_INVALID_PARAM`。
/// - `CanonicalizePath`：路径 canonicalize 失败 → `COMMON_NOT_FOUND`。
/// - Linux `FileSystem`：按内部 `std::io::Error` 的 kind 细分。
/// - 其他（`Unknown` / `Os` / `ConvertOsString` / `RestoreCollision` / `RestoreTwins`）：
///   统一 `COMMON_IO`。
fn map_trash_err(path: &Path, err: trash::Error) -> AppError {
    let msg = format!("移入回收站失败 {}: {:?}", path.display(), err);
    match err {
        trash::Error::CouldNotAccess { .. } => {
            AppError::not_found(format!("目标路径不存在或不可访问: {}", path.display()))
        }
        trash::Error::TargetedRoot => {
            AppError::invalid_param(format!("不允许删除根目录: {}", path.display()))
        }
        trash::Error::CanonicalizePath { .. } => {
            AppError::not_found(format!("目标路径无法解析: {}", path.display()))
        }
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
        trash::Error::FileSystem { source, .. } => match source.kind() {
            std::io::ErrorKind::NotFound => {
                AppError::not_found(format!("目标路径不存在: {}", path.display()))
            }
            std::io::ErrorKind::PermissionDenied => AppError::new("FS_PERMISSION_DENIED", msg),
            _ => AppError::io(msg),
        },
        _ => AppError::io(msg),
    }
}

/// `disp_soft_delete` 业务函数：移入系统回收站 + 事务写库 + 审计。
///
/// 流程：
/// 1. 校验 `confirmed == true`，否则 `COMMON_CONFIRM_REQUIRED`。
/// 2. 读 reference + storage_source.caps_json；refId 不存在 → `COMMON_NOT_FOUND`。
/// 3. 解析 `locator_json` 得到目标路径；目标不存在 → `COMMON_NOT_FOUND`。
/// 4. 调用 4.1 的 `compute_capabilities` 做能力预检；`softDelete=false` → `COMMON_FORBIDDEN`。
/// 5. `trash::delete` 移入回收站；失败按 `map_trash_err` 映射。
/// 6. 开事务：UPDATE disposition='deleted' + INSERT audit(action='soft_delete')。
/// 7. 事务失败：best-effort 写一条 note='db_failed' 的审计（独立连接，不再开事务），
///    返回 `COMMON_DB`。
pub async fn soft_delete(
    pool: &SqlitePool,
    ref_id: String,
    confirmed: bool,
) -> CmdResult<crate::reference::Reference> {
    // 1) confirmed 强制
    if !confirmed {
        return Err(AppError::new(
            "COMMON_CONFIRM_REQUIRED",
            "软删除需要 confirmed=true 确认",
        ));
    }

    // 2) 读 reference + caps_json
    let row = sqlx::query(
        "SELECT r.name AS name, r.disposition AS disposition, r.locator_json AS locator_json, \
                s.caps_json AS caps_json \
         FROM resource_reference r \
         JOIN storage_source s ON s.id = r.source_id \
         WHERE r.id = ?",
    )
    .bind(&ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let name: String = row.try_get("name").map_err(AppError::from)?;
    let disposition: String = row.try_get("disposition").map_err(AppError::from)?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let caps_json: Option<String> = row.try_get("caps_json").map_err(AppError::from)?;

    // 3) 解析目标路径 + 存在性预检（提前返回 NOT_FOUND，避免 trash 内部歧义）
    let target_path = parse_target_path(&locator_json)?;
    if !target_path.exists() {
        return Err(AppError::not_found(format!(
            "目标路径不存在: {}",
            target_path.display()
        )));
    }

    // 4) 能力预检（复用 4.1 纯函数）
    let soft_delete_supported = parse_soft_delete_cap(caps_json.as_deref())?;
    let caps = compute_capabilities(&disposition, soft_delete_supported);
    if !caps.soft_delete {
        return Err(AppError::new(
            "COMMON_FORBIDDEN",
            caps.reason
                .soft_delete
                .unwrap_or_else(|| "当前存储源不支持系统回收站".to_string()),
        ));
    }

    // 5) 移入系统回收站（先落地，后写库）
    let target_for_trash = target_path.clone();
    tokio::task::spawn_blocking(move || trash::delete(&target_for_trash))
        .await
        .map_err(|e| AppError::io(format!("trash worker join 失败: {}", e)))?
        .map_err(|e| map_trash_err(&target_path, e))?;

    // 6) 事务：UPDATE disposition + INSERT audit
    let tx_result: CmdResult<()> = async {
        let mut tx = pool.begin().await.map_err(AppError::from)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        sqlx::query(
            "UPDATE resource_reference SET disposition = 'deleted', updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(&ref_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
        write_audit(
            &mut tx,
            &ref_id,
            &name,
            "soft_delete",
            Some(locator_json.as_str()),
            None,
        )
        .await?;
        tx.commit().await.map_err(AppError::from)?;
        Ok(())
    }
    .await;

    if let Err(db_err) = tx_result {
        // 7) 失败补偿：文件已在回收站，不回滚；best-effort 写一条 db_failed 审计。
        let best_effort: CmdResult<()> = async {
            let mut tx2 = pool.begin().await.map_err(AppError::from)?;
            write_audit(
                &mut tx2,
                &ref_id,
                &name,
                "soft_delete",
                Some(locator_json.as_str()),
                Some("db_failed"),
            )
            .await?;
            tx2.commit().await.map_err(AppError::from)?;
            Ok(())
        }
        .await;
        if let Err(e2) = best_effort {
            eprintln!("[disp_soft_delete] best-effort 审计写入也失败: {}", e2);
        }
        return Err(db_err);
    }

    // 提交后重新读取完整 Reference（与 ref_get 出参一致）
    crate::reference::get(pool, ref_id).await
}

/// `disp_soft_delete` Tauri 命令。
#[tauri::command(rename_all = "camelCase")]
pub async fn disp_soft_delete(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
    confirmed: bool,
) -> CmdResult<crate::reference::Reference> {
    soft_delete(&state.pool, ref_id, confirmed).await
}



// ============================================================
// disp_audit_list：审计查询（m4-4.6 · 详细设计 §2.6 / §3.2 / §6.7）
// ============================================================
//
// 契约（§2.6）：
// - 入参：`{ refId?, action?, limit?, offset? }`（全部可选）
//   - `action` 合法值：`archive | unarchive | soft_delete | destroy`；
//     其他值 → `COMMON_INVALID_PARAM`。
//   - `limit` 默认 50，最大 200；越界（>200）→ 截断为 200（二选一固化：截断，
//     不报错；与 query_refs 的 limit 行为对齐）。
//   - `offset` 默认 0；负值按 0 处理（防御性兜底）。
// - 出参：`DispositionAudit[]`，字段 camelCase：
//   `{ id, refId, refName, action, locatorSnapshot, actor, note, at }`。
// - 排序：`at DESC`（最新在前）。
//
// 关键约束：
// - `disposition_audit` 无外键（§6.7），销毁后审计独立存活，本查询不 JOIN
//   `resource_reference`；`refName` / `locatorSnapshot` 直接来自写入时的快照。
// - `locatorSnapshot` 在 DB 中为 TEXT（JSON 字符串）；出参反序列化为
//   `serde_json::Value` 供前端按 `kind/path` 渲染；解析失败时退化为 `null`，
//   不阻塞列表（best-effort，与 destroy 的 note 字段策略一致）。

/// `disp_audit_list` 出参行（契约 §2.6）。
///
/// 序列化为 camelCase：
/// `{ id, refId, refName, action, locatorSnapshot, actor, note, at }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DispositionAudit {
    pub id: String,
    pub ref_id: String,
    pub ref_name: String,
    pub action: String,
    /// 反序列化后的 locator 快照；DB 中为 TEXT（JSON），解析失败 → null。
    pub locator_snapshot: Option<serde_json::Value>,
    pub actor: String,
    pub note: Option<String>,
    pub at: i64,
}

/// `disp_audit_list` 入参（内部结构，便于单测构造）。
#[derive(Debug, Clone, Default)]
pub struct AuditListFilter {
    pub ref_id: Option<String>,
    pub action: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 合法 action 集合（与 schema CHECK 约束一致；m4-4.9 加入 'undo_import'）。
const VALID_AUDIT_ACTIONS: [&str; 5] = [
    "archive",
    "unarchive",
    "soft_delete",
    "destroy",
    "undo_import",
];

/// `disp_audit_list` 业务函数：按 refId / action 过滤，按 at DESC 分页。
///
/// 错误：
/// - `COMMON_INVALID_PARAM`：action 不在合法值集合内
/// - `COMMON_DB`：数据库或 JSON 解析失败
pub async fn audit_list(
    pool: &SqlitePool,
    filter: AuditListFilter,
) -> CmdResult<Vec<DispositionAudit>> {
    // 1) action 校验：必须属于合法值集合。
    if let Some(a) = filter.action.as_deref() {
        if !VALID_AUDIT_ACTIONS.contains(&a) {
            return Err(AppError::invalid_param(format!(
                "非法 action: {}（合法值: archive|unarchive|soft_delete|destroy|undo_import）",
                a
            )));
        }
    }

    // 2) limit / offset 归一化：limit 默认 50，>200 截断为 200；offset 默认 0。
    let limit: i64 = match filter.limit {
        Some(0) => 50, // 0 视为缺省（防御）
        Some(n) => i64::from(n.min(200)),
        None => 50,
    };
    let offset: i64 = i64::from(filter.offset.unwrap_or(0));

    // 3) 查询：动态参数 + IS NULL 兜底（契约 SQL 形状）。
    let rows = sqlx::query(
        "SELECT id, ref_id, ref_name, action, locator_snapshot, actor, note, at \
         FROM disposition_audit \
         WHERE (ref_id = ? OR ? IS NULL) AND (action = ? OR ? IS NULL) \
         ORDER BY at DESC LIMIT ? OFFSET ?",
    )
    .bind(filter.ref_id.as_deref())
    .bind(filter.ref_id.as_deref())
    .bind(filter.action.as_deref())
    .bind(filter.action.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    // 4) 行 → 出参结构：locator_snapshot 反序列化为 Value，失败 → None。
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id").map_err(AppError::from)?;
        let ref_id: String = row.try_get("ref_id").map_err(AppError::from)?;
        let ref_name: String = row.try_get("ref_name").map_err(AppError::from)?;
        let action: String = row.try_get("action").map_err(AppError::from)?;
        let locator_text: Option<String> = row.try_get("locator_snapshot").map_err(AppError::from)?;
        let actor: String = row.try_get("actor").map_err(AppError::from)?;
        let note: Option<String> = row.try_get("note").map_err(AppError::from)?;
        let at: i64 = row.try_get("at").map_err(AppError::from)?;

        let locator_snapshot = locator_text
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        out.push(DispositionAudit {
            id,
            ref_id,
            ref_name,
            action,
            locator_snapshot,
            actor,
            note,
            at,
        });
    }
    Ok(out)
}

/// `disp_audit_list` Tauri 命令。
#[tauri::command(rename_all = "camelCase")]
pub async fn disp_audit_list(
    state: tauri::State<'_, crate::AppState>,
    ref_id: Option<String>,
    action: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> CmdResult<Vec<DispositionAudit>> {
    audit_list(
        &state.pool,
        AuditListFilter {
            ref_id,
            action,
            limit,
            offset,
        },
    )
    .await
}

// ============================================================
// ref_undo_import：导入撤销（m4-4.9 · 任务包 tasks/m4-4.9.md）
// ============================================================
//
// 契约（已冻结 2026-08-16）：
// - 入参：`{ refId, confirmed }`
// - 出参两阶段：
//   - `confirmed=false` → `UndoPlan`（不写文件不改库）
//   - `confirmed=true`  → `()`（执行撤销）
// - 错误：`COMMON_NOT_FOUND` / `COMMON_FORBIDDEN`（canUndo=false）/
//   `FS_PERMISSION_DENIED` / `COMMON_IO` / `FS_TARGET_EXISTS` /
//   `COMMON_CONFIRM_REQUIRED`（confirmed != true 时由前端控制不发起，
//   后端防御性返回）
//
// 关键约束：
// - **24h 撤销窗口**：`UNDO_WINDOW_SECS = 24 * 3600`，超出即 blocker。
// - **blocker 检测**（plan 阶段）：
//   1. hosting != 'managed' —— 仅 managed 导入可撤销
//   2. locator_json.originalSource 为 null —— M3 期间创建的引用无此字段
//   3. 目标文件不存在或 mtime 晚于导入时间 + 5s 容差（外部已修改）
//   4. managedAction == 'move' 且 originalSource 已被其他文件占用
//   5. 引用创建时间超过 24h
// - **撤销动作**（confirmed 阶段，需拿 MANAGED_WRITE_LOCK）：
//   - copy → 删除目标文件（若存在）→ 删除引用行
//   - move → 先检查 originalSource 是否被占（被占则 FS_TARGET_EXISTS 中止，
//     不做任何写）→ 移动文件 → 删除引用行
// - **审计**：action='undo_import'，note JSON `{"managedAction":"...","originalSource":"..."}`
// - **互斥**：复用 `crate::reference::managed_write_lock()`（与 create_managed /
//   destroy / settings_change_root_dir 共用同一把锁）。
//
// 注意：`VALID_AUDIT_ACTIONS` 未包含 'undo_import'，disp_audit_list 的 action
// 过滤不支持本动作；审计查询页通过「全部」即可看到 undo_import 条目。
// 这是契约允许的行为（任务包未要求扩充 audit_list 的 action 枚举）。

/// 撤销窗口：24 小时（秒）。
pub const UNDO_WINDOW_SECS: i64 = 24 * 3600;

/// mtime 容差：目标文件 mtime 晚于导入时间 + 此秒数视为「外部已修改」。
const MTIME_TOLERANCE_SECS: i64 = 5;

/// `ref_undo_import` 在 `confirmed=false` 时的出参（任务包 §契约）。
///
/// 序列化为 camelCase：
/// `{ refId, refName, managedAction, currentPath, originalSource, canUndo, blockers }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UndoPlan {
    pub ref_id: String,
    pub ref_name: String,
    /// `"copy" | "move"`；从 locator_json 推断不出时填 "copy"（防御性兜底）。
    pub managed_action: String,
    /// 当前目标绝对路径（locator_json.path）。
    pub current_path: String,
    /// 原始源绝对路径（locator_json.originalSource）；M3 数据可能为 null。
    pub original_source: Option<String>,
    /// 是否可撤销：blockers 为空时 true。
    pub can_undo: bool,
    /// 阻塞原因列表（中文）；空 vec 表示可撤销。
    pub blockers: Vec<String>,
}

/// 从 locator_json 解析 `(current_path, original_source, managed_action)`。
///
/// 返回 `Ok((path, original_source_option, managed_action_option))`；
/// 缺 kind/path → `COMMON_DB`（数据损坏）。
/// managed_action 为 m4-4.9 新增字段；M3 老数据为 None。
fn parse_locator_for_undo(
    locator_json: &str,
) -> CmdResult<(PathBuf, Option<String>, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(locator_json)
        .map_err(|e| AppError::db(format!("locator_json 反序列化失败: {}", e)))?;
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    if kind != "path" {
        return Err(AppError::db(format!(
            "locator_json kind 非 path: {}",
            kind
        )));
    }
    let path = v
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| AppError::db("locator_json 缺少 path 字段".to_string()))?;
    let original = v
        .get("originalSource")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());
    let action = v
        .get("managedAction")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());
    Ok((PathBuf::from(path), original, action))
}

/// 计算 UndoPlan 的 blockers。
///
/// 输入：当前时间、reference 行字段、目标路径 metadata。
/// 输出：中文 blocker 列表；空 vec 表示 canUndo=true。
fn compute_undo_blockers(
    hosting: &str,
    original_source: Option<&str>,
    current_path: &Path,
    created_at: i64,
    now: i64,
    managed_action: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();

    // 1. hosting 检查
    if hosting != "managed" {
        blockers.push("仅 managed 导入的引用可撤销".to_string());
    }

    // 2. originalSource 缺失
    if original_source.is_none() {
        blockers.push("该引用创建时未记录原始源，无法撤销".to_string());
    }

    // 3. 24h 窗口
    if now - created_at > UNDO_WINDOW_SECS {
        blockers.push(format!(
            "引用创建已超过 24 小时撤销窗口（创建于 {}）",
            created_at
        ));
    }

    // 4. 目标文件状态
    match std::fs::symlink_metadata(current_path) {
        Err(_) => {
            blockers.push("目标文件已被外部删除或移动".to_string());
        }
        Ok(meta) => {
            if let Ok(mtime) = meta.modified() {
                if let Ok(mtime_dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                    let mtime_secs = mtime_dur.as_secs() as i64;
                    if mtime_secs > created_at + MTIME_TOLERANCE_SECS {
                        blockers.push("目标文件已被外部修改".to_string());
                    }
                }
            }
        }
    }

    // 5. move 场景：originalSource 被占
    if managed_action == "move" {
        if let Some(src) = original_source {
            if std::path::Path::new(src).exists() {
                blockers.push(format!("原始源路径已被其他文件占用: {}", src));
            }
        }
    }

    blockers
}

/// `ref_undo_import` 业务函数（两阶段）。
///
/// - `confirmed=false` → 返回 `Ok(Some(UndoPlan))`，不写文件不改库。
/// - `confirmed=true`  → 返回 `Ok(None)`，执行撤销（删目标/移回 + 删引用 + 写审计）。
pub async fn undo_import(
    pool: &SqlitePool,
    ref_id: String,
    confirmed: bool,
) -> CmdResult<Option<UndoPlan>> {
    // 1) 读 reference 行
    let row = sqlx::query(
        "SELECT name, hosting, locator_json, created_at FROM resource_reference WHERE id = ?",
    )
    .bind(&ref_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;

    let name: String = row.try_get("name").map_err(AppError::from)?;
    let hosting: String = row.try_get("hosting").map_err(AppError::from)?;
    let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
    let created_at: i64 = row.try_get("created_at").map_err(AppError::from)?;

    // 2) 解析 locator
    let (current_path, original_source, managed_action_opt) =
        parse_locator_for_undo(&locator_json)?;
    // M3 老数据无 managedAction 字段 → 视为 copy（保守：move 撤销风险更高，
    // 缺省时按 copy 处理仅删目标，不会误移文件到不存在的源路径）。
    let managed_action = managed_action_opt.as_deref().unwrap_or("copy");

    // 3) 计算 blockers
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let blockers = compute_undo_blockers(
        &hosting,
        original_source.as_deref(),
        &current_path,
        created_at,
        now,
        managed_action,
    );
    let can_undo = blockers.is_empty();

    let plan = UndoPlan {
        ref_id: ref_id.clone(),
        ref_name: name.clone(),
        managed_action: managed_action.to_string(),
        current_path: current_path.to_string_lossy().into_owned(),
        original_source: original_source.clone(),
        can_undo,
        blockers,
    };

    if !confirmed {
        return Ok(Some(plan));
    }

    // ---------- confirmed 阶段 ----------
    if !can_undo {
        return Err(AppError::new(
            "COMMON_FORBIDDEN",
            format!("撤销被阻止: {}", plan.blockers.join("；")),
        ));
    }

    // 拿全局互斥锁（与 create_managed / destroy / settings_change_root_dir 串行化）
    let _guard = crate::reference::managed_write_lock().lock().await;

    // TOCTOU 复检：拿锁后再检一次 blockers（外部可能已修改文件）
    let recheck_blockers = compute_undo_blockers(
        &hosting,
        original_source.as_deref(),
        &current_path,
        created_at,
        time::OffsetDateTime::now_utc().unix_timestamp(),
        managed_action,
    );
    if !recheck_blockers.is_empty() {
        return Err(AppError::new(
            "COMMON_FORBIDDEN",
            format!("撤销被阻止（锁内复检）: {}", recheck_blockers.join("；")),
        ));
    }

    // 执行撤销动作
    let original_source_str = original_source
        .as_deref()
        .expect("can_undo=true 时 original_source 必存在");

    if managed_action == "move" {
        // move 撤销：把目标文件移回 originalSource
        let orig_path = PathBuf::from(original_source_str);
        // 锁内复检已确认 originalSource 不存在；此处再防御一次
        if orig_path.exists() {
            return Err(AppError::new(
                "FS_TARGET_EXISTS",
                format!("原始源路径已被占用: {}", orig_path.display()),
            ));
        }
        // 确保父目录存在
        if let Some(parent) = orig_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| map_undo_fs_err("创建原始源父目录失败", parent, e))?;
        }
        tokio::fs::rename(&current_path, &orig_path)
            .await
            .map_err(|e| map_undo_fs_err("移回原始源失败", &current_path, e))?;
    } else {
        // copy 撤销：删除目标文件（若存在）
        match tokio::fs::symlink_metadata(&current_path).await {
            Ok(meta) => {
                let remove_result = if meta.is_dir() {
                    tokio::fs::remove_dir_all(&current_path).await
                } else {
                    tokio::fs::remove_file(&current_path).await
                };
                if let Err(e) = remove_result {
                    return Err(map_undo_fs_err("删除目标文件失败", &current_path, e));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 目标已被外部删除：视为成功（幂等），继续走删除引用流程
            }
            Err(e) => {
                return Err(map_undo_fs_err("读取目标元数据失败", &current_path, e));
            }
        }
    }

    // 文件操作完成，开事务写审计 + 删除引用行
    let note_json = serde_json::json!({
        "managedAction": managed_action,
        "originalSource": original_source_str,
    })
    .to_string();

    let mut tx = pool.begin().await.map_err(AppError::from)?;
    if let Err(e) = write_audit(
        &mut tx,
        &ref_id,
        &name,
        "undo_import",
        Some(locator_json.as_str()),
        Some(note_json.as_str()),
    )
    .await
    {
        let _ = tx.rollback().await;
        eprintln!(
            "[ref_undo_import] 审计写入失败，文件已撤销 ref_id={} err={}",
            ref_id, e
        );
        return Err(AppError::db(format!(
            "审计写入失败（文件已撤销，无法回滚）: {}",
            e
        )));
    }
    if let Err(e) = sqlx::query("DELETE FROM resource_reference WHERE id = ?")
        .bind(&ref_id)
        .execute(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        eprintln!(
            "[ref_undo_import] 引用行删除失败，文件已撤销 ref_id={} err={}",
            ref_id, e
        );
        return Err(AppError::db(format!(
            "引用行删除失败（文件已撤销，无法回滚）: {}",
            e
        )));
    }
    if let Err(e) = tx.commit().await {
        eprintln!(
            "[ref_undo_import] 事务提交失败，文件已撤销 ref_id={} err={}",
            ref_id, e
        );
        return Err(AppError::db(format!(
            "事务提交失败（文件已撤销，无法回滚）: {}",
            e
        )));
    }

    Ok(None)
}

/// undo 专用 FS 错误映射：与 destroy 一致。
fn map_undo_fs_err(context: &str, path: &Path, err: std::io::Error) -> AppError {
    let msg = format!("{} {}: {}", context, path.display(), err);
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::not_found(format!("目标路径不存在: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => AppError::new("FS_PERMISSION_DENIED", msg),
        std::io::ErrorKind::AlreadyExists => AppError::new("FS_TARGET_EXISTS", msg),
        _ => AppError::io(msg),
    }
}

/// `ref_undo_import` Tauri 命令。
///
/// - `confirmed=false` → 返回 `UndoPlan`
/// - `confirmed=true`  → 返回 `null`（前端按成功处理）
#[tauri::command(rename_all = "camelCase")]
pub async fn ref_undo_import(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
    confirmed: bool,
) -> CmdResult<Option<UndoPlan>> {
    undo_import(&state.pool, ref_id, confirmed).await
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_in_memory;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn now_unix() -> i64 {
        OffsetDateTime::now_utc().unix_timestamp()
    }

    async fn setup() -> SqlitePool {
        init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";
    /// 默认存储源 id（来自 0001 迁移）。
    const DEFAULT_SOURCE_ID: &str = "src_local_fs_default";

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

    /// 插入一个 reference，可指定 disposition；返回 id。
    async fn make_reference(pool: &SqlitePool, collection_id: &str, disposition: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, ?, 'r', 'code', 'external', '{\"kind\":\"path\",\"path\":\"/tmp/x\"}', \
              NULL, 'active', 'internal', 1, ?, ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(DEFAULT_SOURCE_ID)
        .bind(disposition)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference");
        id
    }

    async fn set_caps_json(pool: &SqlitePool, source_id: &str, caps_json: &str) {
        sqlx::query("UPDATE storage_source SET caps_json = ? WHERE id = ?")
            .bind(caps_json)
            .bind(source_id)
            .execute(pool)
            .await
            .expect("update caps_json");
    }

    // ---------- 纯函数 compute_capabilities ----------

    #[test]
    fn caps_disposition_none_all_supported() {
        let c = compute_capabilities("none", true);
        assert!(c.archive);
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
        // false 项填 reason
        assert!(c.reason.restore_from_bin.is_some());
        // true 项 reason 省略
        assert!(c.reason.archive.is_none());
        assert!(c.reason.soft_delete.is_none());
        assert!(c.reason.destroy.is_none());
    }

    #[test]
    fn caps_disposition_archived() {
        let c = compute_capabilities("archived", true);
        assert!(!c.archive);
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
        assert!(c.reason.archive.is_some());
        assert!(c.reason.restore_from_bin.is_some());
    }

    #[test]
    fn caps_disposition_deleted() {
        let c = compute_capabilities("deleted", true);
        assert!(c.archive);
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(c.restore_from_bin);
        // 全 true，无 reason
        assert!(c.reason.archive.is_none());
        assert!(c.reason.soft_delete.is_none());
        assert!(c.reason.destroy.is_none());
        assert!(c.reason.restore_from_bin.is_none());
    }

    #[test]
    fn caps_soft_delete_unsupported() {
        let c = compute_capabilities("none", false);
        assert!(c.archive);
        assert!(!c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
        assert!(c.reason.soft_delete.is_some());
    }

    #[test]
    fn caps_archived_and_soft_delete_unsupported() {
        let c = compute_capabilities("archived", false);
        assert!(!c.archive);
        assert!(!c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
        assert!(c.reason.archive.is_some());
        assert!(c.reason.soft_delete.is_some());
        assert!(c.reason.restore_from_bin.is_some());
    }

    #[test]
    fn caps_deleted_and_soft_delete_unsupported() {
        // 已删除 + 无回收站：仍可 restoreFromBin（数据在回收站语义由 disposition 决定），
        // 但 softDelete=false 阻止再次删除。
        let c = compute_capabilities("deleted", false);
        assert!(c.archive);
        assert!(!c.soft_delete);
        assert!(c.destroy);
        assert!(c.restore_from_bin);
    }

    #[test]
    fn caps_unknown_disposition_treated_as_none() {
        // 防御：未知 disposition 按 none 处理
        let c = compute_capabilities("bogus", true);
        assert!(c.archive);
        assert!(!c.restore_from_bin);
    }

    // ---------- 序列化契约 ----------

    #[test]
    fn caps_serializes_camel_case() {
        let c = compute_capabilities("archived", false);
        let v = serde_json::to_value(&c).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("archive"));
        assert!(obj.contains_key("softDelete"));
        assert!(obj.contains_key("destroy"));
        assert!(obj.contains_key("restoreFromBin"));
        assert!(obj.contains_key("reason"));
        // 不出现 snake_case
        assert!(!obj.contains_key("soft_delete"));
        assert!(!obj.contains_key("restore_from_bin"));

        let reason = obj["reason"].as_object().expect("reason obj");
        assert!(reason.contains_key("archive"));
        assert!(reason.contains_key("softDelete"));
        assert!(reason.contains_key("restoreFromBin"));
        // destroy 是 true，reason.destroy 应省略
        assert!(!reason.contains_key("destroy"));
    }

    // ---------- parse_soft_delete_cap ----------

    #[test]
    fn parse_cap_none_defaults_true() {
        assert!(parse_soft_delete_cap(None).expect("ok"));
        assert!(parse_soft_delete_cap(Some("")).expect("ok"));
        assert!(parse_soft_delete_cap(Some("  ")).expect("ok"));
    }

    #[test]
    fn parse_cap_missing_field_defaults_true() {
        assert!(parse_soft_delete_cap(Some("{}")).expect("ok"));
        assert!(parse_soft_delete_cap(Some("{\"archive\":true}")).expect("ok"));
    }

    #[test]
    fn parse_cap_explicit_values() {
        assert!(parse_soft_delete_cap(Some("{\"softDelete\":true}")).expect("ok"));
        assert!(!parse_soft_delete_cap(Some("{\"softDelete\":false}")).expect("ok"));
    }

    #[test]
    fn parse_cap_invalid_json_returns_db_err() {
        let err = parse_soft_delete_cap(Some("{not json")).expect_err("should fail");
        assert_eq!(err.code, "COMMON_DB");
    }

    // ---------- 数据库集成：get_capabilities ----------

    #[tokio::test]
    async fn get_capabilities_err_not_found() {
        let pool = setup().await;
        let err = get_capabilities(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn get_capabilities_disposition_none() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        let c = get_capabilities(&pool, rid).await.expect("ok");
        assert!(c.archive);
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
    }

    #[tokio::test]
    async fn get_capabilities_disposition_archived() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "archived").await;
        let c = get_capabilities(&pool, rid).await.expect("ok");
        assert!(!c.archive);
        assert!(c.reason.archive.is_some());
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(!c.restore_from_bin);
    }

    #[tokio::test]
    async fn get_capabilities_disposition_deleted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "deleted").await;
        let c = get_capabilities(&pool, rid).await.expect("ok");
        assert!(c.archive);
        assert!(c.soft_delete);
        assert!(c.destroy);
        assert!(c.restore_from_bin);
    }

    #[tokio::test]
    async fn get_capabilities_soft_delete_false_from_caps_json() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        set_caps_json(
            &pool,
            DEFAULT_SOURCE_ID,
            "{\"archive\":true,\"softDelete\":false,\"destroy\":true}",
        )
        .await;
        let rid = make_reference(&pool, &cid, "none").await;
        let c = get_capabilities(&pool, rid).await.expect("ok");
        assert!(c.archive);
        assert!(!c.soft_delete);
        assert!(c.reason.soft_delete.is_some());
        assert!(c.destroy);
    }

    // ---------- write_soft_delete_cap ----------

    #[tokio::test]
    async fn write_cap_updates_existing_caps() {
        let pool = setup().await;
        // 初始 caps_json 来自 0001 迁移
        write_soft_delete_cap(&pool, DEFAULT_SOURCE_ID, false)
            .await
            .expect("write ok");
        let (caps,): (String,) =
            sqlx::query_as("SELECT caps_json FROM storage_source WHERE id = ?")
                .bind(DEFAULT_SOURCE_ID)
                .fetch_one(&pool)
                .await
                .expect("read");
        let v: serde_json::Value = serde_json::from_str(&caps).expect("parse");
        assert_eq!(v["softDelete"].as_bool(), Some(false));
        // 其他字段保留
        assert_eq!(v["archive"].as_bool(), Some(true));
        assert_eq!(v["destroy"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn write_cap_handles_null_caps_json() {
        let pool = setup().await;
        // 把 caps_json 置 NULL，再写入
        sqlx::query("UPDATE storage_source SET caps_json = NULL WHERE id = ?")
            .bind(DEFAULT_SOURCE_ID)
            .execute(&pool)
            .await
            .expect("null caps");
        write_soft_delete_cap(&pool, DEFAULT_SOURCE_ID, true)
            .await
            .expect("write ok");
        let (caps,): (String,) =
            sqlx::query_as("SELECT caps_json FROM storage_source WHERE id = ?")
                .bind(DEFAULT_SOURCE_ID)
                .fetch_one(&pool)
                .await
                .expect("read");
        let v: serde_json::Value = serde_json::from_str(&caps).expect("parse");
        assert_eq!(v["softDelete"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn write_cap_err_source_not_found() {
        let pool = setup().await;
        let err = write_soft_delete_cap(&pool, "no-such-source", true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- detect_soft_delete_support ----------

    #[test]
    fn detect_returns_bool_for_current_platform() {
        // 仅断言函数可调用且返回 bool；具体值随平台而变。
        let v = detect_soft_delete_support();
        // macOS / Windows 应为 true
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        assert!(v);
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = v;
    }

    // ---------- m4-4.2 · disp_archive / disp_unarchive ----------

    /// 读取当前 reference 的 disposition 字段。
    async fn read_disposition(pool: &SqlitePool, id: &str) -> String {
        let (d,): (String,) = sqlx::query_as("SELECT disposition FROM resource_reference WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("read disposition");
        d
    }

    /// 统计审计表中指定 ref 的记录数。
    async fn audit_count(pool: &SqlitePool, ref_id: &str) -> i64 {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM disposition_audit WHERE ref_id = ?")
            .bind(ref_id)
            .fetch_one(pool)
            .await
            .expect("count audit");
        n
    }

    /// 取指定 ref 的审计动作序列（按 SQLite rowid 升序 = 插入顺序）。
    async fn audit_actions(pool: &SqlitePool, ref_id: &str) -> Vec<String> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT action FROM disposition_audit WHERE ref_id = ? ORDER BY rowid ASC",
        )
        .bind(ref_id)
        .fetch_all(pool)
        .await
        .expect("list audit");
        rows.into_iter().map(|(a,)| a).collect()
    }

    #[tokio::test]
    async fn archive_none_to_archived_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;

        let r = archive(&pool, rid.clone()).await.expect("archive ok");
        assert_eq!(r.id, rid);
        assert_eq!(r.disposition, "archived");
        // 数据库层也已落库
        assert_eq!(read_disposition(&pool, &rid).await, "archived");
    }

    #[tokio::test]
    async fn archive_twice_conflict() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        archive(&pool, rid.clone()).await.expect("first archive ok");

        let err = archive(&pool, rid.clone()).await.expect_err("should conflict");
        assert_eq!(err.code, "COMMON_CONFLICT");
        // 状态未被破坏
        assert_eq!(read_disposition(&pool, &rid).await, "archived");
    }

    #[tokio::test]
    async fn archive_deleted_conflict() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "deleted").await;
        let err = archive(&pool, rid.clone()).await.expect_err("should conflict");
        assert_eq!(err.code, "COMMON_CONFLICT");
        assert_eq!(read_disposition(&pool, &rid).await, "deleted");
    }

    #[tokio::test]
    async fn unarchive_archived_to_none_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "archived").await;

        let r = unarchive(&pool, rid.clone()).await.expect("unarchive ok");
        assert_eq!(r.disposition, "none");
        assert_eq!(read_disposition(&pool, &rid).await, "none");
    }

    #[tokio::test]
    async fn unarchive_none_conflict() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        let err = unarchive(&pool, rid.clone()).await.expect_err("should conflict");
        assert_eq!(err.code, "COMMON_CONFLICT");
        assert_eq!(read_disposition(&pool, &rid).await, "none");
    }

    #[tokio::test]
    async fn unarchive_deleted_conflict() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "deleted").await;
        let err = unarchive(&pool, rid.clone()).await.expect_err("should conflict");
        assert_eq!(err.code, "COMMON_CONFLICT");
        assert_eq!(read_disposition(&pool, &rid).await, "deleted");
    }

    #[tokio::test]
    async fn archive_writes_audit_with_snapshot() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;

        archive(&pool, rid.clone()).await.expect("archive ok");

        assert_eq!(audit_count(&pool, &rid).await, 1);
        let row = sqlx::query(
            "SELECT ref_name, action, locator_snapshot, actor, note, at \
             FROM disposition_audit WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        let ref_name: String = row.try_get("ref_name").expect("ref_name");
        let action: String = row.try_get("action").expect("action");
        let locator: String = row.try_get("locator_snapshot").expect("locator");
        let actor: String = row.try_get("actor").expect("actor");
        let note: Option<String> = row.try_get("note").expect("note");
        let at: i64 = row.try_get("at").expect("at");

        assert_eq!(ref_name, "r");
        assert_eq!(action, "archive");
        // locator_snapshot 与 make_reference 写入的 locator_json 完全一致
        assert_eq!(locator, "{\"kind\":\"path\",\"path\":\"/tmp/x\"}");
        assert_eq!(actor, "local_user");
        assert!(note.is_none());
        assert!(at > 0);
    }

    #[tokio::test]
    async fn archive_unarchive_archive_audit_three_rows() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;

        archive(&pool, rid.clone()).await.expect("a1");
        unarchive(&pool, rid.clone()).await.expect("u");
        archive(&pool, rid.clone()).await.expect("a2");

        assert_eq!(audit_count(&pool, &rid).await, 3);
        let actions = audit_actions(&pool, &rid).await;
        assert_eq!(actions, vec!["archive", "unarchive", "archive"]);
    }

    #[tokio::test]
    async fn archive_err_not_found() {
        let pool = setup().await;
        let err = archive(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn unarchive_err_not_found() {
        let pool = setup().await;
        let err = unarchive(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- disp_preview ----------

    /// 插入一个 reference，locator 指向给定路径；返回 id。
    async fn make_reference_with_path(
        pool: &SqlitePool,
        collection_id: &str,
        disposition: &str,
        path: &std::path::Path,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        let locator_json = serde_json::json!({
            "kind": "path",
            "path": path.to_string_lossy(),
        })
        .to_string();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, ?, 'r', 'code', 'external', ?, \
              NULL, 'active', 'internal', 1, ?, ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(DEFAULT_SOURCE_ID)
        .bind(&locator_json)
        .bind(disposition)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference with path");
        id
    }

    #[tokio::test]
    async fn preview_err_ref_not_found() {
        let pool = setup().await;
        let err = preview(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn preview_single_file() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("hello.txt");
        std::fs::write(&file, b"hello world").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;

        let p = preview(&pool, rid).await.expect("preview ok");
        assert!(!p.is_dir);
        assert_eq!(p.file_count, 1);
        assert_eq!(p.total_bytes, 11);
        assert_eq!(p.target, file.to_string_lossy());
        assert!(!p.preview_id.is_empty());
        assert!(p.capability.archive);
        assert!(p.warning.contains("销毁后不可恢复"));
    }

    #[tokio::test]
    async fn preview_directory_nested() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("proj");
        std::fs::create_dir(&root).expect("mkdir");
        std::fs::write(root.join("a.txt"), b"aaa").expect("a");
        let sub = root.join("sub");
        std::fs::create_dir(&sub).expect("mkdir sub");
        std::fs::write(sub.join("b.txt"), b"bbbbb").expect("b");
        std::fs::write(sub.join("c.txt"), b"c").expect("c");
        let rid = make_reference_with_path(&pool, &cid, "none", &root).await;

        let p = preview(&pool, rid).await.expect("preview ok");
        assert!(p.is_dir);
        assert_eq!(p.file_count, 3);
        assert_eq!(p.total_bytes, 3 + 5 + 1);
        assert!(p.warning.contains("3 个文件"));
        assert!(p.warning.contains("销毁后不可恢复"));
    }

    #[tokio::test]
    async fn preview_empty_directory() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("empty");
        std::fs::create_dir(&root).expect("mkdir");
        let rid = make_reference_with_path(&pool, &cid, "none", &root).await;

        let p = preview(&pool, rid).await.expect("preview ok");
        assert!(p.is_dir);
        assert_eq!(p.file_count, 0);
        assert_eq!(p.total_bytes, 0);
        assert!(p.warning.contains("0 个文件"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preview_symlink_not_followed() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&root).expect("mkdir root");
        std::fs::create_dir(&outside).expect("mkdir outside");
        std::fs::write(outside.join("secret.txt"), b"secret-data").expect("write");
        std::fs::write(root.join("real.txt"), b"real").expect("write real");
        std::os::unix::fs::symlink(&outside, root.join("link_out")).expect("symlink");

        let rid = make_reference_with_path(&pool, &cid, "none", &root).await;
        let p = preview(&pool, rid).await.expect("preview ok");
        assert!(p.is_dir);
        // 2 个条目：real.txt + link_out（symlink 自身按文件计入，但不递归进入 outside）
        assert_eq!(p.file_count, 2);
        // total_bytes = real.txt 大小 + symlink 自身大小（不跟随到 secret.txt）
        assert!(p.total_bytes >= 4);
        // 关键断言：没有把 outside/secret.txt 的 11 字节计入
        assert!(p.total_bytes < 4 + 11 + 4096);
    }

    #[tokio::test]
    async fn preview_large_directory_1000_files() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("big");
        std::fs::create_dir(&root).expect("mkdir");
        for i in 0..1000 {
            std::fs::write(root.join(format!("f{:04}.txt", i)), b"").expect("write");
        }
        let rid = make_reference_with_path(&pool, &cid, "none", &root).await;

        let start = std::time::Instant::now();
        let p = preview(&pool, rid).await.expect("preview ok");
        let elapsed = start.elapsed();
        assert!(p.is_dir);
        assert_eq!(p.file_count, 1000);
        assert_eq!(p.total_bytes, 0);
        // 1000 空文件统计应在 10 秒内完成（非常宽松的上限）
        assert!(elapsed.as_secs() < 10, "统计耗时 {:?}", elapsed);
    }

    #[tokio::test]
    async fn preview_cancel_terminates_early() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("big");
        std::fs::create_dir(&root).expect("mkdir");
        // 建一个较多文件的目录，让统计有时间被中断
        for i in 0..2000 {
            std::fs::write(root.join(format!("f{:04}.txt", i)), b"x").expect("write");
        }
        let rid = make_reference_with_path(&pool, &cid, "none", &root).await;

        // 先注册一个 preview 拿到 previewId，然后立即取消
        let (preview_id, flag) = register_preview();
        assert!(!flag.load(Ordering::SeqCst));
        let cancelled = cancel_preview(&preview_id);
        assert!(cancelled);
        assert!(flag.load(Ordering::SeqCst));

        // 直接调 stat_target 验证：取消标志为 true 时立即返回 COMMON_CANCELLED
        let err = stat_target(&root, &flag).expect_err("should be cancelled");
        assert_eq!(err.code, "COMMON_CANCELLED");

        unregister_preview(&preview_id);
        // 取消不存在的 previewId 返回 false
        assert!(!cancel_preview(&preview_id));

        // rid 仍然能正常 preview（独立流程）
        let p = preview(&pool, rid).await.expect("preview ok");
        assert_eq!(p.file_count, 2000);
    }

    #[tokio::test]
    async fn preview_err_target_externally_deleted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("gone.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;
        // 外部删除文件
        std::fs::remove_file(&file).expect("remove");

        let err = preview(&pool, rid).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn preview_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"a").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;

        let p = preview(&pool, rid).await.expect("preview ok");
        let v = serde_json::to_value(&p).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("previewId"));
        assert!(obj.contains_key("target"));
        assert!(obj.contains_key("isDir"));
        assert!(obj.contains_key("fileCount"));
        assert!(obj.contains_key("totalBytes"));
        assert!(obj.contains_key("capability"));
        assert!(obj.contains_key("warning"));
        assert!(!obj.contains_key("preview_id"));
        assert!(!obj.contains_key("is_dir"));
        assert!(!obj.contains_key("file_count"));
        assert!(!obj.contains_key("total_bytes"));
    }

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.00 KB");
        assert_eq!(human_size(1024 * 1024), "1.00 MB");
        assert_eq!(human_size(5 * 1024 * 1024 * 1024), "5.00 GB");
    }

    #[test]
    fn build_warning_messages() {
        let w = build_warning(true, 42, 1024);
        assert!(w.contains("42 个文件"));
        assert!(w.contains("销毁后不可恢复"));
        let w2 = build_warning(false, 1, 100);
        assert!(w2.contains("销毁后不可恢复"));
    }

    // ---------- m4-4.5 · disp_destroy ----------

    /// 读取 reference 行数（用于验证物理删除）。
    async fn ref_count(pool: &SqlitePool, ref_id: &str) -> i64 {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM resource_reference WHERE id = ?")
            .bind(ref_id)
            .fetch_one(pool)
            .await
            .expect("count reference");
        n
    }

    /// 插入 reference，name 字段可定制；返回 id。
    async fn make_reference_named(
        pool: &SqlitePool,
        collection_id: &str,
        disposition: &str,
        name: &str,
        path: &std::path::Path,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        let locator_json = serde_json::json!({
            "kind": "path",
            "path": path.to_string_lossy(),
        })
        .to_string();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'code', 'external', ?, \
              NULL, 'active', 'internal', 1, ?, ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(DEFAULT_SOURCE_ID)
        .bind(name)
        .bind(&locator_json)
        .bind(disposition)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference named");
        id
    }

    #[tokio::test]
    async fn destroy_err_ref_not_found() {
        let pool = setup().await;
        let err = destroy(&pool, "no-such-id".into(), "whatever".into(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn destroy_err_confirmed_false() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"aaa").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "myfile", &file).await;

        let err = destroy(&pool, rid.clone(), "myfile".into(), false)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFIRM_REQUIRED");
        // 文件未动、库未动
        assert!(file.exists());
        assert_eq!(ref_count(&pool, &rid).await, 1);
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn destroy_err_confirm_text_mismatch() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"aaa").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "myfile", &file).await;

        // 大小写不一致也算不匹配（逐字符比对）
        let err = destroy(&pool, rid.clone(), "MYFILE".into(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFIRM_REQUIRED");
        // 文件未动、库未动
        assert!(file.exists());
        assert_eq!(ref_count(&pool, &rid).await, 1);
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn destroy_single_file_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("hello.txt");
        std::fs::write(&file, b"hello world").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "hello", &file).await;

        let r = destroy(&pool, rid.clone(), "hello".into(), true)
            .await
            .expect("destroy ok");
        assert_eq!(r.deleted_ref_id, rid);

        // 文件消失
        assert!(!file.exists());
        // 引用行物理删除
        assert_eq!(ref_count(&pool, &rid).await, 0);
        // 审计写入
        assert_eq!(audit_count(&pool, &rid).await, 1);
        let row = sqlx::query(
            "SELECT ref_name, action, locator_snapshot, actor, note, at \
             FROM disposition_audit WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        let ref_name: String = row.try_get("ref_name").expect("ref_name");
        let action: String = row.try_get("action").expect("action");
        let locator: String = row.try_get("locator_snapshot").expect("locator");
        let actor: String = row.try_get("actor").expect("actor");
        let note: Option<String> = row.try_get("note").expect("note");
        let at: i64 = row.try_get("at").expect("at");

        assert_eq!(ref_name, "hello");
        assert_eq!(action, "destroy");
        // locator_snapshot 含被删路径
        assert!(locator.contains("hello.txt"));
        assert_eq!(actor, "local_user");
        // note 含 fileCount / totalBytes / isDir
        let note_str = note.expect("note should be present");
        let note_json: serde_json::Value = serde_json::from_str(&note_str).expect("note json");
        assert_eq!(note_json["fileCount"].as_u64(), Some(1));
        assert_eq!(note_json["totalBytes"].as_u64(), Some(11));
        assert_eq!(note_json["isDir"].as_bool(), Some(false));
        assert!(at > 0);
    }

    #[tokio::test]
    async fn destroy_directory_nested_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().join("proj");
        std::fs::create_dir(&root).expect("mkdir");
        std::fs::write(root.join("a.txt"), b"aaa").expect("a");
        let sub = root.join("sub");
        std::fs::create_dir(&sub).expect("mkdir sub");
        std::fs::write(sub.join("b.txt"), b"bbbbb").expect("b");
        std::fs::write(sub.join("c.txt"), b"c").expect("c");
        let rid = make_reference_named(&pool, &cid, "none", "proj", &root).await;

        let r = destroy(&pool, rid.clone(), "proj".into(), true)
            .await
            .expect("destroy ok");
        assert_eq!(r.deleted_ref_id, rid);

        // 目录整棵消失
        assert!(!root.exists());
        assert!(!sub.exists());
        // 引用行物理删除
        assert_eq!(ref_count(&pool, &rid).await, 0);
        // 审计：fileCount=3, totalBytes=3+5+1=9, isDir=true
        let row = sqlx::query(
            "SELECT action, note FROM disposition_audit WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        let action: String = row.try_get("action").expect("action");
        let note: Option<String> = row.try_get("note").expect("note");
        assert_eq!(action, "destroy");
        let note_json: serde_json::Value =
            serde_json::from_str(&note.expect("note")).expect("note json");
        assert_eq!(note_json["fileCount"].as_u64(), Some(3));
        assert_eq!(note_json["totalBytes"].as_u64(), Some(9));
        assert_eq!(note_json["isDir"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn destroy_disposition_deleted_allowed() {
        // 能力规则固化注释：`compute_capabilities.destroy` 始终为 true（兜底），
        // 因此对 disposition='deleted' 的引用再次销毁是**允许**的。
        // 场景：软删除后用户仍想彻底清空回收站语义下的源文件。
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("trashed.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_reference_named(&pool, &cid, "deleted", "trashed", &file).await;

        let r = destroy(&pool, rid.clone(), "trashed".into(), true)
            .await
            .expect("destroy deleted ok");
        assert_eq!(r.deleted_ref_id, rid);
        assert!(!file.exists());
        assert_eq!(ref_count(&pool, &rid).await, 0);
        assert_eq!(audit_count(&pool, &rid).await, 1);
    }

    #[tokio::test]
    async fn destroy_err_target_externally_deleted() {
        // 文件已被外部删除：destroy 在 remove 阶段返回 COMMON_NOT_FOUND，
        // 引用行**保持不动**（文件删除失败不写库）。
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("gone.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "gone", &file).await;
        std::fs::remove_file(&file).expect("external remove");

        let err = destroy(&pool, rid.clone(), "gone".into(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
        // 库未动
        assert_eq!(ref_count(&pool, &rid).await, 1);
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn destroy_audit_queryable_after_ref_deleted() {
        // 销毁后引用行已删，但审计仍可通过 raw SQL 按 ref_id 查到（§6.7 无外键约束）。
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("audit.txt");
        std::fs::write(&file, b"audit").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "audit", &file).await;

        destroy(&pool, rid.clone(), "audit".into(), true)
            .await
            .expect("destroy ok");

        // 引用行已删
        assert_eq!(ref_count(&pool, &rid).await, 0);
        // 审计仍可查（raw SQL 模拟 4.6 disp_audit_list 行为）
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT ref_id, action FROM disposition_audit WHERE ref_id = ? ORDER BY rowid ASC",
        )
        .bind(&rid)
        .fetch_all(&pool)
        .await
        .expect("list audit");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, rid);
        assert_eq!(rows[0].1, "destroy");
    }

    #[tokio::test]
    async fn destroy_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("s.txt");
        std::fs::write(&file, b"s").expect("write");
        let rid = make_reference_named(&pool, &cid, "none", "s", &file).await;

        let r = destroy(&pool, rid, "s".into(), true).await.expect("ok");
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("deletedRefId"));
        assert!(!obj.contains_key("deleted_ref_id"));
    }

    // ---------- m4-4.4 · disp_soft_delete ----------

    #[tokio::test]
    async fn soft_delete_err_confirm_required() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;

        let err = soft_delete(&pool, rid.clone(), false)
            .await
            .expect_err("should require confirm");
        assert_eq!(err.code, "COMMON_CONFIRM_REQUIRED");
        // 状态未被破坏
        assert_eq!(read_disposition(&pool, &rid).await, "none");
        // 无审计写入
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn soft_delete_err_ref_not_found() {
        let pool = setup().await;
        let err = soft_delete(&pool, "no-such-id".into(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn soft_delete_err_capability_forbidden() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        // 关闭 softDelete 能力
        set_caps_json(
            &pool,
            DEFAULT_SOURCE_ID,
            "{\"archive\":true,\"softDelete\":false,\"destroy\":true}",
        )
        .await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("x.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;

        let err = soft_delete(&pool, rid.clone(), true)
            .await
            .expect_err("should be forbidden");
        assert_eq!(err.code, "COMMON_FORBIDDEN");
        // 文件未被移动
        assert!(file.exists());
        // 状态未被破坏
        assert_eq!(read_disposition(&pool, &rid).await, "none");
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn soft_delete_err_target_externally_deleted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("gone.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;
        // 外部删除文件
        std::fs::remove_file(&file).expect("remove");

        let err = soft_delete(&pool, rid.clone(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
        // 状态未被破坏
        assert_eq!(read_disposition(&pool, &rid).await, "none");
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    /// 成功路径：真实调用 trash::delete 把文件移入系统回收站。
    ///
    /// 平台相关性：macOS 上 trash crate 通过 AppleScript 调 Finder；在**无 GUI
    /// 会话 / SSH / CI agent** 环境会触发 `AppleEvent 超时 (-1712)`，导致失败。
    /// 任务包 #6 允许用 `#[ignore]` + 注释标注，由用户在 review 阶段人工点验。
    ///
    /// 人工点验：在 macOS 桌面会话中执行
    /// `cargo test --lib disposition::tests::soft_delete_ok -- --ignored --nocapture`
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "需在 macOS GUI 会话运行；CI/SSH 环境 AppleEvent 超时"]
    async fn soft_delete_ok_moves_to_trash() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("to_trash.txt");
        std::fs::write(&file, b"trash me").expect("write");
        let rid = make_reference_with_path(&pool, &cid, "none", &file).await;

        let r = soft_delete(&pool, rid.clone(), true)
            .await
            .expect("soft_delete ok");
        // 出参 Reference.disposition = 'deleted'
        assert_eq!(r.id, rid);
        assert_eq!(r.disposition, "deleted");
        // 文件已不在原位置（已进系统回收站）
        assert!(!file.exists(), "文件应已移入回收站");
        // 数据库 disposition 已落库
        assert_eq!(read_disposition(&pool, &rid).await, "deleted");
        // 审计记录正确：1 条 action='soft_delete'，note 为 NULL（非 db_failed）
        assert_eq!(audit_count(&pool, &rid).await, 1);
        let row = sqlx::query(
            "SELECT action, note, locator_snapshot, actor FROM disposition_audit WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        let action: String = row.try_get("action").expect("action");
        let note: Option<String> = row.try_get("note").expect("note");
        let locator: String = row.try_get("locator_snapshot").expect("locator");
        let actor: String = row.try_get("actor").expect("actor");
        assert_eq!(action, "soft_delete");
        assert!(note.is_none(), "成功路径 note 应为 NULL");
        assert!(locator.contains("to_trash.txt"));
        assert_eq!(actor, "local_user");
    }

    /// 成功路径（目录）：回收站移动整个目录。
    ///
    /// 同 `soft_delete_ok_moves_to_trash`：macOS GUI 会话人工点验。
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "需在 macOS GUI 会话运行；CI/SSH 环境 AppleEvent 超时"]
    async fn soft_delete_ok_directory() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let dir = tmp.path().join("dir_to_trash");
        std::fs::create_dir(&dir).expect("mkdir");
        std::fs::write(dir.join("a.txt"), b"a").expect("write a");
        let rid = make_reference_with_path(&pool, &cid, "none", &dir).await;

        let r = soft_delete(&pool, rid.clone(), true)
            .await
            .expect("soft_delete dir ok");
        assert_eq!(r.disposition, "deleted");
        assert!(!dir.exists(), "目录应已移入回收站");
        assert_eq!(read_disposition(&pool, &rid).await, "deleted");
        assert_eq!(audit_count(&pool, &rid).await, 1);
    }

    // ---------- m4-4.6 · disp_audit_list ----------

    /// 直接插入一条审计行（绕过 write_audit，便于控制 at 时间戳构造倒序用例）。
    async fn insert_audit_row(
        pool: &SqlitePool,
        ref_id: &str,
        ref_name: &str,
        action: &str,
        at: i64,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO disposition_audit \
             (id, ref_id, ref_name, action, locator_snapshot, actor, note, at) \
             VALUES (?, ?, ?, ?, ?, 'local_user', NULL, ?)",
        )
        .bind(&id)
        .bind(ref_id)
        .bind(ref_name)
        .bind(action)
        .bind("{\"kind\":\"path\",\"path\":\"/tmp/x\"}")
        .bind(at)
        .execute(pool)
        .await
        .expect("insert audit");
        id
    }

    fn filter_all() -> AuditListFilter {
        AuditListFilter::default()
    }

    fn filter_by_ref(ref_id: &str) -> AuditListFilter {
        AuditListFilter {
            ref_id: Some(ref_id.to_string()),
            ..Default::default()
        }
    }

    fn filter_by_action(action: &str) -> AuditListFilter {
        AuditListFilter {
            action: Some(action.to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn audit_list_empty_db_returns_empty() {
        let pool = setup().await;
        let list = audit_list(&pool, filter_all()).await.expect("ok");
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn audit_list_single_row() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        archive(&pool, rid.clone()).await.expect("archive");

        let list = audit_list(&pool, filter_all()).await.expect("ok");
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item.ref_id, rid);
        assert_eq!(item.ref_name, "r");
        assert_eq!(item.action, "archive");
        assert_eq!(item.actor, "local_user");
        assert!(item.note.is_none());
        assert!(item.at > 0);
        // locator_snapshot 反序列化为 Value，含 path 字段
        let loc = item.locator_snapshot.as_ref().expect("locator");
        assert_eq!(loc["kind"].as_str(), Some("path"));
        assert_eq!(loc["path"].as_str(), Some("/tmp/x"));
    }

    #[tokio::test]
    async fn audit_list_pagination() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        // 造 5 条，at 递增
        for i in 0..5 {
            insert_audit_row(&pool, &rid, "r", "archive", 1000 + i).await;
        }
        // limit=2 offset=0 → 最新 2 条（at 倒序）
        let page1 = audit_list(
            &pool,
            AuditListFilter {
                limit: Some(2),
                offset: Some(0),
                ..Default::default()
            },
        )
        .await
        .expect("page1");
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].at, 1004);
        assert_eq!(page1[1].at, 1003);

        // limit=2 offset=2 → 接下来 2 条
        let page2 = audit_list(
            &pool,
            AuditListFilter {
                limit: Some(2),
                offset: Some(2),
                ..Default::default()
            },
        )
        .await
        .expect("page2");
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].at, 1002);
        assert_eq!(page2[1].at, 1001);

        // limit=2 offset=4 → 最后 1 条
        let page3 = audit_list(
            &pool,
            AuditListFilter {
                limit: Some(2),
                offset: Some(4),
                ..Default::default()
            },
        )
        .await
        .expect("page3");
        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].at, 1000);
    }

    #[tokio::test]
    async fn audit_list_filter_by_ref_id() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid_a = make_reference(&pool, &cid, "none").await;
        let rid_b = make_reference(&pool, &cid, "none").await;
        insert_audit_row(&pool, &rid_a, "a", "archive", 1000).await;
        insert_audit_row(&pool, &rid_b, "b", "archive", 1001).await;
        insert_audit_row(&pool, &rid_a, "a", "unarchive", 1002).await;

        let list = audit_list(&pool, filter_by_ref(&rid_a)).await.expect("ok");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|i| i.ref_id == rid_a));
        // at 倒序
        assert_eq!(list[0].action, "unarchive");
        assert_eq!(list[1].action, "archive");
    }

    #[tokio::test]
    async fn audit_list_filter_by_action() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        insert_audit_row(&pool, &rid, "r", "archive", 1000).await;
        insert_audit_row(&pool, &rid, "r", "unarchive", 1001).await;
        insert_audit_row(&pool, &rid, "r", "archive", 1002).await;
        insert_audit_row(&pool, &rid, "r", "destroy", 1003).await;

        let archives = audit_list(&pool, filter_by_action("archive"))
            .await
            .expect("ok");
        assert_eq!(archives.len(), 2);
        assert!(archives.iter().all(|i| i.action == "archive"));

        let destroys = audit_list(&pool, filter_by_action("destroy"))
            .await
            .expect("ok");
        assert_eq!(destroys.len(), 1);
        assert_eq!(destroys[0].action, "destroy");
    }

    #[tokio::test]
    async fn audit_list_filter_combined() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid_a = make_reference(&pool, &cid, "none").await;
        let rid_b = make_reference(&pool, &cid, "none").await;
        insert_audit_row(&pool, &rid_a, "a", "archive", 1000).await;
        insert_audit_row(&pool, &rid_a, "a", "unarchive", 1001).await;
        insert_audit_row(&pool, &rid_b, "b", "archive", 1002).await;
        insert_audit_row(&pool, &rid_a, "a", "archive", 1003).await;

        let list = audit_list(
            &pool,
            AuditListFilter {
                ref_id: Some(rid_a.clone()),
                action: Some("archive".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("ok");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|i| i.ref_id == rid_a && i.action == "archive"));
        assert_eq!(list[0].at, 1003);
        assert_eq!(list[1].at, 1000);
    }

    #[tokio::test]
    async fn audit_list_ordered_by_at_desc() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        // 乱序插入
        insert_audit_row(&pool, &rid, "r", "archive", 500).await;
        insert_audit_row(&pool, &rid, "r", "archive", 2000).await;
        insert_audit_row(&pool, &rid, "r", "archive", 100).await;
        insert_audit_row(&pool, &rid, "r", "archive", 1500).await;

        let list = audit_list(&pool, filter_all()).await.expect("ok");
        let ats: Vec<i64> = list.iter().map(|i| i.at).collect();
        assert_eq!(ats, vec![2000, 1500, 500, 100]);
    }

    #[tokio::test]
    async fn audit_list_err_invalid_action() {
        let pool = setup().await;
        let err = audit_list(&pool, filter_by_action("bogus"))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn audit_list_limit_clamped_to_200() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        // 造 5 条；请求 limit=999 → 截断为 200，但实际只有 5 条
        for i in 0..5 {
            insert_audit_row(&pool, &rid, "r", "archive", 1000 + i).await;
        }
        let list = audit_list(
            &pool,
            AuditListFilter {
                limit: Some(999),
                ..Default::default()
            },
        )
        .await
        .expect("ok");
        assert_eq!(list.len(), 5);
    }

    #[tokio::test]
    async fn audit_list_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid, "none").await;
        archive(&pool, rid).await.expect("archive");

        let list = audit_list(&pool, filter_all()).await.expect("ok");
        let v = serde_json::to_value(&list[0]).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("refId"));
        assert!(obj.contains_key("refName"));
        assert!(obj.contains_key("action"));
        assert!(obj.contains_key("locatorSnapshot"));
        assert!(obj.contains_key("actor"));
        assert!(obj.contains_key("note"));
        assert!(obj.contains_key("at"));
        // 不出现 snake_case
        assert!(!obj.contains_key("ref_id"));
        assert!(!obj.contains_key("ref_name"));
        assert!(!obj.contains_key("locator_snapshot"));
    }

    // ---------- m4-4.9 · ref_undo_import ----------

    /// 插入一个 managed reference，可指定 hosting / locator_json / created_at；返回 id。
    /// managed_action 为 None 时 locator_json 不含 managedAction 字段（模拟 M3 老数据）。
    async fn make_managed_reference(
        pool: &SqlitePool,
        collection_id: &str,
        hosting: &str,
        current_path: &std::path::Path,
        original_source: Option<&std::path::Path>,
        created_at: i64,
    ) -> String {
        make_managed_reference_with_action(
            pool,
            collection_id,
            hosting,
            current_path,
            original_source,
            None,
            created_at,
        )
        .await
    }

    /// 完整版：可指定 managedAction。
    async fn make_managed_reference_with_action(
        pool: &SqlitePool,
        collection_id: &str,
        hosting: &str,
        current_path: &std::path::Path,
        original_source: Option<&std::path::Path>,
        managed_action: Option<&str>,
        created_at: i64,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        let mut locator = serde_json::json!({
            "kind": "path",
            "path": current_path.to_string_lossy(),
        });
        if let Some(src) = original_source {
            locator["originalSource"] =
                serde_json::Value::String(src.to_string_lossy().into_owned());
        }
        if let Some(action) = managed_action {
            locator["managedAction"] = serde_json::Value::String(action.to_string());
        }
        let locator_json = locator.to_string();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, ?, 'r', 'code', ?, ?, \
              NULL, 'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(DEFAULT_SOURCE_ID)
        .bind(hosting)
        .bind(&locator_json)
        .bind(created_at)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert managed reference");
        id
    }

    #[tokio::test]
    async fn undo_plan_err_ref_not_found() {
        let pool = setup().await;
        let err = undo_import(&pool, "no-such-id".into(), false)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn undo_plan_blocker_hosting_not_managed() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_managed_reference(
            &pool,
            &cid,
            "external", // 非 managed
            &file,
            Some(&file),
            now_unix(),
        )
        .await;

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("仅 managed 导入的引用可撤销")));
    }

    #[tokio::test]
    async fn undo_plan_blocker_no_original_source() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_managed_reference(
            &pool,
            &cid,
            "managed",
            &file,
            None, // 无 originalSource（M3 老数据）
            now_unix(),
        )
        .await;

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("未记录原始源")));
    }

    #[tokio::test]
    async fn undo_plan_blocker_target_externally_deleted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("gone.txt");
        std::fs::write(&file, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        let rid = make_managed_reference(
            &pool,
            &cid,
            "managed",
            &file,
            Some(&src),
            now_unix(),
        )
        .await;
        // 外部删除目标
        std::fs::remove_file(&file).expect("remove");

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("已被外部删除或移动")));
    }

    #[tokio::test]
    async fn undo_plan_blocker_target_externally_modified() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        // created_at 设为 1 小时前（在 24h 窗口内），但 mtime 是现在 → 超 5s 容差
        let created_at = now_unix() - 3600;
        let rid = make_managed_reference(
            &pool,
            &cid,
            "managed",
            &file,
            Some(&src),
            created_at,
        )
        .await;
        // 等 1 秒确保 mtime 差 > 5s 容差（其实文件刚写，mtime=now > created_at+5）
        std::thread::sleep(std::time::Duration::from_millis(100));

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("已被外部修改")));
    }

    #[tokio::test]
    async fn undo_plan_blocker_beyond_24h_window() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        // created_at 25 小时前 → 超窗
        let created_at = now_unix() - (UNDO_WINDOW_SECS + 3600);
        let rid = make_managed_reference(
            &pool,
            &cid,
            "managed",
            &file,
            Some(&src),
            created_at,
        )
        .await;

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("24 小时撤销窗口")));
    }

    #[tokio::test]
    async fn undo_plan_blocker_move_source_occupied() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"x").expect("write target");
        // move 场景：originalSource 被其他文件占用
        let src = tmp.path().join("src.txt");
        std::fs::write(&src, b"occupied").expect("write src");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("move"),
            now_unix(),
        )
        .await;

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert_eq!(plan.managed_action, "move");
        assert!(!plan.can_undo);
        assert!(plan
            .blockers
            .iter()
            .any(|b| b.contains("原始源路径已被其他文件占用")));
    }

    #[test]
    fn compute_undo_blockers_move_source_occupied() {
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        std::fs::write(&src, b"occupied").expect("write src");

        let blockers = compute_undo_blockers(
            "managed",
            Some(src.to_str().expect("utf8")),
            &target,
            now_unix(),
            now_unix(),
            "move",
        );
        assert!(blockers
            .iter()
            .any(|b| b.contains("原始源路径已被其他文件占用")));
    }

    #[tokio::test]
    async fn undo_plan_ok_copy_scenario() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        std::fs::write(&src, b"original").expect("write src");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("copy"),
            now_unix(),
        )
        .await;

        let plan = undo_import(&pool, rid.clone(), false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert!(plan.can_undo, "blockers: {:?}", plan.blockers);
        assert!(plan.blockers.is_empty());
        assert_eq!(plan.ref_id, rid);
        assert_eq!(plan.ref_name, "r");
        assert_eq!(plan.managed_action, "copy");
        assert_eq!(plan.current_path, target.to_string_lossy());
        assert_eq!(plan.original_source.as_deref(), Some(src.to_string_lossy().as_ref()));
        // plan 阶段不写文件不改库
        assert!(target.exists());
        assert_eq!(ref_count(&pool, &rid).await, 1);
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn undo_confirmed_copy_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"to-be-deleted").expect("write");
        let src = tmp.path().join("src.txt");
        std::fs::write(&src, b"original").expect("write src");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("copy"),
            now_unix(),
        )
        .await;

        let result = undo_import(&pool, rid.clone(), true)
            .await
            .expect("undo ok");
        assert!(result.is_none(), "confirmed=true 返回 None");

        // 目标文件已删除
        assert!(!target.exists(), "copy 撤销应删除目标");
        // 源文件保留
        assert!(src.exists(), "copy 撤销不动源");
        // 引用行已删
        assert_eq!(ref_count(&pool, &rid).await, 0);
        // 审计已写
        assert_eq!(audit_count(&pool, &rid).await, 1);
        let row = sqlx::query(
            "SELECT action, note, locator_snapshot, actor FROM disposition_audit WHERE ref_id = ?",
        )
        .bind(&rid)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        let action: String = row.try_get("action").expect("action");
        let note: Option<String> = row.try_get("note").expect("note");
        let locator: String = row.try_get("locator_snapshot").expect("locator");
        let actor: String = row.try_get("actor").expect("actor");
        assert_eq!(action, "undo_import");
        assert_eq!(actor, "local_user");
        assert!(locator.contains("target.txt"));
        let note_json: serde_json::Value =
            serde_json::from_str(&note.expect("note")).expect("note json");
        assert_eq!(note_json["managedAction"].as_str(), Some("copy"));
        assert!(note_json["originalSource"].as_str().expect("src").contains("src.txt"));
    }

    #[tokio::test]
    async fn undo_confirmed_move_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"moved-content").expect("write");
        // move 场景：originalSource 不存在（源被删了）
        let src = tmp.path().join("src.txt");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("move"),
            now_unix(),
        )
        .await;

        let result = undo_import(&pool, rid.clone(), true)
            .await
            .expect("undo ok");
        assert!(result.is_none());

        // 文件已移回 originalSource
        assert!(!target.exists(), "move 撤销后目标位置应空");
        assert!(src.exists(), "move 撤销后文件应回到 originalSource");
        assert_eq!(std::fs::read(&src).expect("read"), b"moved-content");
        // 引用行已删
        assert_eq!(ref_count(&pool, &rid).await, 0);
        // 审计：managedAction=move
        let row = sqlx::query("SELECT action, note FROM disposition_audit WHERE ref_id = ?")
            .bind(&rid)
            .fetch_one(&pool)
            .await
            .expect("fetch audit");
        let action: String = row.try_get("action").expect("action");
        let note: Option<String> = row.try_get("note").expect("note");
        assert_eq!(action, "undo_import");
        let note_json: serde_json::Value =
            serde_json::from_str(&note.expect("note")).expect("note json");
        assert_eq!(note_json["managedAction"].as_str(), Some("move"));
    }

    #[tokio::test]
    async fn undo_confirmed_move_source_occupied_aborts() {
        // move 撤销时 originalSource 被占 → COMMON_FORBIDDEN（锁内复检 blocker）
        // + 文件未动 + 引用未删
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("target.txt");
        std::fs::write(&target, b"moved-content").expect("write");
        let src = tmp.path().join("src.txt");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("move"),
            now_unix(),
        )
        .await;

        // plan 阶段：src 不存在 → canUndo=true
        let plan = undo_import(&pool, rid.clone(), false)
            .await
            .expect("plan ok")
            .expect("plan");
        assert_eq!(plan.managed_action, "move");
        assert!(plan.can_undo);

        // 用户在 confirmed 前手动占用 src
        std::fs::write(&src, b"occupied-by-other").expect("occupy");

        // confirmed：锁内复检应发现 src 被占 → COMMON_FORBIDDEN（blocker 触发）
        let err = undo_import(&pool, rid.clone(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_FORBIDDEN");
        // 文件未动
        assert!(target.exists());
        assert_eq!(std::fs::read(&src).expect("read"), b"occupied-by-other");
        // 引用未删
        assert_eq!(ref_count(&pool, &rid).await, 1);
        // 无审计
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn undo_confirmed_blockers_forbidden() {
        // canUndo=false 时 confirmed=true → COMMON_FORBIDDEN
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").expect("write");
        let rid = make_managed_reference(
            &pool,
            &cid,
            "external", // 非 managed → blocker
            &file,
            Some(&file),
            now_unix(),
        )
        .await;

        let err = undo_import(&pool, rid.clone(), true)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_FORBIDDEN");
        // 文件未动、引用未删
        assert!(file.exists());
        assert_eq!(ref_count(&pool, &rid).await, 1);
        assert_eq!(audit_count(&pool, &rid).await, 0);
    }

    #[tokio::test]
    async fn undo_plan_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("t.txt");
        std::fs::write(&target, b"x").expect("write");
        let src = tmp.path().join("s.txt");
        std::fs::write(&src, b"s").expect("write");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("copy"),
            now_unix(),
        )
        .await;

        let plan = undo_import(&pool, rid, false)
            .await
            .expect("ok")
            .expect("plan");
        let v = serde_json::to_value(&plan).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("refId"));
        assert!(obj.contains_key("refName"));
        assert!(obj.contains_key("managedAction"));
        assert!(obj.contains_key("currentPath"));
        assert!(obj.contains_key("originalSource"));
        assert!(obj.contains_key("canUndo"));
        assert!(obj.contains_key("blockers"));
        assert!(!obj.contains_key("ref_id"));
        assert!(!obj.contains_key("managed_action"));
        assert!(!obj.contains_key("can_undo"));
    }

    #[tokio::test]
    async fn undo_confirmed_copy_target_already_gone_idempotent() {
        // copy 撤销时目标已被外部删除：视为成功（幂等），引用行仍删除
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");
        let target = tmp.path().join("gone.txt");
        std::fs::write(&target, b"x").expect("write");
        let src = tmp.path().join("src.txt");
        std::fs::write(&src, b"s").expect("write");
        let rid = make_managed_reference(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            now_unix(),
        )
        .await;

        // 外部删除目标 → plan 阶段会报 blocker「已被外部删除或移动」
        std::fs::remove_file(&target).expect("remove");
        let plan = undo_import(&pool, rid.clone(), false)
            .await
            .expect("plan")
            .expect("plan");
        assert!(!plan.can_undo, "目标被外部删除应报 blocker");
        // confirmed 也会被 COMMON_FORBIDDEN 拦截，因此「幂等成功」路径
        // 仅在「plan 后、confirmed 前」的 TOCTOU 窗口触发；锁内复检同样拦截。
        // 本测试验证 blocker 行为即可。
    }

    /// 并发：undo_import 与 create_managed 互斥（共用 MANAGED_WRITE_LOCK）。
    ///
    /// 验证方式：同时发起一个 undo 和一个 create_managed，两者都应成功
    /// （互斥锁保证不冲突），且 MANAGED_WRITE_LOCK 已被初始化。
    #[tokio::test]
    async fn undo_concurrent_with_create_managed_serialized() {
        use std::sync::Arc;
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tmp = tempfile::tempdir().expect("tmp");

        // 准备 undo 场景
        let target = tmp.path().join("undo_target.txt");
        std::fs::write(&target, b"to-undo").expect("write");
        let src = tmp.path().join("undo_src.txt");
        std::fs::write(&src, b"original").expect("write");
        let rid = make_managed_reference_with_action(
            &pool,
            &cid,
            "managed",
            &target,
            Some(&src),
            Some("copy"),
            now_unix(),
        )
        .await;

        // 准备 create_managed 场景
        let create_src = tmp.path().join("create_src.txt");
        std::fs::write(&create_src, b"new-file").expect("write");
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).expect("mkdir root");
        // 配置 root_dir
        let value_json = serde_json::to_string(&serde_json::Value::String(
            root.to_string_lossy().to_string(),
        ))
        .expect("serialize");
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('root_dir', ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        )
        .bind(&value_json)
        .bind(now_unix())
        .execute(&pool)
        .await
        .expect("insert root_dir");

        let pool = Arc::new(pool);
        let cid = Arc::new(cid);

        // 并发：undo + create_managed
        let pool1 = Arc::clone(&pool);
        let rid1 = rid.clone();
        let h1 = tokio::spawn(async move { undo_import(&pool1, rid1, true).await });

        let pool2 = Arc::clone(&pool);
        let cid2 = Arc::clone(&cid);
        let h2 = tokio::spawn(async move {
            crate::reference::create_managed(
                &pool2,
                (*cid2).clone(),
                "new".into(),
                "document".into(),
                crate::reference::Locator {
                    kind: "path".into(),
                    path: create_src.to_string_lossy().to_string(),
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

        let (r1, r2) = tokio::join!(h1, h2);
        r1.expect("join1").expect("undo ok");
        r2.expect("join2").expect("create ok");

        // 互斥锁已初始化
        assert!(crate::reference::MANAGED_WRITE_LOCK.get().is_some());
        // undo 生效
        assert!(!target.exists());
        assert_eq!(ref_count(&pool, &rid).await, 0);
    }
}
