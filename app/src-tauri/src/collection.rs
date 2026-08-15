//! 资源集管理（详细设计 §2.4）
//!
//! 6 个命令：`collection_create` / `collection_update` / `collection_archive` /
//! `collection_restore` / `collection_get` / `collection_list`。
//!
//! 归档为逻辑归档（改 status 字段），不触碰其下引用（§6.2）。
//! tags 通过 `collection_tag` 关联表存储（§3.2）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};

/// 资源集名称最大长度（与 space.name 对齐，契约未另设上限）。
const NAME_MAX_LEN: usize = 64;

/// 引用健康度（§2.4 collection_get.referencesByType.*.health）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefHealth {
    Ok,
    Missing,
    Unknown,
}

/// 资源集（与 `collection` 表一一对应，附带 tags 列表）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub space_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub status: String,
    pub tags: Vec<String>,
    /// Unix 秒
    pub created_at: i64,
    /// Unix 秒
    pub updated_at: i64,
}

/// 资源引用（§2.5 Reference，本阶段仅用于 `collection_get` 分组返回）。
///
/// 字段与 `resource_reference` 表一一对应；`locator_json` 透传为 JSON 值。
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
    pub created_at: i64,
    pub updated_at: i64,
}

/// `referencesByType` 中每个引用条目的包装：`{ ref, health }`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceWithHealth {
    #[serde(rename = "ref")]
    pub reference: Reference,
    pub health: RefHealth,
}

/// 六类型分组结构（§2.4）。
///
/// 使用 `BTreeMap` 而非 struct，便于按契约输出固定六键且可扩展；
/// 序列化后键为 `code` / `document` / `data` / `artifact` / `tool` / `media`。
pub type ReferencesByType = BTreeMap<String, Vec<ReferenceWithHealth>>;

/// `collection_get` 出参：`Collection` + `referencesByType`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    #[serde(flatten)]
    pub collection: Collection,
    pub references_by_type: ReferencesByType,
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn validate_name(name: &str) -> CmdResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_param("资源集名称不能为空"));
    }
    if trimmed.chars().count() > NAME_MAX_LEN {
        return Err(AppError::invalid_param(format!(
            "资源集名称超长（>{} 字符）",
            NAME_MAX_LEN
        )));
    }
    Ok(())
}

fn row_to_collection(row: &sqlx::sqlite::SqliteRow, tags: Vec<String>) -> Result<Collection, sqlx::Error> {
    Ok(Collection {
        id: row.try_get("id")?,
        space_id: row.try_get("space_id")?,
        name: row.try_get("name")?,
        summary: row.try_get("summary")?,
        status: row.try_get("status")?,
        tags,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn fetch_tags(pool: &SqlitePool, collection_id: &str) -> CmdResult<Vec<String>> {
    let rows = sqlx::query(
        "SELECT tag FROM collection_tag WHERE collection_id = ? ORDER BY tag ASC",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    Ok(rows.iter().map(|r| r.get::<String, _>("tag")).collect())
}

async fn replace_tags(pool: &SqlitePool, collection_id: &str, tags: &[String]) -> CmdResult<()> {
    sqlx::query("DELETE FROM collection_tag WHERE collection_id = ?")
        .bind(collection_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT OR IGNORE INTO collection_tag (collection_id, tag) VALUES (?, ?)",
        )
        .bind(collection_id)
        .bind(trimmed)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    }
    Ok(())
}

async fn fetch_collection(pool: &SqlitePool, id: &str) -> CmdResult<Collection> {
    let row = sqlx::query(
        "SELECT id, space_id, name, summary, status, created_at, updated_at \
         FROM collection WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("资源集不存在: {}", id)))?;
    let tags = fetch_tags(pool, id).await?;
    row_to_collection(&row, tags).map_err(AppError::from)
}

/// 校验空间存在且未归档（契约：`collection_create` 在空间不存在或已归档时返回 COMMON_NOT_FOUND）。
async fn ensure_space_active(pool: &SqlitePool, space_id: &str) -> CmdResult<()> {
    let row = sqlx::query("SELECT status FROM space WHERE id = ?")
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("空间不存在: {}", space_id)))?;
    let status: String = row.try_get("status").map_err(AppError::from)?;
    if status != "active" {
        return Err(AppError::not_found(format!(
            "空间不存在或已归档: {}",
            space_id
        )));
    }
    Ok(())
}

fn row_to_reference(row: &sqlx::sqlite::SqliteRow) -> Result<Reference, sqlx::Error> {
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
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// 六类型固定键（§2.4 referencesByType）。
const REF_TYPES: [&str; 6] = ["code", "document", "data", "artifact", "tool", "media"];

/// 按类型分组返回某资源集下所有引用。
///
/// 本阶段健康度检测未实现，统一返回 `unknown`（§2.4 允许值之一）。
async fn fetch_references_by_type(
    pool: &SqlitePool,
    collection_id: &str,
) -> CmdResult<ReferencesByType> {
    let mut grouped: ReferencesByType = BTreeMap::new();
    for t in REF_TYPES {
        grouped.insert(t.to_string(), Vec::new());
    }

    let rows = sqlx::query(
        "SELECT id, collection_id, source_id, name, type, hosting, locator_json, \
                description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at \
         FROM resource_reference \
         WHERE collection_id = ? AND disposition != 'deleted' \
         ORDER BY created_at ASC, id ASC",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    for row in &rows {
        let reference = row_to_reference(row).map_err(AppError::from)?;
        let key = reference.ref_type.clone();
        grouped
            .entry(key)
            .or_insert_with(Vec::new)
            .push(ReferenceWithHealth {
                reference,
                health: RefHealth::Unknown,
            });
    }
    Ok(grouped)
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

pub async fn create(
    pool: &SqlitePool,
    space_id: String,
    name: String,
    summary: Option<String>,
    tags: Option<Vec<String>>,
) -> CmdResult<Collection> {
    validate_name(&name)?;
    ensure_space_active(pool, &space_id).await?;

    let id = Uuid::new_v4().to_string();
    let now = now_unix();
    sqlx::query(
        "INSERT INTO collection (id, space_id, name, summary, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(&id)
    .bind(&space_id)
    .bind(name.trim())
    .bind(&summary)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    if let Some(tags) = tags {
        replace_tags(pool, &id, &tags).await?;
    }
    fetch_collection(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: String,
    name: Option<String>,
    summary: Option<String>,
    tags: Option<Vec<String>>,
) -> CmdResult<Collection> {
    let existing = fetch_collection(pool, &id).await?;
    if existing.status == "archived" {
        return Err(AppError::conflict(format!(
            "已归档资源集不可编辑: {}",
            id
        )));
    }

    if let Some(ref n) = name {
        validate_name(n)?;
    }

    let new_name = name.as_deref().map(str::trim).unwrap_or(&existing.name);
    let new_summary = summary.or(existing.summary);
    let now = now_unix();

    sqlx::query(
        "UPDATE collection SET name = ?, summary = ?, updated_at = ? WHERE id = ?",
    )
    .bind(new_name)
    .bind(&new_summary)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    if let Some(tags) = tags {
        replace_tags(pool, &id, &tags).await?;
    }
    fetch_collection(pool, &id).await
}

pub async fn archive(pool: &SqlitePool, id: String) -> CmdResult<Collection> {
    let existing = fetch_collection(pool, &id).await?;
    if existing.status == "archived" {
        return Err(AppError::conflict(format!("资源集已归档: {}", id)));
    }
    let now = now_unix();
    sqlx::query("UPDATE collection SET status = 'archived', updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    fetch_collection(pool, &id).await
}

pub async fn restore(pool: &SqlitePool, id: String) -> CmdResult<Collection> {
    // 未找到 → COMMON_NOT_FOUND；已 active 时幂等返回（与 space_restore 行为对齐）
    let existing = fetch_collection(pool, &id).await?;
    if existing.status == "active" {
        return Ok(existing);
    }
    let now = now_unix();
    sqlx::query("UPDATE collection SET status = 'active', updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    fetch_collection(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: String) -> CmdResult<CollectionDetail> {
    let collection = fetch_collection(pool, &id).await?;
    let references_by_type = fetch_references_by_type(pool, &id).await?;
    Ok(CollectionDetail {
        collection,
        references_by_type,
    })
}

/// 校验空间存在（不校验归档状态；契约：`collection_list` 仅要求空间存在）。
async fn ensure_space_exists(pool: &SqlitePool, space_id: &str) -> CmdResult<()> {
    sqlx::query("SELECT id FROM space WHERE id = ?")
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("空间不存在: {}", space_id)))?;
    Ok(())
}

pub async fn list(
    pool: &SqlitePool,
    space_id: String,
    status: Option<String>,
) -> CmdResult<Vec<Collection>> {
    ensure_space_exists(pool, &space_id).await?;

    let status = status.unwrap_or_else(|| "active".to_string());
    let rows = match status.as_str() {
        "active" | "archived" => sqlx::query(
            "SELECT id, space_id, name, summary, status, created_at, updated_at \
             FROM collection WHERE space_id = ? AND status = ? \
             ORDER BY created_at ASC, id ASC",
        )
        .bind(&space_id)
        .bind(&status)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?,
        "all" => sqlx::query(
            "SELECT id, space_id, name, summary, status, created_at, updated_at \
             FROM collection WHERE space_id = ? \
             ORDER BY created_at ASC, id ASC",
        )
        .bind(&space_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?,
        other => {
            return Err(AppError::invalid_param(format!(
                "status 取值非法: {}（应为 active|archived|all）",
                other
            )))
        }
    };

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("id").map_err(AppError::from)?;
        let tags = fetch_tags(pool, &id).await?;
        out.push(row_to_collection(row, tags).map_err(AppError::from)?);
    }
    Ok(out)
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn collection_create(
    state: tauri::State<'_, crate::AppState>,
    space_id: String,
    name: String,
    summary: Option<String>,
    tags: Option<Vec<String>>,
) -> CmdResult<Collection> {
    create(&state.pool, space_id, name, summary, tags).await
}

#[tauri::command]
pub async fn collection_update(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    summary: Option<String>,
    tags: Option<Vec<String>>,
) -> CmdResult<Collection> {
    update(&state.pool, id, name, summary, tags).await
}

#[tauri::command]
pub async fn collection_archive(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<Collection> {
    archive(&state.pool, id).await
}

#[tauri::command]
pub async fn collection_restore(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<Collection> {
    restore(&state.pool, id).await
}

#[tauri::command]
pub async fn collection_get(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<CollectionDetail> {
    get(&state.pool, id).await
}

#[tauri::command]
pub async fn collection_list(
    state: tauri::State<'_, crate::AppState>,
    space_id: String,
    status: Option<String>,
) -> CmdResult<Vec<Collection>> {
    list(&state.pool, space_id, status).await
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

    // ---------- collection_create ----------

    #[tokio::test]
    async fn collection_create_ok_minimal() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "论文".into(), None, None)
            .await
            .expect("create ok");
        assert_eq!(c.name, "论文");
        assert_eq!(c.space_id, PRESET_SPACE);
        assert_eq!(c.status, "active");
        assert!(c.tags.is_empty());
        assert!(!c.id.is_empty());
        assert!(c.created_at > 0);
        assert_eq!(c.created_at, c.updated_at);
    }

    #[tokio::test]
    async fn collection_create_ok_full_fields() {
        let pool = setup().await;
        let c = create(
            &pool,
            PRESET_SPACE.into(),
            "项目A".into(),
            Some("项目A 资料".into()),
            Some(vec!["重要".into(), "客户X".into()]),
        )
        .await
        .expect("create ok");
        assert_eq!(c.summary.as_deref(), Some("项目A 资料"));
        assert_eq!(c.tags, vec!["客户X".to_string(), "重要".to_string()]);
    }

    #[tokio::test]
    async fn collection_create_err_space_not_found() {
        let pool = setup().await;
        let err = create(&pool, "no-such-space".into(), "x".into(), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_create_err_space_archived() {
        let pool = setup().await;
        // 归档预置空间
        sqlx::query("UPDATE space SET status='archived' WHERE id = ?")
            .bind(PRESET_SPACE)
            .execute(&pool)
            .await
            .expect("archive space");
        let err = create(&pool, PRESET_SPACE.into(), "x".into(), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND", "归档空间按契约视为 NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_create_err_empty_name() {
        let pool = setup().await;
        let err = create(&pool, PRESET_SPACE.into(), "   ".into(), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn collection_create_err_too_long_name() {
        let pool = setup().await;
        let long = "x".repeat(NAME_MAX_LEN + 1);
        let err = create(&pool, PRESET_SPACE.into(), long, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- collection_update ----------

    #[tokio::test]
    async fn collection_update_ok_partial() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "原名".into(), None, None)
            .await
            .expect("create ok");
        let updated = update(
            &pool,
            c.id.clone(),
            Some("新名".into()),
            Some("新简介".into()),
            None,
        )
        .await
        .expect("update ok");
        assert_eq!(updated.name, "新名");
        assert_eq!(updated.summary.as_deref(), Some("新简介"));
        assert!(updated.updated_at >= updated.created_at);
    }

    #[tokio::test]
    async fn collection_update_ok_replace_tags() {
        let pool = setup().await;
        let c = create(
            &pool,
            PRESET_SPACE.into(),
            "a".into(),
            None,
            Some(vec!["t1".into(), "t2".into()]),
        )
        .await
        .expect("create ok");
        let updated = update(
            &pool,
            c.id.clone(),
            None,
            None,
            Some(vec!["t3".into()]),
        )
        .await
        .expect("update ok");
        assert_eq!(updated.tags, vec!["t3".to_string()]);
    }

    #[tokio::test]
    async fn collection_update_err_not_found() {
        let pool = setup().await;
        let err = update(&pool, "no-such-id".into(), Some("x".into()), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_update_err_archived_conflict() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "a".into(), None, None)
            .await
            .expect("create ok");
        archive(&pool, c.id.clone()).await.expect("archive ok");
        let err = update(&pool, c.id.clone(), Some("b".into()), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    #[tokio::test]
    async fn collection_update_err_invalid_name() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "a".into(), None, None)
            .await
            .expect("create ok");
        let err = update(&pool, c.id.clone(), Some("".into()), None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- collection_archive ----------

    #[tokio::test]
    async fn collection_archive_ok() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "a".into(), None, None)
            .await
            .expect("create ok");
        let archived = archive(&pool, c.id.clone()).await.expect("archive ok");
        assert_eq!(archived.status, "archived");
    }

    #[tokio::test]
    async fn collection_archive_err_not_found() {
        let pool = setup().await;
        let err = archive(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_archive_err_already_archived() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "a".into(), None, None)
            .await
            .expect("create ok");
        archive(&pool, c.id.clone()).await.expect("archive ok");
        let err = archive(&pool, c.id.clone()).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    // ---------- collection_restore ----------

    #[tokio::test]
    async fn collection_restore_ok() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "a".into(), None, None)
            .await
            .expect("create ok");
        archive(&pool, c.id.clone()).await.expect("archive ok");
        let restored = restore(&pool, c.id.clone()).await.expect("restore ok");
        assert_eq!(restored.status, "active");
    }

    #[tokio::test]
    async fn collection_restore_err_not_found() {
        let pool = setup().await;
        let err = restore(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- collection_get ----------

    #[tokio::test]
    async fn collection_get_ok_empty_references_six_groups() {
        let pool = setup().await;
        let c = create(
            &pool,
            PRESET_SPACE.into(),
            "空集".into(),
            Some("无引用".into()),
            Some(vec!["tagA".into()]),
        )
        .await
        .expect("create ok");

        let detail = get(&pool, c.id.clone()).await.expect("get ok");
        assert_eq!(detail.collection.id, c.id);
        assert_eq!(detail.collection.name, "空集");
        assert_eq!(detail.collection.tags, vec!["tagA".to_string()]);

        // 六类型分组全部存在且为空
        let keys: Vec<&str> = detail
            .references_by_type
            .keys()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            keys,
            vec!["artifact", "code", "data", "document", "media", "tool"],
            "BTreeMap 按键字典序输出"
        );
        for t in REF_TYPES {
            let v = detail
                .references_by_type
                .get(t)
                .unwrap_or_else(|| panic!("missing group {}", t));
            assert!(v.is_empty(), "group {} should be empty", t);
        }

        // 序列化验证：JSON 顶层键符合契约（camelCase + 六类型分组键）
        let json = serde_json::to_value(&detail).expect("serialize");
        let obj = json.as_object().expect("top-level object");
        assert!(obj.contains_key("referencesByType"));
        assert!(obj.contains_key("spaceId"));
        assert!(obj.contains_key("createdAt"));
        let groups = obj["referencesByType"].as_object().expect("groups object");
        for t in REF_TYPES {
            assert!(groups.contains_key(t), "missing key {}", t);
            assert!(groups[t].as_array().expect("array").is_empty());
        }
    }

    #[tokio::test]
    async fn collection_get_err_not_found() {
        let pool = setup().await;
        let err = get(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_get_ok_groups_references_by_type() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "集".into(), None, None)
            .await
            .expect("create ok");

        // 手工插入两条不同类型的引用（本阶段引用服务未实现，直接 SQL）
        let now = now_unix();
        for (id, ty) in [("r1", "code"), ("r2", "document")] {
            sqlx::query(
                "INSERT INTO resource_reference \
                 (id, collection_id, source_id, name, type, hosting, locator_json, \
                  description, lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
                 VALUES (?, ?, 'src_local_fs_default', ?, ?, 'external', '{}', NULL, \
                         'active', 'internal', 1, 'none', ?, ?)",
            )
            .bind(id)
            .bind(&c.id)
            .bind(format!("ref-{}", id))
            .bind(ty)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .expect("insert ref");
        }

        let detail = get(&pool, c.id.clone()).await.expect("get ok");
        assert_eq!(detail.references_by_type["code"].len(), 1);
        assert_eq!(detail.references_by_type["document"].len(), 1);
        assert!(detail.references_by_type["data"].is_empty());
        assert!(detail.references_by_type["artifact"].is_empty());
        assert!(detail.references_by_type["tool"].is_empty());
        assert!(detail.references_by_type["media"].is_empty());

        // 健康度占位为 unknown
        let entry = &detail.references_by_type["code"][0];
        assert_eq!(entry.health, RefHealth::Unknown);
        assert_eq!(entry.reference.id, "r1");
        assert_eq!(entry.reference.ref_type, "code");
    }

    #[tokio::test]
    async fn collection_archive_does_not_touch_references() {
        // §6.2：归档为逻辑归档，不影响已关联引用
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "集".into(), None, None)
            .await
            .expect("create ok");
        let now = now_unix();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES ('r1', ?, 'src_local_fs_default', 'r', 'code', 'external', '{}', \
                     'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(&c.id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert ref");

        archive(&pool, c.id.clone()).await.expect("archive ok");

        let detail = get(&pool, c.id.clone()).await.expect("get ok");
        assert_eq!(detail.collection.status, "archived");
        assert_eq!(detail.references_by_type["code"].len(), 1);
    }

    // ---------- collection_list ----------

    #[tokio::test]
    async fn collection_list_default_active() {
        let pool = setup().await;
        let a = create(&pool, PRESET_SPACE.into(), "A".into(), None, None)
            .await
            .expect("create a");
        let b = create(&pool, PRESET_SPACE.into(), "B".into(), None, None)
            .await
            .expect("create b");
        archive(&pool, b.id.clone()).await.expect("archive b");

        let items = list(&pool, PRESET_SPACE.into(), None)
            .await
            .expect("list ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, a.id);
        assert_eq!(items[0].status, "active");
    }

    #[tokio::test]
    async fn collection_list_archived() {
        let pool = setup().await;
        let a = create(&pool, PRESET_SPACE.into(), "A".into(), None, None)
            .await
            .expect("create a");
        let b = create(&pool, PRESET_SPACE.into(), "B".into(), None, None)
            .await
            .expect("create b");
        archive(&pool, b.id.clone()).await.expect("archive b");

        let items = list(&pool, PRESET_SPACE.into(), Some("archived".into()))
            .await
            .expect("list ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, b.id);
        assert_eq!(items[0].status, "archived");
        // 确认未误返回 active 项
        assert!(items.iter().all(|x| x.id != a.id));
    }

    #[tokio::test]
    async fn collection_list_all() {
        let pool = setup().await;
        let a = create(&pool, PRESET_SPACE.into(), "A".into(), None, None)
            .await
            .expect("create a");
        let b = create(&pool, PRESET_SPACE.into(), "B".into(), None, None)
            .await
            .expect("create b");
        archive(&pool, b.id.clone()).await.expect("archive b");

        let items = list(&pool, PRESET_SPACE.into(), Some("all".into()))
            .await
            .expect("list ok");
        assert_eq!(items.len(), 2);
        let ids: Vec<&str> = items.iter().map(|x| x.id.as_str()).collect();
        assert!(ids.contains(&a.id.as_str()));
        assert!(ids.contains(&b.id.as_str()));
    }

    #[tokio::test]
    async fn collection_list_scoped_by_space() {
        let pool = setup().await;
        // 预置空间来自 0002 迁移；此处再建一个空间用于隔离校验
        sqlx::query(
            "INSERT INTO space (id, name, status, created_at, updated_at) \
             VALUES ('space_other', '其他空间', 'active', 0, 0)",
        )
        .execute(&pool)
        .await
        .expect("insert other space");
        create(&pool, PRESET_SPACE.into(), "A".into(), None, None)
            .await
            .expect("create in work");
        create(&pool, "space_other".into(), "X".into(), None, None)
            .await
            .expect("create in other");

        let items = list(&pool, PRESET_SPACE.into(), None)
            .await
            .expect("list ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "A");
        assert_eq!(items[0].space_id, PRESET_SPACE);
    }

    #[tokio::test]
    async fn collection_list_includes_tags() {
        let pool = setup().await;
        create(
            &pool,
            PRESET_SPACE.into(),
            "带标签".into(),
            None,
            Some(vec!["t1".into(), "t2".into()]),
        )
        .await
        .expect("create ok");

        let items = list(&pool, PRESET_SPACE.into(), None)
            .await
            .expect("list ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tags, vec!["t1".to_string(), "t2".to_string()]);
    }

    #[tokio::test]
    async fn collection_list_err_space_not_found() {
        let pool = setup().await;
        let err = list(&pool, "no-such-space".into(), None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn collection_list_err_invalid_status() {
        let pool = setup().await;
        let err = list(&pool, PRESET_SPACE.into(), Some("bogus".into()))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- 序列化契约 ----------

    #[tokio::test]
    async fn collection_serializes_camel_case() {
        let pool = setup().await;
        let c = create(&pool, PRESET_SPACE.into(), "集".into(), None, None)
            .await
            .expect("create ok");
        let json = serde_json::to_value(&c).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("spaceId"));
        assert!(obj.contains_key("createdAt"));
        assert!(obj.contains_key("updatedAt"));
        assert!(!obj.contains_key("space_id"));
    }

    #[tokio::test]
    async fn ref_health_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&RefHealth::Unknown).expect("ser"),
            "\"unknown\""
        );
        assert_eq!(
            serde_json::to_string(&RefHealth::Ok).expect("ser"),
            "\"ok\""
        );
        assert_eq!(
            serde_json::to_string(&RefHealth::Missing).expect("ser"),
            "\"missing\""
        );
    }
}
