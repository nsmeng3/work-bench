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
}
