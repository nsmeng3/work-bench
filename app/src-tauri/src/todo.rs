//! 待办管理（M7-2 · 任务包 m7-7.2）
//!
//! 9 个命令：`todo_list` / `todo_get` / `todo_create` / `todo_update` /
//! `todo_set_status` / `todo_delete` / `todo_link_ref` / `todo_unlink_ref` /
//! `todo_list_by_ref`。
//!
//! 关键设计：
//! - todo 挂 `space_id`（可空，NULL 表示全局 todo），不挂 collection_id。
//! - 状态机四态：pending / doing / done / cancelled。
//!   `done` / `cancelled` 允许回到 `pending`（重新打开 / 恢复）。
//! - 切到 `done` 时写 `done_at = now`；切出 `done` 时清 `done_at = NULL`。
//! - 资源引用通过 `todo_ref_link` 多对多关联；`link_ref` / `unlink_ref` 幂等。
//! - 删除空间 → `todo.space_id` 自动 SET NULL（不丢 todo）。
//! - 删除引用 → `todo_ref_link` 级联清理。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};
use crate::reference::Reference;

/// 标题最大长度（与 0007_todo.sql CHECK 对齐）。
const TITLE_MAX_LEN: usize = 200;

/// 合法状态集合。
const STATUSES: [&str; 4] = ["pending", "doing", "done", "cancelled"];

// ============================================================
// 类型（契约：camelCase 序列化）
// ============================================================

/// 待办实体（与 `todo` 表一一对应）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// pending | doing | done | cancelled
    pub status: String,
    /// NULL 表示全局 todo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    /// 0 普通 / 1 重要 / 2 紧急
    pub priority: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_at: Option<i64>,
    pub sort_order: i64,
    /// Unix 秒
    pub created_at: i64,
    /// Unix 秒
    pub updated_at: i64,
}

/// `todo_get` 出参：Todo + 挂载的资源引用数组。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoWithRefs {
    #[serde(flatten)]
    pub todo: Todo,
    pub refs: Vec<Reference>,
}

/// `todo_create` 入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoCreateInput {
    pub title: String,
    pub note: Option<String>,
    pub space_id: Option<String>,
    pub priority: Option<i64>,
    pub due_at: Option<i64>,
}

/// `todo_update` 入参（patch 语义）。
///
/// 外层 `Option` 表示"是否更新该字段"：
/// - `None` → 不更新（保留原值）
/// - `Some(x)` → 更新为 x
///
/// 对可空字段（`note` / `space_id` / `due_at`）使用 `Option<Option<T>>`：
/// - `None` → 不更新
/// - `Some(None)` → 置空（写 NULL）
/// - `Some(Some(v))` → 更新为 v
///
/// 通过 `double_option` 自定义反序列化区分"字段缺失"与"显式 null"。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodoPatch {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub note: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub space_id: Option<Option<String>>,
    pub priority: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    pub due_at: Option<Option<i64>>,
    pub sort_order: Option<i64>,
}

/// 区分"字段缺失"与"显式 null"的反序列化器：
/// - 字段缺失 → `None`（由 `#[serde(default)]` 提供）
/// - 字段为 `null` → `Some(None)`
/// - 字段为值 → `Some(Some(v))`
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    // 能调到这里说明字段存在（否则走 default）；直接按 Option<T> 解析，
    // null → None → 包一层 Some(None)；值 → Some(v) → Some(Some(v))。
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

// ============================================================
// 内部工具
// ============================================================

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn validate_title(title: &str) -> CmdResult<()> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_param("待办标题不能为空"));
    }
    if trimmed.chars().count() > TITLE_MAX_LEN {
        return Err(AppError::invalid_param(format!(
            "待办标题超长（>{} 字符）",
            TITLE_MAX_LEN
        )));
    }
    Ok(())
}

fn validate_status(status: &str) -> CmdResult<()> {
    if !STATUSES.contains(&status) {
        return Err(AppError::invalid_param(format!(
            "status 取值非法: {}（应为 pending|doing|done|cancelled）",
            status
        )));
    }
    Ok(())
}

fn validate_priority(priority: i64) -> CmdResult<()> {
    if !(0..=2).contains(&priority) {
        return Err(AppError::invalid_param(format!(
            "priority 取值非法: {}（应为 0|1|2）",
            priority
        )));
    }
    Ok(())
}

fn row_to_todo(row: &sqlx::sqlite::SqliteRow) -> Result<Todo, sqlx::Error> {
    Ok(Todo {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        note: row.try_get("note")?,
        status: row.try_get("status")?,
        space_id: row.try_get("space_id")?,
        priority: row.try_get("priority")?,
        due_at: row.try_get("due_at")?,
        done_at: row.try_get("done_at")?,
        sort_order: row.try_get("sort_order")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn fetch_todo(pool: &SqlitePool, id: &str) -> CmdResult<Todo> {
    let row = sqlx::query(
        "SELECT id, title, note, status, space_id, priority, due_at, done_at, \
                sort_order, created_at, updated_at \
         FROM todo WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("待办不存在: {}", id)))?;
    row_to_todo(&row).map_err(AppError::from)
}

/// 拉取 todo 挂载的引用列表（按挂载时间升序）。
async fn fetch_todo_refs(pool: &SqlitePool, todo_id: &str) -> CmdResult<Vec<Reference>> {
    let rows = sqlx::query(
        "SELECT r.id, r.collection_id, r.source_id, r.name, r.type, r.hosting, r.locator_json, \
                r.description, r.lifecycle, r.confidentiality, r.indexed, r.disposition, \
                r.created_at, r.updated_at \
         FROM todo_ref_link l \
         JOIN resource_reference r ON r.id = l.ref_id \
         WHERE l.todo_id = ? \
         ORDER BY l.created_at ASC, r.id ASC",
    )
    .bind(todo_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let locator_json: String = row.try_get("locator_json").map_err(AppError::from)?;
        let locator: serde_json::Value =
            serde_json::from_str(&locator_json).unwrap_or(serde_json::Value::Null);
        let indexed_int: i64 = row.try_get("indexed").map_err(AppError::from)?;
        let ref_id: String = row.try_get("id").map_err(AppError::from)?;
        // 引用标签（reference_tag）
        let tag_rows = sqlx::query(
            "SELECT tag FROM reference_tag WHERE reference_id = ? ORDER BY tag ASC",
        )
        .bind(&ref_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        let tags: Vec<String> = tag_rows.iter().map(|r| r.get::<String, _>("tag")).collect();

        out.push(Reference {
            id: ref_id,
            collection_id: row.try_get("collection_id").map_err(AppError::from)?,
            source_id: row.try_get("source_id").map_err(AppError::from)?,
            name: row.try_get("name").map_err(AppError::from)?,
            ref_type: row.try_get("type").map_err(AppError::from)?,
            hosting: row.try_get("hosting").map_err(AppError::from)?,
            locator,
            description: row.try_get("description").map_err(AppError::from)?,
            lifecycle: row.try_get("lifecycle").map_err(AppError::from)?,
            confidentiality: row.try_get("confidentiality").map_err(AppError::from)?,
            indexed: indexed_int != 0,
            disposition: row.try_get("disposition").map_err(AppError::from)?,
            tags,
            created_at: row.try_get("created_at").map_err(AppError::from)?,
            updated_at: row.try_get("updated_at").map_err(AppError::from)?,
        });
    }
    Ok(out)
}

/// 校验空间存在（仅当 space_id 非空时调用）。
async fn ensure_space_exists(pool: &SqlitePool, space_id: &str) -> CmdResult<()> {
    sqlx::query("SELECT id FROM space WHERE id = ?")
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("空间不存在: {}", space_id)))?;
    Ok(())
}

/// 校验引用存在。
async fn ensure_reference_exists(pool: &SqlitePool, ref_id: &str) -> CmdResult<()> {
    sqlx::query("SELECT id FROM resource_reference WHERE id = ?")
        .bind(ref_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("资源引用不存在: {}", ref_id)))?;
    Ok(())
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `todo_list`：全部 / 按空间 / 按状态筛选。
///
/// - `space_id` 为 `Some("global")` 时仅返回全局 todo（space_id IS NULL）。
/// - `status` 缺省时返回 pending + doing（不含 done/cancelled）；
///   传 `"all"` 返回全部；传具体状态值时按该状态过滤。
/// - `include_done` 为兼容参数：true 等价于 status="all"。
pub async fn list(
    pool: &SqlitePool,
    space_id: Option<String>,
    status: Option<String>,
    include_done: Option<bool>,
) -> CmdResult<Vec<Todo>> {
    // 解析 status 过滤
    let status_filter: Option<String> = match (status.as_deref(), include_done) {
        (Some("all"), _) | (None, Some(true)) => None, // 不过滤
        (Some(s), _) => {
            validate_status(s)?;
            Some(s.to_string())
        }
        (None, _) => Some("__active__".to_string()), // 特殊标记：pending+doing
    };

    // 解析 space 过滤
    let space_filter: Option<Option<String>> = match space_id.as_deref() {
        None => None,                          // 不过滤
        Some("global") => Some(None),          // 仅全局
        Some(s) => Some(Some(s.to_string())),  // 指定空间
    };

    let mut sql = String::from(
        "SELECT id, title, note, status, space_id, priority, due_at, done_at, \
                sort_order, created_at, updated_at FROM todo WHERE 1=1",
    );
    let mut binds: Vec<String> = Vec::new();

    match &space_filter {
        None => {}
        Some(None) => sql.push_str(" AND space_id IS NULL"),
        Some(Some(s)) => {
            sql.push_str(" AND space_id = ?");
            binds.push(s.clone());
        }
    }

    match &status_filter {
        None => {}
        Some(s) if s == "__active__" => {
            sql.push_str(" AND status IN ('pending','doing')");
        }
        Some(s) => {
            sql.push_str(" AND status = ?");
            binds.push(s.clone());
        }
    }

    sql.push_str(" ORDER BY sort_order ASC, created_at DESC, id ASC");

    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(pool).await.map_err(AppError::from)?;
    rows.iter().map(row_to_todo).collect::<Result<_, _>>().map_err(AppError::from)
}

/// `todo_get`：含挂载的引用数组。
pub async fn get(pool: &SqlitePool, id: String) -> CmdResult<TodoWithRefs> {
    let todo = fetch_todo(pool, &id).await?;
    let refs = fetch_todo_refs(pool, &id).await?;
    Ok(TodoWithRefs { todo, refs })
}

/// `todo_create`：title 必填，其他可空。
pub async fn create(pool: &SqlitePool, input: TodoCreateInput) -> CmdResult<Todo> {
    validate_title(&input.title)?;
    if let Some(p) = input.priority {
        validate_priority(p)?;
    }
    if let Some(ref sid) = input.space_id {
        ensure_space_exists(pool, sid).await?;
    }

    let id = Uuid::new_v4().to_string();
    let now = now_unix();
    sqlx::query(
        "INSERT INTO todo (id, title, note, status, space_id, priority, due_at, \
                          done_at, sort_order, created_at, updated_at) \
         VALUES (?, ?, ?, 'pending', ?, ?, ?, NULL, 0, ?, ?)",
    )
    .bind(&id)
    .bind(input.title.trim())
    .bind(&input.note)
    .bind(&input.space_id)
    .bind(input.priority.unwrap_or(0))
    .bind(input.due_at)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    fetch_todo(pool, &id).await
}

/// `todo_update`：patch 语义，仅更新传入字段。
pub async fn update(pool: &SqlitePool, id: String, patch: TodoPatch) -> CmdResult<Todo> {
    let existing = fetch_todo(pool, &id).await?;

    if let Some(ref t) = patch.title {
        validate_title(t)?;
    }
    if let Some(p) = patch.priority {
        validate_priority(p)?;
    }
    if let Some(Some(ref sid)) = patch.space_id {
        ensure_space_exists(pool, sid).await?;
    }

    let new_title = patch
        .title
        .as_deref()
        .map(str::trim)
        .map(str::to_string)
        .unwrap_or_else(|| existing.title.clone());
    let new_note = match &patch.note {
        None => existing.note.clone(),
        Some(inner) => inner.clone(),
    };
    let new_space_id = match &patch.space_id {
        None => existing.space_id.clone(),
        Some(inner) => inner.clone(),
    };
    let new_priority = patch.priority.unwrap_or(existing.priority);
    let new_due_at = match &patch.due_at {
        None => existing.due_at,
        Some(inner) => *inner,
    };
    let new_sort_order = patch.sort_order.unwrap_or(existing.sort_order);
    let now = now_unix();

    sqlx::query(
        "UPDATE todo SET title = ?, note = ?, space_id = ?, priority = ?, \
                        due_at = ?, sort_order = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&new_title)
    .bind(&new_note)
    .bind(&new_space_id)
    .bind(new_priority)
    .bind(new_due_at)
    .bind(new_sort_order)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    fetch_todo(pool, &id).await
}

/// `todo_set_status`：状态机 + done_at 写/清。
///
/// 状态机（任务包 §状态机）：
/// - pending → doing / done / cancelled
/// - doing   → pending / done / cancelled
/// - done    → pending（重新打开）
/// - cancelled → pending（恢复）
///
/// 切到 done 时写 done_at = now；切出 done 时清 done_at = NULL。
pub async fn set_status(pool: &SqlitePool, id: String, status: String) -> CmdResult<Todo> {
    validate_status(&status)?;
    let existing = fetch_todo(pool, &id).await?;

    // 状态机校验
    let from = existing.status.as_str();
    let to = status.as_str();
    let allowed = match from {
        "pending" => matches!(to, "pending" | "doing" | "done" | "cancelled"),
        "doing" => matches!(to, "pending" | "doing" | "done" | "cancelled"),
        "done" => matches!(to, "pending" | "done"),
        "cancelled" => matches!(to, "pending" | "cancelled"),
        _ => false,
    };
    if !allowed {
        return Err(AppError::conflict(format!(
            "非法状态迁移: {} → {}",
            from, to
        )));
    }

    let now = now_unix();
    let new_done_at: Option<i64> = match (from, to) {
        (_, "done") => Some(now),       // 进入 done：记录完成时间
        ("done", _) => None,            // 离开 done：清空
        _ => existing.done_at,          // 其他迁移保持原值（理论上不会到这）
    };

    sqlx::query("UPDATE todo SET status = ?, done_at = ?, updated_at = ? WHERE id = ?")
        .bind(to)
        .bind(new_done_at)
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

    fetch_todo(pool, &id).await
}

/// `todo_delete`：硬删除（todo_ref_link 由 ON DELETE CASCADE 清理）。
pub async fn delete(pool: &SqlitePool, id: String) -> CmdResult<()> {
    // 先确认存在，保持与其他模块一致的 NOT_FOUND 语义
    let _ = fetch_todo(pool, &id).await?;
    sqlx::query("DELETE FROM todo WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// `todo_link_ref`：挂载资源引用到 todo（幂等：重复挂不报错）。
pub async fn link_ref(pool: &SqlitePool, todo_id: String, ref_id: String) -> CmdResult<()> {
    let _ = fetch_todo(pool, &todo_id).await?;
    ensure_reference_exists(pool, &ref_id).await?;

    let now = now_unix();
    sqlx::query(
        "INSERT OR IGNORE INTO todo_ref_link (todo_id, ref_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(&todo_id)
    .bind(&ref_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    // 触碰 todo.updated_at，便于前端按更新时间排序
    sqlx::query("UPDATE todo SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&todo_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// `todo_unlink_ref`：解除挂载（幂等：不存在也不报错）。
pub async fn unlink_ref(pool: &SqlitePool, todo_id: String, ref_id: String) -> CmdResult<()> {
    let _ = fetch_todo(pool, &todo_id).await?;

    sqlx::query("DELETE FROM todo_ref_link WHERE todo_id = ? AND ref_id = ?")
        .bind(&todo_id)
        .bind(&ref_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;

    let now = now_unix();
    sqlx::query("UPDATE todo SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&todo_id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// `todo_list_by_ref`：反查"被哪些 todo 引用"（资源详情页用）。
pub async fn list_by_ref(pool: &SqlitePool, ref_id: String) -> CmdResult<Vec<Todo>> {
    ensure_reference_exists(pool, &ref_id).await?;
    let rows = sqlx::query(
        "SELECT t.id, t.title, t.note, t.status, t.space_id, t.priority, t.due_at, t.done_at, \
                t.sort_order, t.created_at, t.updated_at \
         FROM todo_ref_link l \
         JOIN todo t ON t.id = l.todo_id \
         WHERE l.ref_id = ? \
         ORDER BY t.created_at DESC, t.id ASC",
    )
    .bind(&ref_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    rows.iter().map(row_to_todo).collect::<Result<_, _>>().map_err(AppError::from)
}

// ============================================================
// m7-7.3 · todo_today（Dashboard 今日待办）
// ============================================================

/// `todo_today`：返回 status IN ('pending','doing') 的 todo，
/// 按 priority DESC, due_at IS NULL, due_at ASC, created_at ASC 排序。
///
/// 与 `todo_list` 的差异：
/// - 固定 status 过滤（不支持自定义）；
/// - 固定排序规则（面向"今日该做什么"的优先级视角）；
/// - 固定 LIMIT 20（Dashboard 首页只展示前几条，避免一次拉全量）。
pub async fn today(pool: &SqlitePool) -> CmdResult<Vec<Todo>> {
    let rows = sqlx::query(
        "SELECT id, title, note, status, space_id, priority, due_at, done_at, \
                sort_order, created_at, updated_at \
         FROM todo \
         WHERE status IN ('pending','doing') \
         ORDER BY priority DESC, \
                  due_at IS NULL, due_at ASC, \
                  created_at ASC \
         LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    rows.iter().map(row_to_todo).collect::<Result<_, _>>().map_err(AppError::from)
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn todo_list(
    state: tauri::State<'_, crate::AppState>,
    space_id: Option<String>,
    status: Option<String>,
    include_done: Option<bool>,
) -> CmdResult<Vec<Todo>> {
    list(&state.pool, space_id, status, include_done).await
}

#[tauri::command]
pub async fn todo_get(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<TodoWithRefs> {
    get(&state.pool, id).await
}

#[tauri::command]
pub async fn todo_create(
    state: tauri::State<'_, crate::AppState>,
    input: TodoCreateInput,
) -> CmdResult<Todo> {
    create(&state.pool, input).await
}

#[tauri::command]
pub async fn todo_update(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    patch: TodoPatch,
) -> CmdResult<Todo> {
    update(&state.pool, id, patch).await
}

#[tauri::command]
pub async fn todo_set_status(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    status: String,
) -> CmdResult<Todo> {
    set_status(&state.pool, id, status).await
}

#[tauri::command]
pub async fn todo_delete(state: tauri::State<'_, crate::AppState>, id: String) -> CmdResult<()> {
    delete(&state.pool, id).await
}

#[tauri::command]
pub async fn todo_link_ref(
    state: tauri::State<'_, crate::AppState>,
    todo_id: String,
    ref_id: String,
) -> CmdResult<()> {
    link_ref(&state.pool, todo_id, ref_id).await
}

#[tauri::command]
pub async fn todo_unlink_ref(
    state: tauri::State<'_, crate::AppState>,
    todo_id: String,
    ref_id: String,
) -> CmdResult<()> {
    unlink_ref(&state.pool, todo_id, ref_id).await
}

#[tauri::command]
pub async fn todo_list_by_ref(
    state: tauri::State<'_, crate::AppState>,
    ref_id: String,
) -> CmdResult<Vec<Todo>> {
    list_by_ref(&state.pool, ref_id).await
}

#[tauri::command]
pub async fn todo_today(state: tauri::State<'_, crate::AppState>) -> CmdResult<Vec<Todo>> {
    today(&state.pool).await
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

    /// 在指定资源集插入一条引用，返回 ref_id。
    async fn insert_ref(pool: &SqlitePool, id: &str, collection_id: &str) {
        let now = now_unix();
        sqlx::query(
            "INSERT INTO resource_reference \
             (id, collection_id, source_id, name, type, hosting, locator_json, \
              lifecycle, confidentiality, indexed, disposition, created_at, updated_at) \
             VALUES (?, ?, 'src_local_fs_default', ?, 'code', 'external', '{}', \
                     'active', 'internal', 1, 'none', ?, ?)",
        )
        .bind(id)
        .bind(collection_id)
        .bind(format!("ref-{}", id))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert ref");
    }

    /// 在预置空间下新建一个资源集，返回 collection_id。
    async fn insert_collection(pool: &SqlitePool, id: &str) {
        let now = now_unix();
        sqlx::query(
            "INSERT INTO collection (id, space_id, name, status, created_at, updated_at) \
             VALUES (?, ?, ?, 'active', ?, ?)",
        )
        .bind(id)
        .bind(PRESET_SPACE)
        .bind(format!("col-{}", id))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert collection");
    }

    fn minimal_input(title: &str) -> TodoCreateInput {
        TodoCreateInput {
            title: title.to_string(),
            note: None,
            space_id: None,
            priority: None,
            due_at: None,
        }
    }

    // ---------- todo_create ----------

    #[tokio::test]
    async fn todo_create_ok_minimal_global() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("写月报")).await.expect("create ok");
        assert_eq!(t.title, "写月报");
        assert_eq!(t.status, "pending");
        assert_eq!(t.space_id, None, "默认全局 todo");
        assert_eq!(t.priority, 0);
        assert_eq!(t.due_at, None);
        assert_eq!(t.done_at, None);
        assert_eq!(t.sort_order, 0);
        assert!(t.created_at > 0);
        assert_eq!(t.created_at, t.updated_at);
    }

    #[tokio::test]
    async fn todo_create_ok_with_space_and_fields() {
        let pool = setup().await;
        let input = TodoCreateInput {
            title: "评审 PR #42".into(),
            note: Some("关注性能".into()),
            space_id: Some(PRESET_SPACE.into()),
            priority: Some(2),
            due_at: Some(1_800_000_000),
        };
        let t = create(&pool, input).await.expect("create ok");
        assert_eq!(t.space_id.as_deref(), Some(PRESET_SPACE));
        assert_eq!(t.priority, 2);
        assert_eq!(t.due_at, Some(1_800_000_000));
        assert_eq!(t.note.as_deref(), Some("关注性能"));
    }

    #[tokio::test]
    async fn todo_create_err_empty_title() {
        let pool = setup().await;
        let err = create(&pool, minimal_input("   "))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn todo_create_err_too_long_title() {
        let pool = setup().await;
        let long = "x".repeat(TITLE_MAX_LEN + 1);
        let err = create(&pool, minimal_input(&long))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn todo_create_err_invalid_priority() {
        let pool = setup().await;
        let mut input = minimal_input("x");
        input.priority = Some(3);
        let err = create(&pool, input).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn todo_create_err_space_not_found() {
        let pool = setup().await;
        let mut input = minimal_input("x");
        input.space_id = Some("no-such-space".into());
        let err = create(&pool, input).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- todo_get（含 refs JOIN） ----------

    #[tokio::test]
    async fn todo_get_ok_with_refs() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;
        insert_ref(&pool, "r2", "c1").await;

        let t = create(&pool, minimal_input("带引用")).await.expect("create");
        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("link r1");
        link_ref(&pool, t.id.clone(), "r2".into()).await.expect("link r2");

        let detail = get(&pool, t.id.clone()).await.expect("get ok");
        assert_eq!(detail.todo.id, t.id);
        assert_eq!(detail.refs.len(), 2);
        let ids: Vec<&str> = detail.refs.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"r1"));
        assert!(ids.contains(&"r2"));
    }

    #[tokio::test]
    async fn todo_get_err_not_found() {
        let pool = setup().await;
        let err = get(&pool, "no-such".into()).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- todo_update（patch 语义） ----------

    #[tokio::test]
    async fn todo_update_patch_no_fields_keeps_original() {
        let pool = setup().await;
        let t = create(&pool, TodoCreateInput {
            title: "原标题".into(),
            note: Some("原备注".into()),
            space_id: Some(PRESET_SPACE.into()),
            priority: Some(1),
            due_at: Some(123),
        })
        .await
        .expect("create");

        // 所有字段都不传 → 保持原值
        let updated = update(&pool, t.id.clone(), TodoPatch::default())
            .await
            .expect("update");
        assert_eq!(updated.title, "原标题");
        assert_eq!(updated.note.as_deref(), Some("原备注"));
        assert_eq!(updated.space_id.as_deref(), Some(PRESET_SPACE));
        assert_eq!(updated.priority, 1);
        assert_eq!(updated.due_at, Some(123));
    }

    #[tokio::test]
    async fn todo_update_patch_update_value() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("old")).await.expect("create");
        let updated = update(
            &pool,
            t.id.clone(),
            TodoPatch {
                title: Some("new".into()),
                note: Some(Some("备注".into())),
                space_id: Some(Some(PRESET_SPACE.into())),
                priority: Some(2),
                due_at: Some(Some(999)),
                sort_order: Some(5),
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.title, "new");
        assert_eq!(updated.note.as_deref(), Some("备注"));
        assert_eq!(updated.space_id.as_deref(), Some(PRESET_SPACE));
        assert_eq!(updated.priority, 2);
        assert_eq!(updated.due_at, Some(999));
        assert_eq!(updated.sort_order, 5);
    }

    #[tokio::test]
    async fn todo_update_patch_clear_nullable_fields() {
        let pool = setup().await;
        let t = create(&pool, TodoCreateInput {
            title: "t".into(),
            note: Some("n".into()),
            space_id: Some(PRESET_SPACE.into()),
            priority: None,
            due_at: Some(42),
        })
        .await
        .expect("create");

        // Some(None) → 置空
        let updated = update(
            &pool,
            t.id.clone(),
            TodoPatch {
                note: Some(None),
                space_id: Some(None),
                due_at: Some(None),
                ..Default::default()
            },
        )
        .await
        .expect("update");
        assert_eq!(updated.note, None);
        assert_eq!(updated.space_id, None, "置空后变全局 todo");
        assert_eq!(updated.due_at, None);
    }

    #[tokio::test]
    async fn todo_update_err_not_found() {
        let pool = setup().await;
        let err = update(&pool, "no-such".into(), TodoPatch::default())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn todo_update_err_invalid_title() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("a")).await.expect("create");
        let err = update(
            &pool,
            t.id.clone(),
            TodoPatch {
                title: Some("".into()),
                ..Default::default()
            },
        )
        .await
        .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- todo_set_status（状态机 + done_at） ----------

    #[tokio::test]
    async fn todo_set_status_pending_to_done_writes_done_at() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        assert_eq!(t.done_at, None);

        let done = set_status(&pool, t.id.clone(), "done".into())
            .await
            .expect("to done");
        assert_eq!(done.status, "done");
        assert!(done.done_at.is_some(), "进入 done 必须写 done_at");
        let done_at = done.done_at.unwrap();
        assert!(done_at >= t.created_at);
    }

    #[tokio::test]
    async fn todo_set_status_done_to_pending_clears_done_at() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let done = set_status(&pool, t.id.clone(), "done".into())
            .await
            .expect("to done");
        assert!(done.done_at.is_some());

        let reopened = set_status(&pool, t.id.clone(), "pending".into())
            .await
            .expect("reopen");
        assert_eq!(reopened.status, "pending");
        assert_eq!(reopened.done_at, None, "离开 done 必须清 done_at");
    }

    #[tokio::test]
    async fn todo_set_status_doing_to_done_to_pending() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let doing = set_status(&pool, t.id.clone(), "doing".into())
            .await
            .expect("doing");
        assert_eq!(doing.status, "doing");
        assert_eq!(doing.done_at, None);

        let done = set_status(&pool, t.id.clone(), "done".into())
            .await
            .expect("done");
        assert!(done.done_at.is_some());

        let back = set_status(&pool, t.id.clone(), "pending".into())
            .await
            .expect("pending");
        assert_eq!(back.done_at, None);
    }

    #[tokio::test]
    async fn todo_set_status_cancelled_to_pending_allowed() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let cancelled = set_status(&pool, t.id.clone(), "cancelled".into())
            .await
            .expect("cancelled");
        assert_eq!(cancelled.status, "cancelled");

        let restored = set_status(&pool, t.id.clone(), "pending".into())
            .await
            .expect("restore");
        assert_eq!(restored.status, "pending");
    }

    #[tokio::test]
    async fn todo_set_status_done_to_doing_rejected() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        set_status(&pool, t.id.clone(), "done".into())
            .await
            .expect("done");
        let err = set_status(&pool, t.id.clone(), "doing".into())
            .await
            .expect_err("done→doing 不允许");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    #[tokio::test]
    async fn todo_set_status_cancelled_to_done_rejected() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        set_status(&pool, t.id.clone(), "cancelled".into())
            .await
            .expect("cancelled");
        let err = set_status(&pool, t.id.clone(), "done".into())
            .await
            .expect_err("cancelled→done 不允许");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    #[tokio::test]
    async fn todo_set_status_err_invalid_status() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let err = set_status(&pool, t.id.clone(), "weird".into())
            .await
            .expect_err("invalid status");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn todo_set_status_err_not_found() {
        let pool = setup().await;
        let err = set_status(&pool, "no-such".into(), "done".into())
            .await
            .expect_err("not found");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- todo_delete（级联清 todo_ref_link） ----------

    #[tokio::test]
    async fn todo_delete_cascades_ref_links() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;

        let t = create(&pool, minimal_input("x")).await.expect("create");
        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("link");

        delete(&pool, t.id.clone()).await.expect("delete");

        // link 已被级联删除
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM todo_ref_link WHERE todo_id = ?",
        )
        .bind(&t.id)
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 0);

        // 引用本身保留
        let (ref_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM resource_reference WHERE id = 'r1'")
                .fetch_one(&pool)
                .await
                .expect("count ref");
        assert_eq!(ref_count, 1);
    }

    #[tokio::test]
    async fn todo_delete_err_not_found() {
        let pool = setup().await;
        let err = delete(&pool, "no-such".into()).await.expect_err("not found");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- link_ref / unlink_ref 幂等 ----------

    #[tokio::test]
    async fn todo_link_ref_idempotent() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;
        let t = create(&pool, minimal_input("x")).await.expect("create");

        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("first");
        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("second ok");

        let detail = get(&pool, t.id.clone()).await.expect("get");
        assert_eq!(detail.refs.len(), 1, "重复挂不产生重复行");
    }

    #[tokio::test]
    async fn todo_link_ref_err_todo_not_found() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;
        let err = link_ref(&pool, "no-such".into(), "r1".into())
            .await
            .expect_err("todo not found");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn todo_link_ref_err_ref_not_found() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let err = link_ref(&pool, t.id.clone(), "no-such".into())
            .await
            .expect_err("ref not found");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn todo_unlink_ref_idempotent() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;
        let t = create(&pool, minimal_input("x")).await.expect("create");

        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("link");
        unlink_ref(&pool, t.id.clone(), "r1".into()).await.expect("unlink");
        // 再删一次：幂等
        unlink_ref(&pool, t.id.clone(), "r1".into()).await.expect("unlink again");

        let detail = get(&pool, t.id.clone()).await.expect("get");
        assert_eq!(detail.refs.len(), 0);
    }

    // ---------- 外键行为：space 删除 → todo.space_id SET NULL ----------

    #[tokio::test]
    async fn todo_space_delete_sets_null() {
        let pool = setup().await;
        // 新建一个可删除的空间（预置空间下挂着 collection，RESTRICT 删不掉）
        let now = now_unix();
        sqlx::query(
            "INSERT INTO space (id, name, status, created_at, updated_at) \
             VALUES ('sp_tmp', '临时', 'active', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert space");

        let t = create(&pool, TodoCreateInput {
            title: "x".into(),
            note: None,
            space_id: Some("sp_tmp".into()),
            priority: None,
            due_at: None,
        })
        .await
        .expect("create");
        assert_eq!(t.space_id.as_deref(), Some("sp_tmp"));

        sqlx::query("DELETE FROM space WHERE id = 'sp_tmp'")
            .execute(&pool)
            .await
            .expect("delete space");

        let after = fetch_todo(&pool, &t.id).await.expect("fetch");
        assert_eq!(after.space_id, None, "ON DELETE SET NULL 生效");
    }

    // ---------- 外键行为：reference 删除 → todo_ref_link CASCADE ----------

    #[tokio::test]
    async fn todo_ref_delete_cascades_link() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        link_ref(&pool, t.id.clone(), "r1".into()).await.expect("link");

        sqlx::query("DELETE FROM resource_reference WHERE id = 'r1'")
            .execute(&pool)
            .await
            .expect("delete ref");

        let detail = get(&pool, t.id.clone()).await.expect("get");
        assert_eq!(detail.refs.len(), 0, "ON DELETE CASCADE 清理挂载");
    }

    // ---------- todo_list_by_ref ----------

    #[tokio::test]
    async fn todo_list_by_ref_returns_linking_todos() {
        let pool = setup().await;
        insert_collection(&pool, "c1").await;
        insert_ref(&pool, "r1", "c1").await;

        let t1 = create(&pool, minimal_input("t1")).await.expect("c1");
        let t2 = create(&pool, minimal_input("t2")).await.expect("c2");
        let _t3 = create(&pool, minimal_input("t3")).await.expect("c3");

        link_ref(&pool, t1.id.clone(), "r1".into()).await.expect("l1");
        link_ref(&pool, t2.id.clone(), "r1".into()).await.expect("l2");

        let list = list_by_ref(&pool, "r1".into()).await.expect("list");
        assert_eq!(list.len(), 2);
        let ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&t1.id.as_str()));
        assert!(ids.contains(&t2.id.as_str()));
    }

    #[tokio::test]
    async fn todo_list_by_ref_err_ref_not_found() {
        let pool = setup().await;
        let err = list_by_ref(&pool, "no-such".into())
            .await
            .expect_err("not found");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- todo_list 筛选 ----------

    #[tokio::test]
    async fn todo_list_default_excludes_done_and_cancelled() {
        let pool = setup().await;
        let t1 = create(&pool, minimal_input("pending-1")).await.expect("c");
        let _t2 = create(&pool, minimal_input("pending-2")).await.expect("c");
        set_status(&pool, t1.id.clone(), "done".into()).await.expect("done");

        let list = list(&pool, None, None, None).await.expect("list");
        assert_eq!(list.len(), 1, "默认仅 pending+doing");
        assert_eq!(list[0].title, "pending-2");
    }

    #[tokio::test]
    async fn todo_list_include_done_via_status_all() {
        let pool = setup().await;
        let t1 = create(&pool, minimal_input("a")).await.expect("c");
        let _t2 = create(&pool, minimal_input("b")).await.expect("c");
        set_status(&pool, t1.id.clone(), "done".into()).await.expect("done");

        let list = list(&pool, None, Some("all".into()), None)
            .await
            .expect("list");
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn todo_list_filter_by_space() {
        let pool = setup().await;
        create(&pool, minimal_input("global")).await.expect("g");
        create(&pool, TodoCreateInput {
            title: "work".into(),
            note: None,
            space_id: Some(PRESET_SPACE.into()),
            priority: None,
            due_at: None,
        })
        .await
        .expect("w");

        let work = list(&pool, Some(PRESET_SPACE.into()), None, None)
            .await
            .expect("list");
        assert_eq!(work.len(), 1);
        assert_eq!(work[0].title, "work");

        let global = list(&pool, Some("global".into()), None, None)
            .await
            .expect("list");
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].title, "global");
    }

    #[tokio::test]
    async fn todo_list_filter_by_status() {
        let pool = setup().await;
        let t1 = create(&pool, minimal_input("a")).await.expect("c");
        let _t2 = create(&pool, minimal_input("b")).await.expect("c");
        set_status(&pool, t1.id.clone(), "done".into()).await.expect("done");

        let dones = list(&pool, None, Some("done".into()), None)
            .await
            .expect("list");
        assert_eq!(dones.len(), 1);
        assert_eq!(dones[0].status, "done");
    }

    #[tokio::test]
    async fn todo_list_err_invalid_status() {
        let pool = setup().await;
        let err = list(&pool, None, Some("weird".into()), None)
            .await
            .expect_err("invalid");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- 序列化契约 ----------

    #[tokio::test]
    async fn todo_serializes_camel_case() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("create");
        let json = serde_json::to_value(&t).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("spaceId") == false, "spaceId=None 时被 skip");
        assert!(obj.contains_key("createdAt"));
        assert!(obj.contains_key("updatedAt"));
        assert!(obj.contains_key("sortOrder"));
        assert!(!obj.contains_key("created_at"));
        assert!(!obj.contains_key("space_id"));
    }

    #[tokio::test]
    async fn todo_patch_deserializes_double_option() {
        // 外层 None = 字段未传；Some(None) = 显式置空
        let json_no_field = serde_json::json!({ "title": "x" });
        let patch: TodoPatch = serde_json::from_value(json_no_field).expect("parse");
        assert_eq!(patch.space_id, None, "字段未传 → None");

        let json_null = serde_json::json!({ "spaceId": null });
        let patch: TodoPatch = serde_json::from_value(json_null).expect("parse");
        assert_eq!(patch.space_id, Some(None), "显式 null → Some(None)");

        let json_value = serde_json::json!({ "spaceId": "sp_1" });
        let patch: TodoPatch = serde_json::from_value(json_value).expect("parse");
        assert_eq!(patch.space_id, Some(Some("sp_1".into())));
    }

    // ---------- m7-7.3 · todo_today ----------

    #[tokio::test]
    async fn todo_today_returns_only_pending_and_doing() {
        let pool = setup().await;
        let t_pending = create(&pool, minimal_input("pending")).await.expect("c");
        let t_doing = create(&pool, minimal_input("doing")).await.expect("c");
        let t_done = create(&pool, minimal_input("done")).await.expect("c");
        let t_cancelled = create(&pool, minimal_input("cancelled")).await.expect("c");

        set_status(&pool, t_doing.id.clone(), "doing".into()).await.expect("doing");
        set_status(&pool, t_done.id.clone(), "done".into()).await.expect("done");
        set_status(&pool, t_cancelled.id.clone(), "cancelled".into())
            .await
            .expect("cancelled");

        let list = today(&pool).await.expect("today");
        assert_eq!(list.len(), 2, "仅 pending+doing");
        let ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&t_pending.id.as_str()));
        assert!(ids.contains(&t_doing.id.as_str()));
    }

    #[tokio::test]
    async fn todo_today_orders_by_priority_desc_then_due_at() {
        let pool = setup().await;
        // 创建顺序故意打乱，验证排序而非插入顺序
        let low_no_due = create(&pool, TodoCreateInput {
            title: "低优先级无截止".into(),
            note: None,
            space_id: None,
            priority: Some(0),
            due_at: None,
        })
        .await
        .expect("c");
        let high_late_due = create(&pool, TodoCreateInput {
            title: "高优先级晚截止".into(),
            note: None,
            space_id: None,
            priority: Some(2),
            due_at: Some(2_000_000_000),
        })
        .await
        .expect("c");
        let high_early_due = create(&pool, TodoCreateInput {
            title: "高优先级早截止".into(),
            note: None,
            space_id: None,
            priority: Some(2),
            due_at: Some(1_000_000_000),
        })
        .await
        .expect("c");
        let high_no_due = create(&pool, TodoCreateInput {
            title: "高优先级无截止".into(),
            note: None,
            space_id: None,
            priority: Some(2),
            due_at: None,
        })
        .await
        .expect("c");

        let list = today(&pool).await.expect("today");
        assert_eq!(list.len(), 4);
        // 期望顺序：priority=2 内部按 due_at ASC NULLS LAST，再按 created_at ASC
        assert_eq!(list[0].id, high_early_due.id, "priority=2 + 早截止 优先");
        assert_eq!(list[1].id, high_late_due.id, "priority=2 + 晚截止 次之");
        assert_eq!(list[2].id, high_no_due.id, "priority=2 + 无截止 再次");
        assert_eq!(list[3].id, low_no_due.id, "priority=0 排最后");
    }

    #[tokio::test]
    async fn todo_today_same_priority_and_due_falls_back_to_created_at() {
        let pool = setup().await;
        // 同优先级同 due_at（NULL），按 created_at ASC
        let t1 = create(&pool, minimal_input("first")).await.expect("c");
        // 确保 created_at 不同（SQLite 秒级精度，需要手动区分）
        sqlx::query("UPDATE todo SET created_at = created_at + 1 WHERE id = ?")
            .bind(&t1.id)
            .execute(&pool)
            .await
            .expect("bump");
        let t2 = create(&pool, minimal_input("second")).await.expect("c");

        let list = today(&pool).await.expect("today");
        assert_eq!(list.len(), 2);
        // t2.created_at < t1.created_at（t1 被 +1），所以 t2 排前
        assert_eq!(list[0].id, t2.id);
        assert_eq!(list[1].id, t1.id);
    }

    #[tokio::test]
    async fn todo_today_empty_when_no_active() {
        let pool = setup().await;
        let t = create(&pool, minimal_input("x")).await.expect("c");
        set_status(&pool, t.id.clone(), "done".into()).await.expect("done");

        let list = today(&pool).await.expect("today");
        assert!(list.is_empty(), "全部 done 时应返回空");
    }
}
