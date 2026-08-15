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
}
