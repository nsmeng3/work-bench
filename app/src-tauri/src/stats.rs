//! 工作台统计（m7-7.6 · 统计菜单）
//!
//! 单命令 `stats_overview`：聚合各表计数，供统计页一次拉取。
//! 全部为只读 COUNT/GROUP BY 查询，秒级以内。

use serde::Serialize;
use sqlx::sqlite::SqlitePool;

use crate::error::{AppError, CmdResult};

/// 按类型的引用计数条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeCount {
    /// resource_reference.type（code/document/data/artifact/tool/media）
    pub r#type: String,
    pub count: i64,
}

/// 近 7 天访问趋势条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    /// 本地日期，格式 "MM-DD"
    pub date: String,
    pub count: i64,
}

/// `stats_overview` 出参。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    pub space_count: i64,
    pub collection_count: i64,
    pub reference_count: i64,
    pub watch_dir_count: i64,
    /// 引用按类型分布（只统计未删除 disposition != 'deleted'）
    pub refs_by_type: Vec<TypeCount>,
    pub todo_pending: i64,
    pub todo_doing: i64,
    /// 本地今日完成的 todo 数
    pub todo_done_today: i64,
    pub todo_done_total: i64,
    pub inbox_pending: i64,
    pub inbox_snoozed: i64,
    /// 近 7 天资源访问趋势（ref_access_log，含今天，按本地日期升序）
    pub access_last_7d: Vec<DayCount>,
}

async fn count(pool: &SqlitePool, sql: &str) -> CmdResult<i64> {
    let row: (i64,) = sqlx::query_as(sql).fetch_one(pool).await.map_err(AppError::from)?;
    Ok(row.0)
}

/// 本地今日 00:00 的 Unix 秒（拿不到本地时区则退回 UTC 零点）。
fn local_day_start(offset_days_ago: i64) -> i64 {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let midnight = now
        .replace_time(time::Time::MIDNIGHT)
        .unix_timestamp();
    midnight - offset_days_ago * 86400
}

pub async fn overview(pool: &SqlitePool) -> CmdResult<StatsOverview> {
    let space_count = count(pool, "SELECT COUNT(*) FROM space WHERE status = 'active'").await?;
    let collection_count =
        count(pool, "SELECT COUNT(*) FROM collection WHERE status = 'active'").await?;
    let reference_count = count(
        pool,
        "SELECT COUNT(*) FROM resource_reference WHERE disposition != 'deleted'",
    )
    .await?;
    let watch_dir_count = count(pool, "SELECT COUNT(*) FROM watch_dir").await?;

    let refs_by_type: Vec<(String, i64)> = sqlx::query_as(
        "SELECT type, COUNT(*) FROM resource_reference
         WHERE disposition != 'deleted' GROUP BY type ORDER BY COUNT(*) DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)?;
    let refs_by_type = refs_by_type
        .into_iter()
        .map(|(t, c)| TypeCount { r#type: t, count: c })
        .collect();

    let todo_pending = count(pool, "SELECT COUNT(*) FROM todo WHERE status = 'pending'").await?;
    let todo_doing = count(pool, "SELECT COUNT(*) FROM todo WHERE status = 'doing'").await?;
    let todo_done_total = count(pool, "SELECT COUNT(*) FROM todo WHERE status = 'done'").await?;
    let today_start = local_day_start(0);
    let todo_done_today: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM todo WHERE status = 'done' AND done_at >= ?",
    )
    .bind(today_start)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;

    let inbox_pending =
        count(pool, "SELECT COUNT(*) FROM inbox_item WHERE status = 'pending'").await?;
    let inbox_snoozed =
        count(pool, "SELECT COUNT(*) FROM inbox_item WHERE status = 'snoozed'").await?;

    // 近 7 天访问趋势：拉出时间戳在 Rust 侧按本地日期分桶，
    // 避免依赖 SQLite 的时区处理（'localtime' 修饰符对 Unix 秒可用，
    // 但分桶格式在前端展示前还需转换，Rust 侧做更直观）。
    let week_start = local_day_start(6);
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT at FROM ref_access_log WHERE at >= ?")
        .bind(week_start)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

    let mut buckets: [i64; 7] = [0; 7];
    for (at,) in rows {
        let day_idx = (at - week_start).div_euclid(86400);
        if (0..7).contains(&day_idx) {
            buckets[day_idx as usize] += 1;
        }
    }
    let access_last_7d = buckets
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let ts = week_start + i as i64 * 86400;
            let date = time::OffsetDateTime::from_unix_timestamp(ts)
                .map(|dt| {
                    let local = dt
                        .to_offset(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
                    format!("{:02}-{:02}", local.month() as u8, local.day())
                })
                .unwrap_or_else(|_| "?".to_string());
            DayCount { date, count: *c }
        })
        .collect();

    Ok(StatsOverview {
        space_count,
        collection_count,
        reference_count,
        watch_dir_count,
        refs_by_type,
        todo_pending,
        todo_doing,
        todo_done_today: todo_done_today.0,
        todo_done_total,
        inbox_pending,
        inbox_snoozed,
        access_last_7d,
    })
}

#[tauri::command]
pub async fn stats_overview(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<StatsOverview> {
    overview(&state.pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// local_day_start 应为 86400 的整数倍偏移，且 ≤ 当前时间。
    #[test]
    fn day_start_monotonic() {
        let today = local_day_start(0);
        let week_ago = local_day_start(6);
        assert_eq!(today - week_ago, 6 * 86400);
        assert!(today <= time::OffsetDateTime::now_utc().unix_timestamp());
    }
}
