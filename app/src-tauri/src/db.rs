//! 数据库接入与迁移框架（详细设计 §3.3）
//!
//! - 使用 sqlx SQLite，启动时自动执行未应用的迁移。
//! - 数据库文件位置固定为用户 home 目录下：`~/.workbench/workbench.db`。
//!   （历史版本位于 `{平台用户数据目录}/resource-workbench/`，启动时自动迁移。）
//! - 迁移文件按 `0001_init.sql`、`0002_*.sql` 递增，只增不改。

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

use crate::error::{AppError, CmdResult};

/// 嵌入的迁移集合（编译期打包 `migrations/` 目录）。
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// 数据库文件名（位于数据目录下）。
pub const DB_FILE_NAME: &str = "workbench.db";

/// 数据目录名（位于用户 home 目录下，即 `~/.workbench`）。
pub const DATA_DIR_NAME: &str = ".workbench";

/// 旧版数据目录（Tauri app data 目录下的子目录名），仅用于一次性迁移。
pub const LEGACY_APP_DATA_DIR: &str = "resource-workbench";

/// 若新数据目录中尚无数据库、而旧目录中存在，则将数据库文件迁移到新目录。
///
/// 迁移范围为主库及 WAL/SHM 伴随文件；跨设备 rename 失败时回退为复制+删除。
/// 返回是否发生了迁移。
pub fn migrate_legacy_data(legacy_dir: &Path, new_dir: &Path) -> CmdResult<bool> {
    let new_db = new_dir.join(DB_FILE_NAME);
    let old_db = legacy_dir.join(DB_FILE_NAME);
    if new_db.exists() || !old_db.exists() {
        return Ok(false);
    }

    std::fs::create_dir_all(new_dir)
        .map_err(|e| AppError::io(format!("创建数据目录失败 {}: {}", new_dir.display(), e)))?;

    for suffix in ["", "-wal", "-shm"] {
        let name = format!("{}{}", DB_FILE_NAME, suffix);
        let from = legacy_dir.join(&name);
        if !from.exists() {
            continue;
        }
        let to = new_dir.join(&name);
        if std::fs::rename(&from, &to).is_err() {
            // 跨设备等情况 rename 失败：复制后删除源文件
            std::fs::copy(&from, &to).map_err(|e| {
                AppError::io(format!(
                    "迁移数据库文件失败 {} -> {}: {}",
                    from.display(),
                    to.display(),
                    e
                ))
            })?;
            std::fs::remove_file(&from).map_err(|e| {
                AppError::io(format!("删除旧数据库文件失败 {}: {}", from.display(), e))
            })?;
        }
    }

    Ok(true)
}

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

    // 启动时兜底修复：watch_dir 表若缺少 name/description 列则自动添加（m6-6.3）
    ensure_watch_dir_columns(&pool).await?;

    Ok(pool)
}

/// 检查 watch_dir 表结构，缺失 name/description 列时自动补全。
/// 用于兼容旧版本数据库（迁移未执行或执行失败的情况）。
async fn ensure_watch_dir_columns(pool: &SqlitePool) -> CmdResult<()> {
    let has_name: bool = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('watch_dir') WHERE name = 'name'",
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;

    if !has_name {
        sqlx::query("ALTER TABLE watch_dir ADD COLUMN name TEXT")
            .execute(pool)
            .await
            .map_err(AppError::from)?;
    }

    let has_description: bool = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('watch_dir') WHERE name = 'description'",
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::from)?;

    if !has_description {
        sqlx::query("ALTER TABLE watch_dir ADD COLUMN description TEXT")
            .execute(pool)
            .await
            .map_err(AppError::from)?;
    }

    Ok(())
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
            "ref_access_log",
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

    #[test]
    fn migrate_legacy_data_moves_db_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("legacy");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("workbench.db"), b"db").unwrap();
        std::fs::write(legacy.join("workbench.db-wal"), b"wal").unwrap();

        let migrated = migrate_legacy_data(&legacy, &new).expect("migrate ok");
        assert!(migrated, "should report migration");
        assert_eq!(std::fs::read(new.join("workbench.db")).unwrap(), b"db");
        assert_eq!(std::fs::read(new.join("workbench.db-wal")).unwrap(), b"wal");
        assert!(!legacy.join("workbench.db").exists(), "old db removed");
        assert!(!legacy.join("workbench.db-wal").exists(), "old wal removed");
    }

    #[test]
    fn migrate_legacy_data_skips_when_new_db_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("legacy");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(legacy.join("workbench.db"), b"old").unwrap();
        std::fs::write(new.join("workbench.db"), b"new").unwrap();

        let migrated = migrate_legacy_data(&legacy, &new).expect("migrate ok");
        assert!(!migrated, "should not migrate over existing db");
        // 新库保持原样，旧库不被动过
        assert_eq!(std::fs::read(new.join("workbench.db")).unwrap(), b"new");
        assert_eq!(std::fs::read(legacy.join("workbench.db")).unwrap(), b"old");
    }

    #[test]
    fn migrate_legacy_data_noop_without_legacy_db() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("legacy");
        let new = tmp.path().join("new");

        let migrated = migrate_legacy_data(&legacy, &new).expect("migrate ok");
        assert!(!migrated, "nothing to migrate");
        assert!(!new.join("workbench.db").exists(), "no db created");
    }
}
