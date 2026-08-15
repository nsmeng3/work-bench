//! 空间管理（详细设计 §2.3）
//!
//! 5 个命令：`space_create` / `space_update` / `space_archive` /
//! `space_restore` / `space_list`。
//!
//! 归档为逻辑归档（改 status 字段），不触碰其下资源集与引用（§6.1）。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};

/// 空间名称最大长度（§3.2 space.name 长度≤64）。
const NAME_MAX_LEN: usize = 64;

/// 空间结构（与 `space` 表一一对应）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub status: String,
    /// Unix 秒
    pub created_at: i64,
    /// Unix 秒
    pub updated_at: i64,
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn validate_name(name: &str) -> CmdResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_param("空间名称不能为空"));
    }
    if trimmed.chars().count() > NAME_MAX_LEN {
        return Err(AppError::invalid_param(format!(
            "空间名称超长（>{} 字符）",
            NAME_MAX_LEN
        )));
    }
    Ok(())
}

fn row_to_space(row: &sqlx::sqlite::SqliteRow) -> Result<Space, sqlx::Error> {
    Ok(Space {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        color: row.try_get("color")?,
        icon: row.try_get("icon")?,
        status: row.try_get("status")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn fetch_space(pool: &SqlitePool, id: &str) -> CmdResult<Space> {
    let row = sqlx::query(
        "SELECT id, name, description, color, icon, status, created_at, updated_at \
         FROM space WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("空间不存在: {}", id)))?;
    row_to_space(&row).map_err(AppError::from)
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

pub async fn create(
    pool: &SqlitePool,
    name: String,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CmdResult<Space> {
    validate_name(&name)?;
    let id = Uuid::new_v4().to_string();
    let now = now_unix();
    sqlx::query(
        "INSERT INTO space (id, name, description, color, icon, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(&id)
    .bind(name.trim())
    .bind(&description)
    .bind(&color)
    .bind(&icon)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    fetch_space(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: String,
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CmdResult<Space> {
    let existing = fetch_space(pool, &id).await?;
    if existing.status == "archived" {
        return Err(AppError::conflict(format!(
            "已归档空间不可编辑: {}",
            id
        )));
    }

    if let Some(ref n) = name {
        validate_name(n)?;
    }

    let new_name = name.as_deref().map(str::trim).unwrap_or(&existing.name);
    let new_description = description.or(existing.description);
    let new_color = color.or(existing.color);
    let new_icon = icon.or(existing.icon);
    let now = now_unix();

    sqlx::query(
        "UPDATE space SET name = ?, description = ?, color = ?, icon = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(new_name)
    .bind(&new_description)
    .bind(&new_color)
    .bind(&new_icon)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    fetch_space(pool, &id).await
}

pub async fn archive(pool: &SqlitePool, id: String) -> CmdResult<Space> {
    let existing = fetch_space(pool, &id).await?;
    if existing.status == "archived" {
        return Err(AppError::conflict(format!("空间已归档: {}", id)));
    }
    let now = now_unix();
    sqlx::query("UPDATE space SET status = 'archived', updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    fetch_space(pool, &id).await
}

pub async fn restore(pool: &SqlitePool, id: String) -> CmdResult<Space> {
    // 未找到 → COMMON_NOT_FOUND；已 active 时幂等返回（契约未要求 CONFLICT）
    let existing = fetch_space(pool, &id).await?;
    if existing.status == "active" {
        return Ok(existing);
    }
    let now = now_unix();
    sqlx::query("UPDATE space SET status = 'active', updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    fetch_space(pool, &id).await
}

pub async fn list(pool: &SqlitePool, status: Option<String>) -> CmdResult<Vec<Space>> {
    let status = status.unwrap_or_else(|| "active".to_string());
    let rows = match status.as_str() {
        "active" | "archived" => sqlx::query(
            "SELECT id, name, description, color, icon, status, created_at, updated_at \
             FROM space WHERE status = ? ORDER BY created_at ASC, id ASC",
        )
        .bind(&status)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?,
        "all" => sqlx::query(
            "SELECT id, name, description, color, icon, status, created_at, updated_at \
             FROM space ORDER BY created_at ASC, id ASC",
        )
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
    rows.iter().map(row_to_space).collect::<Result<_, _>>().map_err(AppError::from)
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
pub async fn space_create(
    state: tauri::State<'_, crate::AppState>,
    name: String,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CmdResult<Space> {
    create(&state.pool, name, description, color, icon).await
}

#[tauri::command]
pub async fn space_update(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
) -> CmdResult<Space> {
    update(&state.pool, id, name, description, color, icon).await
}

#[tauri::command]
pub async fn space_archive(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<Space> {
    archive(&state.pool, id).await
}

#[tauri::command]
pub async fn space_restore(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<Space> {
    restore(&state.pool, id).await
}

#[tauri::command]
pub async fn space_list(
    state: tauri::State<'_, crate::AppState>,
    status: Option<String>,
) -> CmdResult<Vec<Space>> {
    list(&state.pool, status).await
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

    // ---------- space_create ----------

    #[tokio::test]
    async fn space_create_ok_minimal() {
        let pool = setup().await;
        let s = create(&pool, "工作区".into(), None, None, None)
            .await
            .expect("create ok");
        assert_eq!(s.name, "工作区");
        assert_eq!(s.status, "active");
        assert!(!s.id.is_empty());
        assert!(s.created_at > 0);
        assert_eq!(s.created_at, s.updated_at);
    }

    #[tokio::test]
    async fn space_create_ok_full_fields() {
        let pool = setup().await;
        let s = create(
            &pool,
            "研究".into(),
            Some("研究空间".into()),
            Some("#FF0000".into()),
            Some("book".into()),
        )
        .await
        .expect("create ok");
        assert_eq!(s.description.as_deref(), Some("研究空间"));
        assert_eq!(s.color.as_deref(), Some("#FF0000"));
        assert_eq!(s.icon.as_deref(), Some("book"));
    }

    #[tokio::test]
    async fn space_create_err_empty_name() {
        let pool = setup().await;
        let err = create(&pool, "   ".into(), None, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn space_create_err_too_long_name() {
        let pool = setup().await;
        let long = "x".repeat(NAME_MAX_LEN + 1);
        let err = create(&pool, long, None, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- space_update ----------

    #[tokio::test]
    async fn space_update_ok_partial() {
        let pool = setup().await;
        let s = create(&pool, "原名".into(), None, None, None)
            .await
            .expect("create ok");
        let updated = update(
            &pool,
            s.id.clone(),
            Some("新名".into()),
            None,
            Some("#00FF00".into()),
            None,
        )
        .await
        .expect("update ok");
        assert_eq!(updated.name, "新名");
        assert_eq!(updated.color.as_deref(), Some("#00FF00"));
        assert!(updated.updated_at >= updated.created_at);
    }

    #[tokio::test]
    async fn space_update_err_not_found() {
        let pool = setup().await;
        let err = update(&pool, "no-such-id".into(), Some("x".into()), None, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn space_update_err_archived_conflict() {
        let pool = setup().await;
        let s = create(&pool, "a".into(), None, None, None)
            .await
            .expect("create ok");
        archive(&pool, s.id.clone()).await.expect("archive ok");
        let err = update(&pool, s.id.clone(), Some("b".into()), None, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    #[tokio::test]
    async fn space_update_err_invalid_name() {
        let pool = setup().await;
        let s = create(&pool, "a".into(), None, None, None)
            .await
            .expect("create ok");
        let err = update(&pool, s.id.clone(), Some("".into()), None, None, None)
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- space_archive ----------

    #[tokio::test]
    async fn space_archive_ok() {
        let pool = setup().await;
        let s = create(&pool, "a".into(), None, None, None)
            .await
            .expect("create ok");
        let archived = archive(&pool, s.id.clone()).await.expect("archive ok");
        assert_eq!(archived.status, "archived");
    }

    #[tokio::test]
    async fn space_archive_err_not_found() {
        let pool = setup().await;
        let err = archive(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    #[tokio::test]
    async fn space_archive_err_already_archived() {
        let pool = setup().await;
        let s = create(&pool, "a".into(), None, None, None)
            .await
            .expect("create ok");
        archive(&pool, s.id.clone()).await.expect("archive ok");
        let err = archive(&pool, s.id.clone()).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_CONFLICT");
    }

    // ---------- space_restore ----------

    #[tokio::test]
    async fn space_restore_ok() {
        let pool = setup().await;
        let s = create(&pool, "a".into(), None, None, None)
            .await
            .expect("create ok");
        archive(&pool, s.id.clone()).await.expect("archive ok");
        let restored = restore(&pool, s.id.clone()).await.expect("restore ok");
        assert_eq!(restored.status, "active");
    }

    #[tokio::test]
    async fn space_restore_err_not_found() {
        let pool = setup().await;
        let err = restore(&pool, "no-such-id".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- space_list ----------

    #[tokio::test]
    async fn space_list_default_active() {
        let pool = setup().await;
        // 迁移已预置 2 个 active 空间
        let initial = list(&pool, None).await.expect("list ok");
        assert_eq!(initial.len(), 2);

        let s = create(&pool, "extra".into(), None, None, None)
            .await
            .expect("create ok");
        archive(&pool, s.id.clone()).await.expect("archive ok");

        let active = list(&pool, None).await.expect("list ok");
        assert_eq!(active.len(), 2, "default filter is active");
        assert!(active.iter().all(|x| x.status == "active"));
    }

    #[tokio::test]
    async fn space_list_archived_and_all() {
        let pool = setup().await;
        let s = create(&pool, "to-archive".into(), None, None, None)
            .await
            .expect("create ok");
        archive(&pool, s.id.clone()).await.expect("archive ok");

        let archived = list(&pool, Some("archived".into())).await.expect("list ok");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, s.id);

        let all = list(&pool, Some("all".into())).await.expect("list ok");
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn space_list_err_invalid_status() {
        let pool = setup().await;
        let err = list(&pool, Some("weird".into()))
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- 预置空间 ----------

    #[tokio::test]
    async fn preset_spaces_seeded() {
        let pool = setup().await;
        let all = list(&pool, Some("all".into())).await.expect("list ok");
        let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"工作"));
        assert!(names.contains(&"生活"));
        let work = all.iter().find(|s| s.name == "工作").unwrap();
        assert_eq!(work.id, "preset_space_work");
    }
}
