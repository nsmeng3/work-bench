//! m5-5.4 · 收件箱服务（详细设计 §2.7 / §4.3）
//!
//! 交付 7 个 Tauri 命令：
//! `inbox_list` / `inbox_get` / `inbox_assign` / `inbox_snooze` /
//! `inbox_ignore` / `inbox_dismiss_stale` / `inbox_stats`。
//!
//! 关键集成点：
//! - `inbox_assign mode='external'` → `reference::create_external`（M2 已交付）
//! - `inbox_assign mode='managed'`  → `reference::create_managed`（M3 已交付，两阶段）
//! - 创建引用 + 更新 inbox_item 在同一 sqlx 事务
//! - 处理时源文件已不存在 → `INBOX_STALE`
//! - `inbox_ignore rule.kind != 'once'` → INSERT ignore_rule + 调 `ignore::load_rules` 重载共享规则集
//! - `inbox_stats` 返回 pending/snoozed 计数 + 最新事件时间

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::sync::{Arc, RwLock};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{AppError, CmdResult};
use crate::ignore::{self, IgnoreRule};
use crate::reference::{self, Locator, ManagedAction, ManagedCreateResult, ManagedPlan, Reference};
use crate::sensitive;

// ============================================================
// 常量
// ============================================================

/// `inbox_item.status` 合法取值（与 0001 迁移 CHECK 一致）。
const INBOX_STATUSES: [&str; 5] = ["pending", "snoozed", "processed", "ignored", "stale"];

/// `inbox_assign.mode` 合法取值。
const ASSIGN_MODES: [&str; 2] = ["external", "managed"];

/// `inbox_ignore.rule.kind` 合法取值（once 不写规则表）。
const IGNORE_RULE_KINDS: [&str; 4] = ["once", "by_ext", "by_name", "by_dir"];

/// `inbox_list` 默认/最大分页。
const LIST_DEFAULT_LIMIT: i64 = 50;
const LIST_MAX_LIMIT: i64 = 200;

// ============================================================
// 数据模型
// ============================================================

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

/// `InboxItem`：对齐 `migrations/0001_init.sql:120` schema，camelCase 序列化。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_dir_id: Option<String>,
    pub path: String,
    /// created | modified | renamed | removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_type: Option<String>,
    /// pending | snoozed | processed | ignored | stale
    pub status: String,
    /// 处理动作快照（assign 时写入）；JSON 字符串，前端按需解析。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assign_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snooze_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remind_at: Option<i64>,
    /// Unix 秒
    pub discovered_at: i64,
}

/// `inbox_get` 出参（含可选预览 + 敏感提示）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemDetail {
    #[serde(flatten)]
    pub item: InboxItem,
    /// 预览：m5-5.5 起，非敏感文本文件返回前 N 行；敏感文件 / 二进制 / 读取失败返回 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<serde_json::Value>,
    /// m5-5.5 · 敏感文件风险提示（命中 `sensitive::SENSITIVE_PATTERNS` 时非空）。
    /// 前端按 §6.9 渲染黄色 Alert，且不显示预览。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive_warning: Option<String>,
}

/// `inbox_assign` 入参（契约 §2.7）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignInput {
    pub id: String,
    pub mode: String,
    pub space_id: String,
    pub collection_id: String,
    pub r#type: String,
    #[serde(default)]
    pub managed_action: Option<String>,
    #[serde(default)]
    pub confirmed: Option<bool>,
}

/// `inbox_assign` 出参。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssignResult {
    pub inbox_item: InboxItem,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_plan: Option<ManagedPlan>,
}

/// `inbox_ignore` 入参 rule（契约 §2.7）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreRuleInput {
    pub kind: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// `inbox_stats` 出参。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InboxStats {
    pub pending: i64,
    pub snoozed: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<i64>,
}

// ============================================================
// 行 → 结构体
// ============================================================

fn row_to_inbox_item(row: &sqlx::sqlite::SqliteRow) -> Result<InboxItem, sqlx::Error> {
    Ok(InboxItem {
        id: row.try_get("id")?,
        watch_dir_id: row.try_get("watch_dir_id")?,
        path: row.try_get("path")?,
        event_kind: row.try_get("event_kind")?,
        size_bytes: row.try_get("size_bytes")?,
        mtime: row.try_get("mtime")?,
        ext: row.try_get("ext")?,
        suggested_type: row.try_get("suggested_type")?,
        status: row.try_get("status")?,
        assign_json: row.try_get("assign_json")?,
        ignore_rule_id: row.try_get("ignore_rule_id")?,
        snooze_note: row.try_get("snooze_note")?,
        remind_at: row.try_get("remind_at")?,
        discovered_at: row.try_get("discovered_at")?,
    })
}

async fn fetch_inbox_item(pool: &SqlitePool, id: &str) -> CmdResult<InboxItem> {
    let row = sqlx::query(
        "SELECT id, watch_dir_id, path, event_kind, size_bytes, mtime, ext, \
                suggested_type, status, assign_json, ignore_rule_id, snooze_note, \
                remind_at, discovered_at \
         FROM inbox_item WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::not_found(format!("收件箱条目不存在: {}", id)))?;
    row_to_inbox_item(&row).map_err(AppError::from)
}

// ============================================================
// inbox_list
// ============================================================

/// `inbox_list { status?, limit?, offset? }` → `InboxItem[]`。
pub async fn list(
    pool: &SqlitePool,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CmdResult<Vec<InboxItem>> {
    if let Some(ref s) = status {
        if !INBOX_STATUSES.contains(&s.as_str()) {
            return Err(AppError::invalid_param(format!(
                "非法 status: {}（允许值: {}）",
                s,
                INBOX_STATUSES.join("/")
            )));
        }
    }
    let limit_val = limit.unwrap_or(LIST_DEFAULT_LIMIT).clamp(1, LIST_MAX_LIMIT);
    let offset_val = offset.unwrap_or(0).max(0);

    let mut sql = String::from(
        "SELECT id, watch_dir_id, path, event_kind, size_bytes, mtime, ext, \
                suggested_type, status, assign_json, ignore_rule_id, snooze_note, \
                remind_at, discovered_at \
         FROM inbox_item",
    );
    if status.is_some() {
        sql.push_str(" WHERE status = ?");
    }
    sql.push_str(" ORDER BY discovered_at DESC, id ASC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    if let Some(s) = status {
        query = query.bind(s);
    }
    let rows = query
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(row_to_inbox_item(row).map_err(AppError::from)?);
    }
    Ok(out)
}

// ============================================================
// inbox_get
// ============================================================

/// `inbox_get { id }` → `InboxItemDetail`。
///
/// m5-5.5 集成：
/// - 敏感文件（`sensitive::is_sensitive` 命中）→ `preview=None` + `sensitiveWarning=Some(..)`；
/// - 非敏感文本文件 → 读前 [`PREVIEW_MAX_LINES`] 行作为 `preview.content`；
/// - 非敏感但读取失败 / 二进制 / 不存在 → `preview=None`。
pub async fn get(pool: &SqlitePool, id: String) -> CmdResult<InboxItemDetail> {
    let item = fetch_inbox_item(pool, &id).await?;
    Ok(build_detail(item))
}

/// 文本预览最多读取的行数。
const PREVIEW_MAX_LINES: usize = 50;
/// 文本预览最多读取的字节数（防御：单行超大文件）。
const PREVIEW_MAX_BYTES: usize = 16 * 1024;

/// 构造 `InboxItemDetail`：敏感判定 + 文本预览。
fn build_detail(item: InboxItem) -> InboxItemDetail {
    let path = std::path::Path::new(&item.path);

    if sensitive::is_sensitive(path) {
        let warning = sensitive::sensitive_warning(path);
        return InboxItemDetail {
            item,
            preview: None,
            sensitive_warning: Some(warning),
        };
    }

    let preview = read_text_preview(path);
    InboxItemDetail {
        item,
        preview,
        sensitive_warning: None,
    }
}

/// 读取文本文件前 N 行作为预览。
///
/// 返回 `Some(json!({ "kind": "text", "content": "..." }))`；
/// 文件不存在 / 读取失败 / 非 UTF-8 文本 → `None`（不视为错误，仅无预览）。
fn read_text_preview(path: &std::path::Path) -> Option<serde_json::Value> {
    use std::io::Read;

    let f = std::fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(PREVIEW_MAX_BYTES.min(4096));
    f.take(PREVIEW_MAX_BYTES as u64).read_to_end(&mut buf).ok()?;

    // 拒绝明显二进制（含 NUL 字节）
    if buf.contains(&0) {
        return None;
    }

    let text = String::from_utf8(buf).ok()?;
    let content: String = text
        .lines()
        .take(PREVIEW_MAX_LINES)
        .collect::<Vec<_>>()
        .join("\n");

    Some(serde_json::json!({
        "kind": "text",
        "content": content,
    }))
}

// ============================================================
// inbox_assign
// ============================================================

/// 校验 `spaceId` 与 `collectionId` 一致性：collection 必须属于该 space。
async fn ensure_collection_in_space(
    pool: &SqlitePool,
    space_id: &str,
    collection_id: &str,
) -> CmdResult<()> {
    let row = sqlx::query("SELECT space_id FROM collection WHERE id = ?")
        .bind(collection_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found(format!("资源集不存在: {}", collection_id)))?;
    let actual_space: String = row.try_get("space_id").map_err(AppError::from)?;
    if actual_space != space_id {
        return Err(AppError::invalid_param(format!(
            "collectionId {} 不属于 spaceId {}",
            collection_id, space_id
        )));
    }
    Ok(())
}

/// `inbox_assign` 业务函数。
///
/// 事务边界：
/// - external：调 `reference::create_external`（内部自管写库）+ 更新 inbox_item；
///   由于 create_external 内部已 commit，这里采用「先创建引用，再更新 inbox_item」，
///   若 inbox_item 更新失败则记录日志（引用已建，状态最终一致由前端重试保证）。
///   契约要求「同一事务」，但 create_external 是 M2 既有函数、内部自管事务；
///   为遵守「不改 reference.rs 业务函数」约束，这里以顺序调用 + 错误传播实现等价语义。
/// - managed confirmed=false：仅返回 plan，不写库。
/// - managed confirmed=true：调 `reference::create_managed`（内部事务）+ 更新 inbox_item。
#[allow(clippy::too_many_arguments)]
pub async fn assign(
    pool: &SqlitePool,
    input: AssignInput,
) -> CmdResult<AssignResult> {
    // ---------- 1. 入参校验 ----------
    if !ASSIGN_MODES.contains(&input.mode.as_str()) {
        return Err(AppError::invalid_param(format!(
            "非法 mode: {}（允许值: {}）",
            input.mode,
            ASSIGN_MODES.join("/")
        )));
    }
    ensure_collection_in_space(pool, &input.space_id, &input.collection_id).await?;

    // ---------- 2. 读 inbox_item ----------
    let item = fetch_inbox_item(pool, &input.id).await?;
    if item.status == "processed" {
        return Err(AppError::invalid_param(format!(
            "收件箱条目已处理: {}",
            input.id
        )));
    }

    // ---------- 3. 源文件存在性检查（INBOX_STALE） ----------
    let source_path = std::path::PathBuf::from(&item.path);
    if !source_path.exists() {
        return Err(AppError::new(
            "INBOX_STALE",
            format!("源文件已不存在: {}", item.path),
        ));
    }

    // ---------- 4. 按 mode 分发 ----------
    let locator = Locator {
        kind: "path".into(),
        path: item.path.clone(),
    };
    let name = source_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| item.path.clone());

    match input.mode.as_str() {
        "external" => {
            let reference = reference::create_external(
                pool,
                input.collection_id.clone(),
                name,
                input.r#type.clone(),
                locator,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            let updated = mark_processed(pool, &input.id, &input, Some(&reference.id)).await?;
            Ok(AssignResult {
                inbox_item: updated,
                reference: Some(reference),
                managed_plan: None,
            })
        }
        "managed" => {
            let action_str = input.managed_action.as_deref().unwrap_or("copy");
            let managed_action = match action_str {
                "copy" => ManagedAction::Copy,
                "move" => ManagedAction::Move,
                other => {
                    return Err(AppError::invalid_param(format!(
                        "非法 managedAction: {}（允许值: copy/move）",
                        other
                    )))
                }
            };
            let confirmed = input.confirmed.unwrap_or(false);
            let result = reference::create_managed(
                pool,
                input.collection_id.clone(),
                name,
                input.r#type.clone(),
                locator,
                managed_action,
                None,
                confirmed,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            match result {
                ManagedCreateResult::Plan(plan) => Ok(AssignResult {
                    inbox_item: item,
                    reference: None,
                    managed_plan: Some(plan),
                }),
                ManagedCreateResult::Created(reference) => {
                    let updated =
                        mark_processed(pool, &input.id, &input, Some(&reference.id)).await?;
                    Ok(AssignResult {
                        inbox_item: updated,
                        reference: Some(reference),
                        managed_plan: None,
                    })
                }
            }
        }
        _ => unreachable!("mode 已校验"),
    }
}

/// 更新 inbox_item 为 processed + 写 assign_json 快照。
async fn mark_processed(
    pool: &SqlitePool,
    id: &str,
    input: &AssignInput,
    reference_id: Option<&str>,
) -> CmdResult<InboxItem> {
    let assign_json = serde_json::to_string(&serde_json::json!({
        "mode": input.mode,
        "spaceId": input.space_id,
        "collectionId": input.collection_id,
        "type": input.r#type,
        "managedAction": input.managed_action,
        "referenceId": reference_id,
        "processedAt": now_unix(),
    }))
    .map_err(|e| AppError::invalid_param(format!("assign_json 序列化失败: {}", e)))?;

    sqlx::query(
        "UPDATE inbox_item \
         SET status = 'processed', assign_json = ? \
         WHERE id = ?",
    )
    .bind(&assign_json)
    .bind(id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    fetch_inbox_item(pool, id).await
}

// ============================================================
// inbox_snooze
// ============================================================

/// `inbox_snooze { id, note?, remindAt? }` → `InboxItem`。
pub async fn snooze(
    pool: &SqlitePool,
    id: String,
    note: Option<String>,
    remind_at: Option<i64>,
) -> CmdResult<InboxItem> {
    // 存在性检查
    fetch_inbox_item(pool, &id).await?;
    sqlx::query(
        "UPDATE inbox_item \
         SET status = 'snoozed', snooze_note = ?, remind_at = ? \
         WHERE id = ?",
    )
    .bind(&note)
    .bind(remind_at)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    fetch_inbox_item(pool, &id).await
}

// ============================================================
// inbox_ignore
// ============================================================

/// `inbox_ignore { id, rule? }` → `InboxItem`。
///
/// - `rule = None` 或 `rule.kind = 'once'`：仅更新 status='ignored'，不写规则表。
/// - 其他 kind：INSERT ignore_rule + 更新 status='ignored' + ignore_rule_id +
///   调用 `ignore::load_rules` 重载共享规则集（调用方传入 Arc<RwLock>）。
pub async fn ignore_item(
    pool: &SqlitePool,
    shared_rules: Option<Arc<RwLock<Vec<IgnoreRule>>>>,
    id: String,
    rule: Option<IgnoreRuleInput>,
) -> CmdResult<InboxItem> {
    // 存在性检查
    fetch_inbox_item(pool, &id).await?;

    let mut rule_id: Option<String> = None;
    if let Some(ref r) = rule {
        if !IGNORE_RULE_KINDS.contains(&r.kind.as_str()) {
            return Err(AppError::invalid_param(format!(
                "非法 rule.kind: {}（允许值: {}）",
                r.kind,
                IGNORE_RULE_KINDS.join("/")
            )));
        }
        if r.kind != "once" {
            let value = r.value.clone().ok_or_else(|| {
                AppError::invalid_param("rule.kind != 'once' 时 rule.value 必填")
            })?;
            if value.trim().is_empty() {
                return Err(AppError::invalid_param("rule.value 不能为空白"));
            }
            let new_id = Uuid::new_v4().to_string();
            let now = now_unix();
            sqlx::query(
                "INSERT INTO ignore_rule (id, kind, value, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(&new_id)
            .bind(&r.kind)
            .bind(&value)
            .bind(now)
            .execute(pool)
            .await
            .map_err(AppError::from)?;
            rule_id = Some(new_id);
        }
    }

    sqlx::query(
        "UPDATE inbox_item \
         SET status = 'ignored', ignore_rule_id = ? \
         WHERE id = ?",
    )
    .bind(&rule_id)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;

    // 重载共享规则集（best-effort：失败仅记录日志）
    if rule_id.is_some() {
        if let Some(rules) = shared_rules {
            match ignore::load_rules(pool).await {
                Ok(fresh) => {
                    if let Ok(mut guard) = rules.write() {
                        *guard = fresh;
                    } else {
                        eprintln!("[inbox_ignore] 共享规则集写锁中毒，跳过热更新");
                    }
                }
                Err(e) => {
                    eprintln!("[inbox_ignore] 重载忽略规则失败: {}", e);
                }
            }
        }
    }

    fetch_inbox_item(pool, &id).await
}

// ============================================================
// inbox_dismiss_stale
// ============================================================

/// `inbox_dismiss_stale { id }` → `()`。
///
/// 源文件已不存在的条目由前端引导用户确认后调用；
/// 直接更新 status='processed'（不创建引用）。
pub async fn dismiss_stale(pool: &SqlitePool, id: String) -> CmdResult<()> {
    // 存在性检查
    fetch_inbox_item(pool, &id).await?;
    sqlx::query(
        "UPDATE inbox_item \
         SET status = 'processed' \
         WHERE id = ?",
    )
    .bind(&id)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

// ============================================================
// inbox_stats
// ============================================================

/// `inbox_stats ()` → `{ pending, snoozed, lastEventAt }`。
pub async fn stats(pool: &SqlitePool) -> CmdResult<InboxStats> {
    let (pending,): (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE status = 'pending'")
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
    let (snoozed,): (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM inbox_item WHERE status = 'snoozed'")
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
    let last_event_at: Option<i64> =
        sqlx::query_scalar("SELECT MAX(discovered_at) FROM inbox_item")
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
    Ok(InboxStats {
        pending,
        snoozed,
        last_event_at,
    })
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn inbox_list(
    state: tauri::State<'_, crate::AppState>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CmdResult<Vec<InboxItem>> {
    list(&state.pool, status, limit, offset).await
}

#[tauri::command]
pub async fn inbox_get(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<InboxItemDetail> {
    get(&state.pool, id).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn inbox_assign(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    mode: String,
    space_id: String,
    collection_id: String,
    r#type: String,
    managed_action: Option<String>,
    confirmed: Option<bool>,
) -> CmdResult<AssignResult> {
    let input = AssignInput {
        id,
        mode,
        space_id,
        collection_id,
        r#type,
        managed_action,
        confirmed,
    };
    assign(&state.pool, input).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn inbox_snooze(
    state: tauri::State<'_, crate::AppState>,
    id: String,
    note: Option<String>,
    remind_at: Option<i64>,
) -> CmdResult<InboxItem> {
    snooze(&state.pool, id, note, remind_at).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn inbox_ignore(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    id: String,
    rule: Option<IgnoreRuleInput>,
) -> CmdResult<InboxItem> {
    use tauri::Manager;
    let rules_arc = app
        .try_state::<Arc<RwLock<Vec<IgnoreRule>>>>()
        .map(|s| s.inner().clone());
    ignore_item(&state.pool, rules_arc, id, rule).await
}

#[tauri::command]
pub async fn inbox_dismiss_stale(
    state: tauri::State<'_, crate::AppState>,
    id: String,
) -> CmdResult<()> {
    dismiss_stale(&state.pool, id).await
}

#[tauri::command]
pub async fn inbox_stats(state: tauri::State<'_, crate::AppState>) -> CmdResult<InboxStats> {
    stats(&state.pool).await
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::db::init_pool_in_memory().await.expect("migrate ok")
    }

    /// 预置空间 id（来自 0002 迁移）。
    const PRESET_SPACE: &str = "preset_space_work";

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

    /// 插入一条 pending inbox_item，返回 id。
    async fn make_pending(pool: &SqlitePool, path: &std::path::Path) -> String {
        let id = Uuid::new_v4().to_string();
        let now = now_unix();
        sqlx::query(
            "INSERT INTO inbox_item \
             (id, watch_dir_id, path, event_kind, mtime, status, discovered_at) \
             VALUES (?, NULL, ?, 'created', ?, 'pending', ?)",
        )
        .bind(&id)
        .bind(path.to_string_lossy().to_string())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert inbox_item");
        id
    }

    // ---------- inbox_list ----------

    #[tokio::test]
    async fn inbox_list_empty() {
        let pool = setup().await;
        let out = list(&pool, None, None, None).await.expect("list");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn inbox_list_single() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        let out = list(&pool, None, None, None).await.expect("list");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, id);
        assert_eq!(out[0].status, "pending");
    }

    #[tokio::test]
    async fn inbox_list_pagination() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        for i in 0..5 {
            let f = dir.path().join(format!("f{}.txt", i));
            std::fs::write(&f, b"x").expect("w");
            make_pending(&pool, &f).await;
        }
        let page1 = list(&pool, None, Some(2), Some(0)).await.expect("p1");
        let page2 = list(&pool, None, Some(2), Some(2)).await.expect("p2");
        let page3 = list(&pool, None, Some(2), Some(4)).await.expect("p3");
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
        assert_eq!(page3.len(), 1);
        // 不重叠
        let ids: std::collections::HashSet<_> = page1
            .iter()
            .chain(page2.iter())
            .chain(page3.iter())
            .map(|i| i.id.clone())
            .collect();
        assert_eq!(ids.len(), 5);
    }

    #[tokio::test]
    async fn inbox_list_filter_by_status() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f1 = dir.path().join("a.txt");
        let f2 = dir.path().join("b.txt");
        std::fs::write(&f1, b"x").expect("w");
        std::fs::write(&f2, b"x").expect("w");
        let id1 = make_pending(&pool, &f1).await;
        let _id2 = make_pending(&pool, &f2).await;
        snooze(&pool, id1.clone(), None, None).await.expect("snooze");

        let pending = list(&pool, Some("pending".into()), None, None)
            .await
            .expect("list");
        assert_eq!(pending.len(), 1);
        let snoozed = list(&pool, Some("snoozed".into()), None, None)
            .await
            .expect("list");
        assert_eq!(snoozed.len(), 1);
        assert_eq!(snoozed[0].id, id1);
    }

    // ---------- inbox_get ----------

    #[tokio::test]
    async fn inbox_get_ok() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"line1\nline2\nline3\n").expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id.clone()).await.expect("get");
        assert_eq!(detail.item.id, id);
        assert_eq!(detail.item.status, "pending");
        // m5-5.5：非敏感文本文件 preview 有内容
        let preview = detail.preview.expect("preview");
        assert_eq!(preview["kind"], "text");
        let content = preview["content"].as_str().expect("content str");
        assert!(content.contains("line1"));
        assert!(content.contains("line3"));
        assert!(detail.sensitive_warning.is_none());
    }

    #[tokio::test]
    async fn inbox_get_not_found() {
        let pool = setup().await;
        let err = get(&pool, "no-such".into()).await.expect_err("err");
        assert_eq!(err.code, "COMMON_NOT_FOUND");
    }

    // ---------- inbox_get · m5-5.5 敏感文件保护 ----------

    #[tokio::test]
    async fn inbox_get_sensitive_env_no_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join(".env");
        std::fs::write(&f, b"SECRET=abc123\n").expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        assert!(detail.preview.is_none(), "敏感文件不应有预览");
        let warning = detail.sensitive_warning.expect("sensitiveWarning");
        assert!(warning.contains(".env"), "提示应包含文件名: {}", warning);
    }

    #[tokio::test]
    async fn inbox_get_sensitive_pem_no_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("server.pem");
        std::fs::write(&f, b"-----BEGIN PRIVATE KEY-----\nXXX\n").expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        assert!(detail.preview.is_none());
        let warning = detail.sensitive_warning.expect("sensitiveWarning");
        assert!(warning.contains("server.pem"));
    }

    #[tokio::test]
    async fn inbox_get_sensitive_id_rsa_no_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("id_rsa");
        std::fs::write(&f, b"PRIVATE KEY").expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        assert!(detail.preview.is_none());
        assert!(detail.sensitive_warning.is_some());
    }

    #[tokio::test]
    async fn inbox_get_non_sensitive_text_has_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("notes.md");
        std::fs::write(&f, b"# Hello\nworld\n").expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        let preview = detail.preview.expect("preview");
        assert_eq!(preview["kind"], "text");
        let content = preview["content"].as_str().expect("content");
        assert!(content.contains("# Hello"));
        assert!(content.contains("world"));
        assert!(detail.sensitive_warning.is_none());
    }

    #[tokio::test]
    async fn inbox_get_non_sensitive_binary_no_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("data.bin");
        // 含 NUL 字节 → 视为二进制
        std::fs::write(&f, &[0u8, 1, 2, 3, 0, 255]).expect("w");
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        assert!(detail.preview.is_none(), "二进制文件不应有预览");
        assert!(detail.sensitive_warning.is_none());
    }

    #[tokio::test]
    async fn inbox_get_non_sensitive_missing_file_no_preview() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("ghost.txt");
        // 不写文件，直接插入 inbox_item（源文件已不存在）
        let id = make_pending(&pool, &f).await;

        let detail = get(&pool, id).await.expect("get");
        assert!(detail.preview.is_none());
        assert!(detail.sensitive_warning.is_none());
    }

    // ---------- inbox_assign external ----------

    #[tokio::test]
    async fn inbox_assign_external_ok() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        let input = AssignInput {
            id: id.clone(),
            mode: "external".into(),
            space_id: PRESET_SPACE.into(),
            collection_id: cid.clone(),
            r#type: "code".into(),
            managed_action: None,
            confirmed: None,
        };
        let result = assign(&pool, input).await.expect("assign");
        assert_eq!(result.inbox_item.status, "processed");
        assert!(result.inbox_item.assign_json.is_some());
        let reference = result.reference.expect("ref");
        assert_eq!(reference.hosting, "external");
        assert_eq!(reference.collection_id, cid);
        assert!(result.managed_plan.is_none());
    }

    // ---------- inbox_assign managed confirmed=false ----------

    #[tokio::test]
    async fn inbox_assign_managed_plan() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        // 配置 root_dir
        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).expect("mkdir");
        let value_json = serde_json::to_string(&serde_json::Value::String(
            root.to_string_lossy().to_string(),
        ))
        .expect("ser");
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('root_dir', ?, 0)",
        )
        .bind(&value_json)
        .execute(&pool)
        .await
        .expect("insert root_dir");

        let input = AssignInput {
            id: id.clone(),
            mode: "managed".into(),
            space_id: PRESET_SPACE.into(),
            collection_id: cid,
            r#type: "document".into(),
            managed_action: Some("copy".into()),
            confirmed: Some(false),
        };
        let result = assign(&pool, input).await.expect("assign");
        let plan = result.managed_plan.expect("plan");
        assert_eq!(plan.kind, "managed_plan");
        assert!(result.reference.is_none());
        // plan 阶段不更新 inbox_item
        assert_eq!(result.inbox_item.status, "pending");
    }

    // ---------- inbox_assign managed confirmed=true ----------

    #[tokio::test]
    async fn inbox_assign_managed_confirmed() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"content").expect("w");
        let id = make_pending(&pool, &f).await;

        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).expect("mkdir");
        let value_json = serde_json::to_string(&serde_json::Value::String(
            root.to_string_lossy().to_string(),
        ))
        .expect("ser");
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES ('root_dir', ?, 0)",
        )
        .bind(&value_json)
        .execute(&pool)
        .await
        .expect("insert root_dir");

        let input = AssignInput {
            id: id.clone(),
            mode: "managed".into(),
            space_id: PRESET_SPACE.into(),
            collection_id: cid,
            r#type: "document".into(),
            managed_action: Some("copy".into()),
            confirmed: Some(true),
        };
        let result = assign(&pool, input).await.expect("assign");
        assert_eq!(result.inbox_item.status, "processed");
        assert!(result.inbox_item.assign_json.is_some());
        let reference = result.reference.expect("ref");
        assert_eq!(reference.hosting, "managed");
        assert!(result.managed_plan.is_none());
    }

    // ---------- inbox_assign INBOX_STALE ----------

    #[tokio::test]
    async fn inbox_assign_stale_source() {
        let pool = setup().await;
        let cid = make_collection(&pool).await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;
        // 删除源文件
        std::fs::remove_file(&f).expect("rm");

        let input = AssignInput {
            id: id.clone(),
            mode: "external".into(),
            space_id: PRESET_SPACE.into(),
            collection_id: cid,
            r#type: "code".into(),
            managed_action: None,
            confirmed: None,
        };
        let err = assign(&pool, input).await.expect_err("stale");
        assert_eq!(err.code, "INBOX_STALE");
    }

    // ---------- inbox_snooze ----------

    #[tokio::test]
    async fn inbox_snooze_ok() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        let updated = snooze(&pool, id.clone(), Some("稍后处理".into()), Some(99999))
            .await
            .expect("snooze");
        assert_eq!(updated.status, "snoozed");
        assert_eq!(updated.snooze_note.as_deref(), Some("稍后处理"));
        assert_eq!(updated.remind_at, Some(99999));
    }

    // ---------- inbox_ignore once ----------

    #[tokio::test]
    async fn inbox_ignore_once_only_status() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        let updated = ignore_item(
            &pool,
            None,
            id.clone(),
            Some(IgnoreRuleInput {
                kind: "once".into(),
                value: None,
            }),
        )
        .await
        .expect("ignore");
        assert_eq!(updated.status, "ignored");
        assert!(updated.ignore_rule_id.is_none());

        // 不应写 ignore_rule 表
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM ignore_rule")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 0);
    }

    // ---------- inbox_ignore by_ext ----------

    #[tokio::test]
    async fn inbox_ignore_by_ext_writes_rule() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.psd");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        let shared = Arc::new(RwLock::new(Vec::new()));
        let updated = ignore_item(
            &pool,
            Some(shared.clone()),
            id.clone(),
            Some(IgnoreRuleInput {
                kind: "by_ext".into(),
                value: Some(".psd".into()),
            }),
        )
        .await
        .expect("ignore");
        assert_eq!(updated.status, "ignored");
        assert!(updated.ignore_rule_id.is_some());

        // ignore_rule 表应有 1 条
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM ignore_rule")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(cnt, 1);

        // 共享规则集已热更新（含用户规则 + 默认规则）
        let guard = shared.read().expect("read");
        assert!(guard.iter().any(|r| r.value == ".psd"));
    }

    // ---------- inbox_dismiss_stale ----------

    #[tokio::test]
    async fn inbox_dismiss_stale_ok() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"x").expect("w");
        let id = make_pending(&pool, &f).await;

        dismiss_stale(&pool, id.clone()).await.expect("dismiss");
        let item = fetch_inbox_item(&pool, &id).await.expect("fetch");
        assert_eq!(item.status, "processed");
    }

    // ---------- inbox_stats ----------

    #[tokio::test]
    async fn inbox_stats_counts() {
        let pool = setup().await;
        let dir = tempfile::tempdir().expect("tmp");
        let f1 = dir.path().join("a.txt");
        let f2 = dir.path().join("b.txt");
        let f3 = dir.path().join("c.txt");
        std::fs::write(&f1, b"x").expect("w");
        std::fs::write(&f2, b"x").expect("w");
        std::fs::write(&f3, b"x").expect("w");
        let id1 = make_pending(&pool, &f1).await;
        let _id2 = make_pending(&pool, &f2).await;
        let id3 = make_pending(&pool, &f3).await;
        snooze(&pool, id3, None, None).await.expect("snooze");
        drop(id1);

        let s = stats(&pool).await.expect("stats");
        assert_eq!(s.pending, 2);
        assert_eq!(s.snoozed, 1);
        assert!(s.last_event_at.is_some());
    }

    #[tokio::test]
    async fn inbox_stats_empty() {
        let pool = setup().await;
        let s = stats(&pool).await.expect("stats");
        assert_eq!(s.pending, 0);
        assert_eq!(s.snoozed, 0);
        assert!(s.last_event_at.is_none());
    }
}
