//! 数据库接入与迁移框架（详细设计 §3.3）
//!
//! - 使用 sqlx SQLite，启动时自动执行未应用的迁移。
//! - 数据库文件位置遵循 Tauri app data 目录约定：
//!   `{平台用户数据目录}/resource-workbench/workbench.db`。
//! - 迁移文件按 `0001_init.sql`、`0002_*.sql` 递增，只增不改。

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

use crate::error::{AppError, CmdResult};

/// 嵌入的迁移集合（编译期打包 `migrations/` 目录）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// 数据库文件名（位于 app data 目录下）。
pub const DB_FILE_NAME: &str = "workbench.db";

/// 应用数据目录的子目录名。
pub const APP_DATA_DIR: &str = "resource-workbench";

/// 在给定目录下打开（必要时创建）SQLite 连接池并执行迁移。
///
/// 单元测试与生产启动共用此入口，仅目录不同。
pub async fn init_pool_at(dir: &Path) -> CmdResult<SqlitePool> {
    std::fs::create_dir_all(dir)
        .map_err(|e| AppError::io(format!("创建数据目录失败 {}: {}", dir.display(), e)))?;

    let db_path: PathBuf = dir.join(DB_FILE_NAME);
    init_pool_with_file(&db_path).await
}

/// 直接给定数据库文件路径，初始化连接池并执行迁移。
pub async fn init_pool_with_file(db_path: &Path) -> CmdResult<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::io(format!("创建数据目录失败 {}: {}", parent.display(), e))
        })?;
    }

    let url = format!("sqlite://{}", db_path.display());
    let opts: SqliteConnectOptions = url
        .parse::<SqliteConnectOptions>()
        .map_err(|e| AppError::db(format!("数据库 URL 解析失败: {}", e)))?
        .create_if_missing(true)
        .foreign_keys(true)
        // 关闭 sqlx 默认日志噪音
        .disable_statement_logging();

    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(|e| AppError::db(format!("打开数据库失败 {}: {}", db_path.display(), e)))?;

    MIGRATOR
        .run(&pool)
        .await
        .map_err(AppError::from)?;

    Ok(pool)
}

/// 内存数据库，仅用于单元测试。
#[cfg(test)]
pub async fn init_pool_in_memory() -> CmdResult<SqlitePool> {
    // SQLite 内存库每个连接独立；max_connections=1 保证迁移与查询看到同一库。
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| AppError::db(format!("内存库 URL 解析失败: {}", e)))?
        .create_if_missing(true)
        .foreign_keys(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| AppError::db(format!("打开内存库失败: {}", e)))?;

    MIGRATOR.run(&pool).await.map_err(AppError::from)?;

    Ok(pool)
}

#[cfg(test)]
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_on_memory_db() {
        let pool = init_pool_in_memory().await.expect("migrate ok");

        // 10 张表全部建出
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .expect("list tables");
        let names: Vec<String> = tables.into_iter().map(|(n,)| n).collect();
        let expected = vec![
            "collection",
            "collection_tag",
            "disposition_audit",
            "ignore_rule",
            "inbox_item",
            "reference_tag",
            "resource_reference",
            "settings",
            "space",
            "storage_source",
            "watch_dir",
        ];
        for t in expected {
            assert!(names.iter().any(|n| n == t), "missing table {}", t);
        }

        // 预置空间已插入
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM space")
            .fetch_one(&pool)
            .await
            .expect("count space");
        assert_eq!(count, 2, "preset spaces seeded");

        // 默认存储源已插入
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM storage_source WHERE kind='local_fs'")
                .fetch_one(&pool)
                .await
                .expect("count source");
        assert_eq!(count, 1, "default storage_source seeded");
    }
}
