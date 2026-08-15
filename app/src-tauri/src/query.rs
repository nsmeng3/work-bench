//! 查询筛选服务（详细设计 §2.9）
//!
//! 本任务（m2-2.5）实现两个命令：
//! - `query_refs`：跨资源集/空间的元数据筛选，返回 `{ total, items }`。
//! - `query_facets`：返回 type/lifecycle/confidentiality/tags 四个维度的
//!   可用筛选值与计数，用于侧边栏。
//!
//! 设计要点：
//! - 第一阶段仅元数据筛选，不做内容检索。
//! - `spaceId` 过滤通过 `collection.space_id` JOIN 实现。
//! - `tags` 为 AND 语义：使用 `EXISTS` 子查询逐个标签约束。
//! - `keyword` 匹配 `name`/`description` 的 LIKE 模糊，转义 `%`/`_`/`\`。
//! - 排序固定 `created_at DESC, id ASC`，保证结果确定性。
//! - `total` 为不受分页影响的计数；`limit` 默认 50、上限 200。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::error::{AppError, CmdResult};
use crate::reference::Reference;

/// `query_refs` 默认分页大小。
const DEFAULT_LIMIT: i64 = 50;
/// `query_refs` 分页上限。
const MAX_LIMIT: i64 = 200;

/// 合法枚举值（与 reference.rs 对齐；契约 §2.9 未另设集合）。
const REF_TYPES: [&str; 6] = ["code", "document", "data", "artifact", "tool", "media"];
const LIFECYCLES: [&str; 4] = ["active", "staged", "delivered", "archived"];
const CONFIDENTIALITIES: [&str; 4] = ["public", "internal", "customer_restricted", "sensitive"];
const DISPOSITIONS: [&str; 3] = ["none", "archived", "deleted"];

/// `query_refs` 出参（§2.9）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryRefsResult {
    pub total: i64,
    pub items: Vec<Reference>,
}

/// `query_facets` 单维度桶。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FacetBucket {
    pub value: String,
    pub count: i64,
}

/// `query_facets` 出参：四个维度的可用筛选值与计数。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueryFacetsResult {
    pub types: Vec<FacetBucket>,
    pub lifecycles: Vec<FacetBucket>,
    pub confidentialities: Vec<FacetBucket>,
    pub tags: Vec<FacetBucket>,
}

/// `query_refs` 入参（内部表示，全部可选）。
#[derive(Debug, Clone, Default)]
pub struct QueryRefsFilter {
    pub space_id: Option<String>,
    pub collection_id: Option<String>,
    pub ref_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub lifecycle: Option<String>,
    pub confidentiality: Option<String>,
    pub disposition: Option<String>,
    pub source_id: Option<String>,
    pub keyword: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn validate_enum(value: &str, allowed: &[&str], field: &str) -> CmdResult<()> {
    if !allowed.contains(&value) {
        return Err(AppError::invalid_param(format!(
            "非法{}: {}（允许值: {}）",
            field,
            value,
            allowed.join("/")
        )));
    }
    Ok(())
}

/// LIKE 转义：`\` → `\\`，`%` → `\%`，`_` → `\_`。
/// 配合 `ESCAPE '\'` 使用，确保用户输入的 `%`/`_` 不被当作通配符。
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            _ => out.push(ch),
        }
    }
    out
}

/// 动态拼接 WHERE 子句与对应 bind 参数。
///
/// 返回 `(where_sql, binds)`，`where_sql` 不含前导 `WHERE`/`AND`，
/// 由调用方决定是否加 `WHERE`（空时省略）。
///
/// 所有用户输入一律走 bind 参数，无 SQL 注入风险。
fn build_where(filter: &QueryRefsFilter) -> (String, Vec<String>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if filter.space_id.is_some() {
        // 空间过滤需经 collection.space_id JOIN
        clauses.push("c.space_id = ?".to_string());
        binds.push(filter.space_id.clone().unwrap());
    }
    if filter.collection_id.is_some() {
        clauses.push("r.collection_id = ?".to_string());
        binds.push(filter.collection_id.clone().unwrap());
    }
    if filter.ref_type.is_some() {
        clauses.push("r.type = ?".to_string());
        binds.push(filter.ref_type.clone().unwrap());
    }
    if filter.lifecycle.is_some() {
        clauses.push("r.lifecycle = ?".to_string());
        binds.push(filter.lifecycle.clone().unwrap());
    }
    if filter.confidentiality.is_some() {
        clauses.push("r.confidentiality = ?".to_string());
        binds.push(filter.confidentiality.clone().unwrap());
    }
    if filter.disposition.is_some() {
        clauses.push("r.disposition = ?".to_string());
        binds.push(filter.disposition.clone().unwrap());
    }
    if filter.source_id.is_some() {
        clauses.push("r.source_id = ?".to_string());
        binds.push(filter.source_id.clone().unwrap());
    }
    if let Some(kw) = &filter.keyword {
        let trimmed = kw.trim();
        if !trimmed.is_empty() {
            let escaped = escape_like(trimmed);
            let pattern = format!("%{}%", escaped);
            clauses.push(
                "(r.name LIKE ? ESCAPE '\\' OR r.description LIKE ? ESCAPE '\\')".to_string(),
            );
            binds.push(pattern.clone());
            binds.push(pattern);
        }
    }
    if let Some(tags) = &filter.tags {
        for tag in tags {
            let trimmed = tag.trim();
            if trimmed.is_empty() {
                continue;
            }
            // AND 语义：每个标签都要求存在
            clauses.push(
                "EXISTS (SELECT 1 FROM reference_tag rt \
                 WHERE rt.reference_id = r.id AND rt.tag = ?)"
                    .to_string(),
            );
            binds.push(trimmed.to_string());
        }
    }

    let where_sql = clauses.join(" AND ");
    (where_sql, binds)
}

fn row_to_reference(
    row: &sqlx::sqlite::SqliteRow,
    tags: Vec<String>,
) -> Result<Reference, sqlx::Error> {
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

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `query_refs` 内部实现：跨资源集/空间的元数据筛选。
///
/// - 所有过滤参数可选；空 filter 等价于全表扫描（仍受 limit/offset 限制）。
/// - `disposition` 缺省时**不排除**已删除（与 `ref_list` 不同；
///   §2.9 未约定默认排除，由调用方显式传值）。
/// - `total` 为不受分页影响的计数。
pub async fn query_refs_impl(pool: &SqlitePool, filter: QueryRefsFilter) -> CmdResult<QueryRefsResult> {
    // 校验枚举字段
    if let Some(ref t) = filter.ref_type {
        validate_enum(t, &REF_TYPES, "引用类型")?;
    }
    if let Some(ref lc) = filter.lifecycle {
        validate_enum(lc, &LIFECYCLES, "生命周期")?;
    }
    if let Some(ref cf) = filter.confidentiality {
        validate_enum(cf, &CONFIDENTIALITIES, "保密级别")?;
    }
    if let Some(ref d) = filter.disposition {
        validate_enum(d, &DISPOSITIONS, "处置状态")?;
    }

    // 分页参数：默认 50、上限 200；offset 默认 0
    let limit = filter
        .limit
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let offset = filter.offset.unwrap_or(0).max(0);

    let (where_sql, binds) = build_where(&filter);
    let where_clause = if where_sql.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_sql)
    };

    // FROM 子句：仅当按 spaceId 过滤时才需要 JOIN collection。
    // 但为简化拼接，统一 JOIN（SQLite 对单表 JOIN 开销可忽略，
    // 且未来扩展空间级过滤时无需改动）。
    let from_clause = " FROM resource_reference r \
                       INNER JOIN collection c ON r.collection_id = c.id";

    // ---- 1. 计算 total（不受分页影响）----
    let count_sql = format!("SELECT COUNT(*) AS cnt{}{}", from_clause, where_clause);
    let mut count_query = sqlx::query(&count_sql);
    for b in &binds {
        count_query = count_query.bind(b);
    }
    let total: i64 = count_query
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?
        .try_get("cnt")
        .map_err(AppError::from)?;

    // ---- 2. 查询分页 items ----
    let list_sql = format!(
        "SELECT r.id, r.collection_id, r.source_id, r.name, r.type, r.hosting, \
                r.locator_json, r.description, r.lifecycle, r.confidentiality, \
                r.indexed, r.disposition, r.created_at, r.updated_at{}{} \
         ORDER BY r.created_at DESC, r.id ASC LIMIT ? OFFSET ?",
        from_clause, where_clause
    );
    let mut list_query = sqlx::query(&list_sql);
    for b in &binds {
        list_query = list_query.bind(b);
    }
    list_query = list_query.bind(limit).bind(offset);

    let rows = list_query.fetch_all(pool).await.map_err(AppError::from)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("id").map_err(AppError::from)?;
        let tags = fetch_tags(pool, &id).await?;
        items.push(row_to_reference(row, tags).map_err(AppError::from)?);
    }

    Ok(QueryRefsResult { total, items })
}

/// `query_facets` 内部实现：返回四个维度的可用筛选值与计数。
///
/// - `spaceId` 可选；传入时仅统计该空间下的引用。
/// - 各维度按计数降序、值升序排序，保证确定性。
pub async fn query_facets_impl(pool: &SqlitePool, space_id: Option<String>) -> CmdResult<QueryFacetsResult> {
    // 空间过滤：JOIN collection 并按 c.space_id 过滤
    let (from_clause, where_clause, binds): (&str, &str, Vec<String>) = match &space_id {
        Some(sid) => (
            " FROM resource_reference r INNER JOIN collection c ON r.collection_id = c.id",
            " WHERE c.space_id = ?",
            vec![sid.clone()],
        ),
        None => (" FROM resource_reference r", "", vec![]),
    };

    async fn fetch_buckets(
        pool: &SqlitePool,
        column: &str,
        from_clause: &str,
        where_clause: &str,
        binds: &[String],
    ) -> CmdResult<Vec<FacetBucket>> {
        let sql = format!(
            "SELECT r.{col} AS value, COUNT(*) AS count{from}{where} \
             GROUP BY r.{col} ORDER BY count DESC, value ASC",
            col = column,
            from = from_clause,
            where = where_clause
        );
        let mut q = sqlx::query(&sql);
        for b in binds {
            q = q.bind(b);
        }
        let rows = q.fetch_all(pool).await.map_err(AppError::from)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(FacetBucket {
                value: row.try_get("value").map_err(AppError::from)?,
                count: row.try_get("count").map_err(AppError::from)?,
            });
        }
        Ok(out)
    }

    let types = fetch_buckets(pool, "type", from_clause, where_clause, &binds).await?;
    let lifecycles = fetch_buckets(pool, "lifecycle", from_clause, where_clause, &binds).await?;
    let confidentialities =
        fetch_buckets(pool, "confidentiality", from_clause, where_clause, &binds).await?;

    // tags 维度：来源是 reference_tag 表，需要 JOIN 回 resource_reference 应用空间过滤
    let tags = {
        let sql = match &space_id {
            Some(_) => {
                "SELECT rt.tag AS value, COUNT(*) AS count \
                 FROM reference_tag rt \
                 INNER JOIN resource_reference r ON rt.reference_id = r.id \
                 INNER JOIN collection c ON r.collection_id = c.id \
                 WHERE c.space_id = ? \
                 GROUP BY rt.tag ORDER BY count DESC, value ASC"
                    .to_string()
            }
            None => {
                "SELECT rt.tag AS value, COUNT(*) AS count \
                 FROM reference_tag rt \
                 GROUP BY rt.tag ORDER BY count DESC, value ASC"
                    .to_string()
            }
        };
        let mut q = sqlx::query(&sql);
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q.fetch_all(pool).await.map_err(AppError::from)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(FacetBucket {
                value: row.try_get("value").map_err(AppError::from)?,
                count: row.try_get("count").map_err(AppError::from)?,
            });
        }
        out
    };

    Ok(QueryFacetsResult {
        types,
        lifecycles,
        confidentialities,
        tags,
    })
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn query_refs(
    state: tauri::State<'_, crate::AppState>,
    space_id: Option<String>,
    collection_id: Option<String>,
    r#type: Option<String>,
    tags: Option<Vec<String>>,
    lifecycle: Option<String>,
    confidentiality: Option<String>,
    disposition: Option<String>,
    source_id: Option<String>,
    keyword: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CmdResult<QueryRefsResult> {
    query_refs_impl(
        &state.pool,
        QueryRefsFilter {
            space_id,
            collection_id,
            ref_type: r#type,
            tags,
            lifecycle,
            confidentiality,
            disposition,
            source_id,
            keyword,
            limit,
            offset,
        },
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn query_facets(
    state: tauri::State<'_, crate::AppState>,
    space_id: Option<String>,
) -> CmdResult<QueryFacetsResult> {
    query_facets_impl(&state.pool, space_id).await
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_pool_in_memory;
    use crate::reference;
    use uuid::Uuid;

    async fn setup() -> SqlitePool {
        init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";
    const PRESET_SPACE_OTHER: &str = "preset_space_life";

    /// 在指定空间下创建一个资源集，返回其 id。
    async fn make_collection_in(pool: &SqlitePool, space_id: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        sqlx::query(
            "INSERT INTO collection (id, space_id, name, status, created_at, updated_at) \
             VALUES (?, ?, 'c', 'active', ?, ?)",
        )
        .bind(&id)
        .bind(space_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert collection");
        id
    }

    async fn make_collection(pool: &SqlitePool) -> String {
        make_collection_in(pool, PRESET_SPACE).await
    }

    /// 快速创建一条引用；返回其 id。
    async fn make_ref(
        pool: &SqlitePool,
        collection_id: &str,
        name: &str,
        ref_type: &str,
        description: Option<&str>,
        tags: Vec<&str>,
        lifecycle: Option<&str>,
        confidentiality: Option<&str>,
    ) -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join(format!("{}.txt", name));
        std::fs::write(&file, b"x").expect("write");
        // 注意：tempdir 在函数结束时被 drop，但引用已写入数据库，路径存在性校验已通过。
        let locator = reference::Locator {
            kind: "path".into(),
            path: file.to_string_lossy().to_string(),
        };
        let r = reference::create_external(
            pool,
            collection_id.to_string(),
            name.to_string(),
            ref_type.to_string(),
            locator,
            description.map(|s| s.to_string()),
            Some(tags.iter().map(|s| s.to_string()).collect()),
            lifecycle.map(|s| s.to_string()),
            confidentiality.map(|s| s.to_string()),
            None,
        )
        .await
        .expect("create ref");
        r.id
    }

    // ---------- query_refs: 全字段单条件 ----------

    #[tokio::test]
    async fn query_refs_filter_by_space_id() {
        let pool = setup().await;
        let c1 = make_collection_in(&pool, PRESET_SPACE).await;
        let c2 = make_collection_in(&pool, PRESET_SPACE_OTHER).await;
        make_ref(&pool, &c1, "r1", "code", None, vec![], None, None).await;
        make_ref(&pool, &c2, "r2", "code", None, vec![], None, None).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                space_id: Some(PRESET_SPACE.into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "r1");
    }

    #[tokio::test]
    async fn query_refs_filter_by_collection_id() {
        let pool = setup().await;
        let c1 = make_collection(&pool).await;
        let c2 = make_collection(&pool).await;
        make_ref(&pool, &c1, "r1", "code", None, vec![], None, None).await;
        make_ref(&pool, &c2, "r2", "code", None, vec![], None, None).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                collection_id: Some(c1.clone()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].collection_id, c1);
    }

    #[tokio::test]
    async fn query_refs_filter_by_type() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec![], None, None).await;
        make_ref(&pool, &cid, "r2", "document", None, vec![], None, None).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                ref_type: Some("code".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].ref_type, "code");
    }

    #[tokio::test]
    async fn query_refs_filter_by_lifecycle() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec![], Some("staged"), None).await;
        make_ref(&pool, &cid, "r2", "code", None, vec![], Some("delivered"), None).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                lifecycle: Some("staged".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].lifecycle, "staged");
    }

    #[tokio::test]
    async fn query_refs_filter_by_confidentiality() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec![], None, Some("sensitive")).await;
        make_ref(&pool, &cid, "r2", "code", None, vec![], None, Some("public")).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                confidentiality: Some("sensitive".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].confidentiality, "sensitive");
    }

    #[tokio::test]
    async fn query_refs_filter_by_disposition() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let id1 = make_ref(&pool, &cid, "r1", "code", None, vec![], None, None).await;
        make_ref(&pool, &cid, "r2", "code", None, vec![], None, None).await;

        sqlx::query("UPDATE resource_reference SET disposition='archived' WHERE id = ?")
            .bind(&id1)
            .execute(&pool)
            .await
            .expect("archive");

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                disposition: Some("archived".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].disposition, "archived");
    }

    #[tokio::test]
    async fn query_refs_filter_by_source_id() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec![], None, None).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                source_id: Some("src_local_fs_default".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);

        let empty = query_refs_impl(
            &pool,
            QueryRefsFilter {
                source_id: Some("no_such_source".into()),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(empty.total, 0);
    }

    // ---------- query_refs: 多条件组合 ----------

    #[tokio::test]
    async fn query_refs_combined_filters() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec!["core"], Some("active"), Some("internal")).await;
        make_ref(&pool, &cid, "r2", "code", None, vec!["core"], Some("staged"), Some("internal")).await;
        make_ref(&pool, &cid, "r3", "document", None, vec!["core"], Some("active"), Some("internal")).await;
        make_ref(&pool, &cid, "r4", "code", None, vec!["other"], Some("active"), Some("internal")).await;

        let result = query_refs_impl(
            &pool,
            QueryRefsFilter {
                ref_type: Some("code".into()),
                lifecycle: Some("active".into()),
                tags: Some(vec!["core".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("query ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].name, "r1");
    }

    // ---------- query_refs: tags AND 语义 ----------

    #[tokio::test]
    async fn query_refs_tags_and_semantics() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec!["a", "b", "c"], None, None).await;
        make_ref(&pool, &cid, "r2", "code", None, vec!["a", "b"], None, None).await;
        make_ref(&pool, &cid, "r3", "code", None, vec!["a"], None, None).await;
        make_ref(&pool, &cid, "r4", "code", None, vec!["b", "c"], None, None).await;

        // 单标签
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                tags: Some(vec!["a".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 3);

        // 两个标签 AND
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                tags: Some(vec!["a".into(), "b".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 2);
        let names: Vec<_> = r.items.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"r1") && names.contains(&"r2"));

        // 三个标签 AND
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                tags: Some(vec!["a".into(), "b".into(), "c".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].name, "r1");

        // 不存在的标签组合
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                tags: Some(vec!["a".into(), "nonexistent".into()]),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 0);
    }

    // ---------- query_refs: keyword ----------

    #[tokio::test]
    async fn query_refs_keyword_matches_name_and_description() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "设计文档", "document", Some("关于架构"), vec![], None, None).await;
        make_ref(&pool, &cid, "代码", "code", Some("设计模式实现"), vec![], None, None).await;
        make_ref(&pool, &cid, "其他", "code", Some("无关"), vec![], None, None).await;

        // 匹配 name
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                keyword: Some("设计".into()),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 2); // "设计文档" (name) + "代码" (description 含"设计")

        // 仅匹配 description
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                keyword: Some("架构".into()),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].name, "设计文档");
    }

    #[tokio::test]
    async fn query_refs_keyword_escapes_wildcards() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        // 名称含字面值 % 和 _
        make_ref(&pool, &cid, "100%_complete", "code", None, vec![], None, None).await;
        make_ref(&pool, &cid, "100 percent complete", "code", None, vec![], None, None).await;

        // 用户输入 % 应被转义，仅匹配字面值
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                keyword: Some("100%".into()),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].name, "100%_complete");

        // 用户输入 _ 应被转义
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                keyword: Some("%_".into()),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 1);
        assert_eq!(r.items[0].name, "100%_complete");
    }

    // ---------- query_refs: 分页 ----------

    #[tokio::test]
    async fn query_refs_pagination_total_independent_of_limit() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        for i in 0..10 {
            make_ref(&pool, &cid, &format!("r{}", i), "code", None, vec![], None, None).await;
        }

        // 第一页
        let page1 = query_refs_impl(
            &pool,
            QueryRefsFilter {
                limit: Some(3),
                offset: Some(0),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(page1.total, 10);
        assert_eq!(page1.items.len(), 3);

        // 第二页
        let page2 = query_refs_impl(
            &pool,
            QueryRefsFilter {
                limit: Some(3),
                offset: Some(3),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(page2.total, 10);
        assert_eq!(page2.items.len(), 3);

        // 两页 items 不重叠
        let ids1: Vec<_> = page1.items.iter().map(|r| r.id.clone()).collect();
        let ids2: Vec<_> = page2.items.iter().map(|r| r.id.clone()).collect();
        for id in &ids1 {
            assert!(!ids2.contains(id));
        }

        // 最后一页
        let last = query_refs_impl(
            &pool,
            QueryRefsFilter {
                limit: Some(3),
                offset: Some(9),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(last.total, 10);
        assert_eq!(last.items.len(), 1);
    }

    #[tokio::test]
    async fn query_refs_limit_clamped_to_max() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        for i in 0..5 {
            make_ref(&pool, &cid, &format!("r{}", i), "code", None, vec![], None, None).await;
        }

        // limit 超上限 → 截断为 200（不会报错）
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                limit: Some(99999),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.total, 5);
        assert_eq!(r.items.len(), 5);

        // limit=0 → 至少为 1
        let r = query_refs_impl(
            &pool,
            QueryRefsFilter {
                limit: Some(0),
                ..Default::default()
            },
        )
        .await
        .expect("q");
        assert_eq!(r.items.len(), 1);

        // 默认 limit=50
        let r = query_refs_impl(&pool, QueryRefsFilter::default()).await.expect("q");
        assert_eq!(r.items.len(), 5);
    }

    // ---------- query_refs: 排序确定性 ----------

    #[tokio::test]
    async fn query_refs_deterministic_order() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        // 同秒创建多条，触发 id ASC tiebreaker
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = make_ref(&pool, &cid, &format!("r{}", i), "code", None, vec![], None, None).await;
            ids.push(id);
        }

        let r = query_refs_impl(&pool, QueryRefsFilter::default()).await.expect("q");
        assert_eq!(r.items.len(), 5);
        // created_at DESC, id ASC：相同 created_at 时按 id 升序
        for w in r.items.windows(2) {
            let a = &w[0];
            let b = &w[1];
            assert!(
                a.created_at > b.created_at
                    || (a.created_at == b.created_at && a.id < b.id),
                "排序应为 created_at DESC, id ASC"
            );
        }
    }

    // ---------- query_refs: 枚举校验 ----------

    #[tokio::test]
    async fn query_refs_err_invalid_enum() {
        let pool = setup().await;
        let err = query_refs_impl(
            &pool,
            QueryRefsFilter {
                ref_type: Some("bogus".into()),
                ..Default::default()
            },
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = query_refs_impl(
            &pool,
            QueryRefsFilter {
                lifecycle: Some("bogus".into()),
                ..Default::default()
            },
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = query_refs_impl(
            &pool,
            QueryRefsFilter {
                confidentiality: Some("bogus".into()),
                ..Default::default()
            },
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = query_refs_impl(
            &pool,
            QueryRefsFilter {
                disposition: Some("bogus".into()),
                ..Default::default()
            },
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- query_refs: 出参结构契约 ----------

    #[tokio::test]
    async fn query_refs_result_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec!["t1"], None, None).await;

        let r = query_refs_impl(&pool, QueryRefsFilter::default()).await.expect("q");
        let json = serde_json::to_value(&r).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("total"));
        assert!(obj.contains_key("items"));
        let item = obj["items"][0].as_object().expect("item object");
        assert!(item.contains_key("collectionId"));
        assert!(item.contains_key("sourceId"));
        assert!(item.contains_key("createdAt"));
        assert!(item.contains_key("updatedAt"));
        assert!(!item.contains_key("collection_id"));
        // locator 结构化
        let locator = item.get("locator").expect("locator");
        assert!(locator.is_object());
        // tags 数组
        assert!(item["tags"].is_array());
    }

    // ---------- query_facets ----------

    #[tokio::test]
    async fn query_facets_all_spaces() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec!["a", "b"], Some("active"), Some("internal")).await;
        make_ref(&pool, &cid, "r2", "code", None, vec!["a"], Some("staged"), Some("sensitive")).await;
        make_ref(&pool, &cid, "r3", "document", None, vec!["b"], Some("active"), Some("internal")).await;

        let f = query_facets_impl(&pool, None).await.expect("facets");

        // types: code=2, document=1
        assert_eq!(f.types.len(), 2);
        let code = f.types.iter().find(|b| b.value == "code").expect("code");
        assert_eq!(code.count, 2);
        let doc = f.types.iter().find(|b| b.value == "document").expect("doc");
        assert_eq!(doc.count, 1);

        // lifecycles: active=2, staged=1
        let active = f.lifecycles.iter().find(|b| b.value == "active").expect("active");
        assert_eq!(active.count, 2);

        // confidentiality: internal=2, sensitive=1
        let internal = f
            .confidentialities
            .iter()
            .find(|b| b.value == "internal")
            .expect("internal");
        assert_eq!(internal.count, 2);

        // tags: a=2, b=2
        let tag_a = f.tags.iter().find(|b| b.value == "a").expect("a");
        assert_eq!(tag_a.count, 2);
        let tag_b = f.tags.iter().find(|b| b.value == "b").expect("b");
        assert_eq!(tag_b.count, 2);
    }

    #[tokio::test]
    async fn query_facets_filter_by_space() {
        let pool = setup().await;
        let c1 = make_collection_in(&pool, PRESET_SPACE).await;
        let c2 = make_collection_in(&pool, PRESET_SPACE_OTHER).await;
        make_ref(&pool, &c1, "r1", "code", None, vec!["x"], None, None).await;
        make_ref(&pool, &c2, "r2", "document", None, vec!["y"], None, None).await;

        let f = query_facets_impl(&pool, Some(PRESET_SPACE.into())).await.expect("facets");
        assert_eq!(f.types.len(), 1);
        assert_eq!(f.types[0].value, "code");
        assert_eq!(f.tags.len(), 1);
        assert_eq!(f.tags[0].value, "x");

        let f2 = query_facets_impl(&pool, Some(PRESET_SPACE_OTHER.into()))
            .await
            .expect("facets");
        assert_eq!(f2.types.len(), 1);
        assert_eq!(f2.types[0].value, "document");
    }

    #[tokio::test]
    async fn query_facets_empty_db() {
        let pool = setup().await;
        let f = query_facets_impl(&pool, None).await.expect("facets");
        assert!(f.types.is_empty());
        assert!(f.lifecycles.is_empty());
        assert!(f.confidentialities.is_empty());
        assert!(f.tags.is_empty());
    }

    #[tokio::test]
    async fn query_facets_serializes_camel_case() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        make_ref(&pool, &cid, "r1", "code", None, vec!["t"], None, None).await;

        let f = query_facets_impl(&pool, None).await.expect("facets");
        let json = serde_json::to_value(&f).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("types"));
        assert!(obj.contains_key("lifecycles"));
        assert!(obj.contains_key("confidentialities"));
        assert!(obj.contains_key("tags"));
        let bucket = obj["types"][0].as_object().expect("bucket");
        assert!(bucket.contains_key("value"));
        assert!(bucket.contains_key("count"));
    }
}
