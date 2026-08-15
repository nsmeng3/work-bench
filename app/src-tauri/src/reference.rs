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
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};

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
        assert_eq!(all[0].name, "r1");
        assert_eq!(all[1].name, "r2");
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
}
