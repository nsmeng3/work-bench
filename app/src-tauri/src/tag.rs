//! 标签子系统（详细设计 §2.3~2.5 各命令的 `tags` 入参；M2-2.4）
//!
//! 4 个命令：`tag_add` / `tag_remove` / `tag_list` / `tag_query`。
//!
//! 双 tag 表：`collection_tag(collection_id, tag)` / `reference_tag(reference_id, tag)`，
//! 主键即 (id, tag)，`tag_add` 幂等通过 `INSERT OR IGNORE` 实现。
//! `tag_query` 为 AND 语义：返回同时含全部给定标签的目标 id 列表。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::error::{AppError, CmdResult};

/// 标签最大长度（契约未另设上限，子系统内部约束）。
const TAG_MAX_LEN: usize = 32;

/// 目标对象类型（`tag_add` / `tag_remove` / `tag_list` 的 `targetKind`，
/// 以及 `tag_query` 的 `kind`）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Collection,
    Reference,
}

impl TargetKind {
    fn parse(s: &str) -> CmdResult<Self> {
        match s {
            "collection" => Ok(Self::Collection),
            "reference" => Ok(Self::Reference),
            other => Err(AppError::invalid_param(format!(
                "targetKind 取值非法: {}（应为 collection|reference）",
                other
            ))),
        }
    }

    /// 对应的 tag 表名。
    fn table(self) -> &'static str {
        match self {
            Self::Collection => "collection_tag",
            Self::Reference => "reference_tag",
        }
    }

    /// tag 表中的目标 id 列名。
    fn id_column(self) -> &'static str {
        match self {
            Self::Collection => "collection_id",
            Self::Reference => "reference_id",
        }
    }

    /// 目标主表名（用于存在性校验）。
    fn parent_table(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Reference => "resource_reference",
        }
    }
}

fn validate_tag(tag: &str) -> CmdResult<&str> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_param("标签不能为空"));
    }
    if trimmed.chars().count() > TAG_MAX_LEN {
        return Err(AppError::invalid_param(format!(
            "标签超长（>{} 字符）",
            TAG_MAX_LEN
        )));
    }
    Ok(trimmed)
}

/// 校验目标存在（不存在 → COMMON_NOT_FOUND）。
async fn ensure_target_exists(pool: &SqlitePool, kind: TargetKind, id: &str) -> CmdResult<()> {
    let sql = format!("SELECT id FROM {} WHERE id = ?", kind.parent_table());
    sqlx::query(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            let label = match kind {
                TargetKind::Collection => "资源集",
                TargetKind::Reference => "引用",
            };
            AppError::not_found(format!("{}不存在: {}", label, id))
        })?;
    Ok(())
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// 单标签追加（幂等，重复添加不报错）。
pub async fn add(
    pool: &SqlitePool,
    target_kind: String,
    target_id: String,
    tag: String,
) -> CmdResult<Vec<String>> {
    let kind = TargetKind::parse(&target_kind)?;
    let tag = validate_tag(&tag)?.to_string();
    ensure_target_exists(pool, kind, &target_id).await?;

    let sql = format!(
        "INSERT OR IGNORE INTO {} ({}, tag) VALUES (?, ?)",
        kind.table(),
        kind.id_column()
    );
    sqlx::query(&sql)
        .bind(&target_id)
        .bind(&tag)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

    list(pool, target_kind, target_id).await
}

/// 移除标签（不存在该标签时幂等返回当前列表）。
pub async fn remove(
    pool: &SqlitePool,
    target_kind: String,
    target_id: String,
    tag: String,
) -> CmdResult<Vec<String>> {
    let kind = TargetKind::parse(&target_kind)?;
    let tag = validate_tag(&tag)?.to_string();
    ensure_target_exists(pool, kind, &target_id).await?;

    let sql = format!(
        "DELETE FROM {} WHERE {} = ? AND tag = ?",
        kind.table(),
        kind.id_column()
    );
    sqlx::query(&sql)
        .bind(&target_id)
        .bind(&tag)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

    list(pool, target_kind, target_id).await
}

/// 列出目标的所有标签（按字母序）。
pub async fn list(
    pool: &SqlitePool,
    target_kind: String,
    target_id: String,
) -> CmdResult<Vec<String>> {
    let kind = TargetKind::parse(&target_kind)?;
    ensure_target_exists(pool, kind, &target_id).await?;

    let sql = format!(
        "SELECT tag FROM {} WHERE {} = ? ORDER BY tag ASC",
        kind.table(),
        kind.id_column()
    );
    let rows = sqlx::query(&sql)
        .bind(&target_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
    Ok(rows.iter().map(|r| r.get::<String, _>("tag")).collect())
}

/// 按标签筛选：返回同时含全部给定标签的目标 id 列表（AND 语义）。
///
/// `tags` 为空时返回空列表（语义上等价于"无过滤条件"，但本命令用于筛选，
/// 返回空避免误导为"全量"）。
pub async fn query(pool: &SqlitePool, kind: String, tags: Vec<String>) -> CmdResult<Vec<String>> {
    let kind = TargetKind::parse(&kind)?;
    if tags.is_empty() {
        return Ok(Vec::new());
    }
    let mut normalized: Vec<String> = Vec::with_capacity(tags.len());
    for t in &tags {
        normalized.push(validate_tag(t)?.to_string());
    }
    // 去重，避免同一标签重复参与 GROUP BY ... HAVING COUNT
    normalized.sort();
    normalized.dedup();

    // AND 语义：GROUP BY id HAVING COUNT(DISTINCT tag) = 标签数
    let placeholders = std::iter::repeat("?")
        .take(normalized.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {id_col} AS target_id FROM {table} \
         WHERE tag IN ({placeholders}) \
         GROUP BY {id_col} \
         HAVING COUNT(DISTINCT tag) = ? \
         ORDER BY {id_col} ASC",
        id_col = kind.id_column(),
        table = kind.table(),
        placeholders = placeholders
    );
    let mut q = sqlx::query(&sql);
    for t in &normalized {
        q = q.bind(t);
    }
    q = q.bind(normalized.len() as i64);
    let rows = q.fetch_all(pool).await.map_err(AppError::from)?;
    Ok(rows
        .iter()
        .map(|r| r.get::<String, _>("target_id"))
        .collect())
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn tag_add(
    state: tauri::State<'_, crate::AppState>,
    target_kind: String,
    target_id: String,
    tag: String,
) -> CmdResult<Vec<String>> {
    add(&state.pool, target_kind, target_id, tag).await
}

#[tauri::command]
pub async fn tag_remove(
    state: tauri::State<'_, crate::AppState>,
    target_kind: String,
    target_id: String,
    tag: String,
) -> CmdResult<Vec<String>> {
    remove(&state.pool, target_kind, target_id, tag).await
}

#[tauri::command]
pub async fn tag_list(
    state: tauri::State<'_, crate::AppState>,
    target_kind: String,
    target_id: String,
) -> CmdResult<Vec<String>> {
    list(&state.pool, target_kind, target_id).await
}

#[tauri::command]
pub async fn tag_query(
    state: tauri::State<'_, crate::AppState>,
    kind: String,
    tags: Vec<String>,
) -> CmdResult<Vec<String>> {
    query(&state.pool, kind, tags).await
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

    async fn setup() -> SqlitePool {
        init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";

    fn now_unix() -> i64 {
        OffsetDateTime::now_utc().unix_timestamp()
    }

    /// 在预置空间下创建一个资源集，返回其 id。
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

    /// 在指定资源集下创建一条引用，返回其 id。
    async fn make_reference(pool: &SqlitePool, collection_id: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, 'src_local_fs_default', 'r', 'code', 'external', '{}', \
                     'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(&id)
        .bind(collection_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert reference");
        id
    }

    // ---------- tag_add ----------

    #[tokio::test]
    async fn tag_add_ok_collection() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tags = add(&pool, "collection".into(), cid.clone(), "重要".into())
            .await
            .expect("add ok");
        assert_eq!(tags, vec!["重要".to_string()]);
    }

    #[tokio::test]
    async fn tag_add_ok_reference() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let rid = make_reference(&pool, &cid).await;
        let tags = add(&pool, "reference".into(), rid.clone(), "代码".into())
            .await
            .expect("add ok");
        assert_eq!(tags, vec!["代码".to_string()]);
    }

    #[tokio::test]
    async fn tag_add_idempotent() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        add(&pool, "collection".into(), cid.clone(), "t1".into())
            .await
            .expect("first add");
        let tags = add(&pool, "collection".into(), cid.clone(), "t1".into())
            .await
            .expect("second add (idempotent)");
        assert_eq!(tags, vec!["t1".to_string()], "重复添加不报错且不重复");
    }

    #[tokio::test]
    async fn tag_add_err_target_not_found() {
        let pool = setup().await;
        let err = add(&pool, "collection".into(), "no-such-id".into(), "t".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn tag_add_err_empty_tag() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let err = add(&pool, "collection".into(), cid, "   ".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn tag_add_err_too_long_tag() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let long = "x".repeat(TAG_MAX_LEN + 1);
        let err = add(&pool, "collection".into(), cid, long)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn tag_add_err_invalid_kind() {
        let pool = setup().await;
        let err = add(&pool, "bogus".into(), "x".into(), "t".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- tag_remove ----------

    #[tokio::test]
    async fn tag_remove_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        add(&pool, "collection".into(), cid.clone(), "t1".into())
            .await
            .expect("add t1");
        add(&pool, "collection".into(), cid.clone(), "t2".into())
            .await
            .expect("add t2");
        let tags = remove(&pool, "collection".into(), cid.clone(), "t1".into())
            .await
            .expect("remove ok");
        assert_eq!(tags, vec!["t2".to_string()]);
    }

    #[tokio::test]
    async fn tag_remove_idempotent_when_missing() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let tags = remove(&pool, "collection".into(), cid.clone(), "never-added".into())
            .await
            .expect("remove missing tag ok");
        assert!(tags.is_empty());
    }

    #[tokio::test]
    async fn tag_remove_err_target_not_found() {
        let pool = setup().await;
        let err = remove(&pool, "reference".into(), "no-such-id".into(), "t".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- tag_list ----------

    #[tokio::test]
    async fn tag_list_ok_sorted() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        add(&pool, "collection".into(), cid.clone(), "b".into())
            .await
            .expect("add b");
        add(&pool, "collection".into(), cid.clone(), "a".into())
            .await
            .expect("add a");
        add(&pool, "collection".into(), cid.clone(), "c".into())
            .await
            .expect("add c");
        let tags = list(&pool, "collection".into(), cid.clone())
            .await
            .expect("list ok");
        assert_eq!(tags, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[tokio::test]
    async fn tag_list_err_target_not_found() {
        let pool = setup().await;
        let err = list(&pool, "collection".into(), "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- tag_query ----------

    #[tokio::test]
    async fn tag_query_collection_and_semantics() {
        let pool = setup().await;
        let c1 = make_collection(&pool).await;
        let c2 = make_collection(&pool).await;
        let c3 = make_collection(&pool).await;

        // c1: t1, t2
        add(&pool, "collection".into(), c1.clone(), "t1".into())
            .await
            .expect("c1 t1");
        add(&pool, "collection".into(), c1.clone(), "t2".into())
            .await
            .expect("c1 t2");
        // c2: t1 only
        add(&pool, "collection".into(), c2.clone(), "t1".into())
            .await
            .expect("c2 t1");
        // c3: t2 only
        add(&pool, "collection".into(), c3.clone(), "t2".into())
            .await
            .expect("c3 t2");

        let ids = query(
            &pool,
            "collection".into(),
            vec!["t1".into(), "t2".into()],
        )
        .await
        .expect("query ok");
        assert_eq!(ids, vec![c1.clone()], "AND 语义：仅 c1 同时含 t1 与 t2");

        let ids = query(&pool, "collection".into(), vec!["t1".into()])
            .await
            .expect("single tag query");
        assert!(ids.contains(&c1));
        assert!(ids.contains(&c2));
        assert!(!ids.contains(&c3));
    }

    #[tokio::test]
    async fn tag_query_reference_and_semantics() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let r1 = make_reference(&pool, &cid).await;
        let r2 = make_reference(&pool, &cid).await;
        add(&pool, "reference".into(), r1.clone(), "x".into())
            .await
            .expect("r1 x");
        add(&pool, "reference".into(), r1.clone(), "y".into())
            .await
            .expect("r1 y");
        add(&pool, "reference".into(), r2.clone(), "x".into())
            .await
            .expect("r2 x");

        let ids = query(&pool, "reference".into(), vec!["x".into(), "y".into()])
            .await
            .expect("query ok");
        assert_eq!(ids, vec![r1.clone()]);
    }

    #[tokio::test]
    async fn tag_query_empty_result() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        add(&pool, "collection".into(), cid.clone(), "t1".into())
            .await
            .expect("add t1");
        let ids = query(&pool, "collection".into(), vec!["no-such-tag".into()])
            .await
            .expect("query ok");
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn tag_query_empty_tags_returns_empty() {
        let pool = setup().await;
        let ids = query(&pool, "collection".into(), vec![])
            .await
            .expect("query ok");
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn tag_query_err_invalid_tag() {
        let pool = setup().await;
        let err = query(&pool, "collection".into(), vec!["".into()])
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn tag_query_err_invalid_kind() {
        let pool = setup().await;
        let err = query(&pool, "bogus".into(), vec!["t".into()])
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- 序列化契约 ----------

    #[tokio::test]
    async fn target_kind_serializes_camel_case() {
        assert_eq!(
            serde_json::to_string(&TargetKind::Collection).expect("ser"),
            "\"collection\""
        );
        assert_eq!(
            serde_json::to_string(&TargetKind::Reference).expect("ser"),
            "\"reference\""
        );
    }
}
