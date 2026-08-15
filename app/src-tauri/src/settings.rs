//! 设置中心（详细设计 §2.8）
//!
//! 本任务（m3-3.4）实现两个 Tauri 命令：
//! - `settings_get_root_dir`：读取资源根目录配置
//! - `settings_init_root_dir`：初始化资源根目录（建出 6 个类型子目录）
//!
//! ## 配置存储
//!
//! 采用**方案 B：复用 `settings` 表**（`migrations/0001_init.sql` 已建表，
//! schema 为 `(key TEXT PRIMARY KEY, value_json TEXT, updated_at INTEGER)`）。
//! 理由：任务包明确"若已有 settings 表则用表"；根目录路径属应用级设置，
//! 与业务数据同库同事务，备份/迁移成本更低。
//!
//! ## 子目录命名
//!
//! 6 个类型子目录：Code / Documents / Data / Artifacts / Tools / Media。
//! M3-3.1 已沉淀同名映射（`type_to_subdir`），待后续整合；本任务自建常量
//! `TYPE_SUBDIRS`，与 3.1 保持一致。

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::error::{AppError, CmdResult};

/// `settings` 表中根目录键名。
const ROOT_DIR_KEY: &str = "root_dir";

/// 6 个类型子目录（与 M3-3.1 `type_to_subdir` 命名一致；待后续整合）。
const TYPE_SUBDIRS: [&str; 6] = ["Code", "Documents", "Data", "Artifacts", "Tools", "Media"];

/// `settings_get_root_dir` 出参（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootDirStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_dir: Option<String>,
    pub initialized: bool,
}

/// `settings_init_root_dir` 出参（§2.8）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InitRootDirResult {
    pub root_dir: String,
    pub created: Vec<String>,
}

// ============================================================
// 内部工具
// ============================================================

/// 从 `settings` 表读取根目录路径。
async fn load_root_dir(pool: &SqlitePool) -> CmdResult<Option<String>> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?")
        .bind(ROOT_DIR_KEY)
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?;
    match row {
        None => Ok(None),
        Some(r) => {
            let value_json: String = r.try_get("value_json").map_err(AppError::from)?;
            let v: serde_json::Value = serde_json::from_str(&value_json)
                .map_err(|e| AppError::db(format!("settings.root_dir 反序列化失败: {}", e)))?;
            // 存的是 JSON string
            let s = v
                .as_str()
                .ok_or_else(|| AppError::db("settings.root_dir 非字符串"))?;
            Ok(Some(s.to_string()))
        }
    }
}

/// 将根目录路径写入 `settings` 表（UPSERT）。
async fn save_root_dir(pool: &SqlitePool, root_dir: &str) -> CmdResult<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let value_json = serde_json::to_string(&serde_json::Value::String(root_dir.to_string()))
        .map_err(|e| AppError::db(format!("settings.root_dir 序列化失败: {}", e)))?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(ROOT_DIR_KEY)
    .bind(&value_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// 校验 `root_dir`：非空、绝对路径、不含 `..`。
fn validate_root_dir(root_dir: &str) -> CmdResult<()> {
    if root_dir.trim().is_empty() {
        return Err(AppError::invalid_param("rootDir 不能为空"));
    }
    let path = std::path::Path::new(root_dir);
    if !path.is_absolute() {
        return Err(AppError::invalid_param(format!(
            "rootDir 必须为绝对路径: {}",
            root_dir
        )));
    }
    // 拒绝包含 `..` 组件的路径，防止越出预期目录
    for comp in path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(AppError::invalid_param(format!(
                "rootDir 不允许包含 '..': {}",
                root_dir
            )));
        }
    }
    Ok(())
}

/// 将 `std::io::Error` 映射为 `AppError`：
/// `PermissionDenied` → `FS_PERMISSION_DENIED`，其他 → `COMMON_IO`。
fn map_io_err(context: impl Into<String>, err: std::io::Error) -> AppError {
    let ctx = context.into();
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            AppError::new("FS_PERMISSION_DENIED", format!("{}: {}", ctx, err))
        }
        _ => AppError::io(format!("{}: {}", ctx, err)),
    }
}

// ============================================================
// 业务函数（命令与测试共用）
// ============================================================

/// `settings_get_root_dir`：读取根目录配置。
///
/// - 未设置 → `{ rootDir: null, initialized: false }`
/// - 已设置但目录不存在 → `{ rootDir: Some(...), initialized: false }`
/// - 已设置且目录存在 → `{ rootDir: Some(...), initialized: true }`
pub async fn get_root_dir(pool: &SqlitePool) -> CmdResult<RootDirStatus> {
    match load_root_dir(pool).await? {
        None => Ok(RootDirStatus {
            root_dir: None,
            initialized: false,
        }),
        Some(p) => {
            let exists = std::path::Path::new(&p).is_dir();
            Ok(RootDirStatus {
                root_dir: Some(p),
                initialized: exists,
            })
        }
    }
}

/// `settings_init_root_dir`：初始化根目录。
///
/// - 校验入参
/// - 递归创建根目录（已存在则跳过）
/// - 建出 6 个类型子目录（已存在跳过）
/// - 持久化到 `settings` 表
/// - 返回 `created`：本次实际新建的子目录绝对路径列表
pub async fn init_root_dir(pool: &SqlitePool, root_dir: String) -> CmdResult<InitRootDirResult> {
    validate_root_dir(&root_dir)?;

    let root = std::path::Path::new(&root_dir);

    // 递归创建根目录（已存在不报错）
    std::fs::create_dir_all(root)
        .map_err(|e| map_io_err(format!("创建根目录失败 {}", root_dir), e))?;

    // 建出 6 个类型子目录；已存在的跳过
    let mut created: Vec<String> = Vec::new();
    for sub in TYPE_SUBDIRS.iter() {
        let sub_path = root.join(sub);
        if sub_path.exists() {
            // 幂等：已存在则跳过，不计入 created
            continue;
        }
        std::fs::create_dir(&sub_path)
            .map_err(|e| map_io_err(format!("创建子目录失败 {}", sub_path.display()), e))?;
        created.push(sub_path.to_string_lossy().to_string());
    }

    // 持久化
    save_root_dir(pool, &root_dir).await?;

    Ok(InitRootDirResult { root_dir, created })
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_get_root_dir(
    state: tauri::State<'_, crate::AppState>,
) -> CmdResult<RootDirStatus> {
    get_root_dir(&state.pool).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn settings_init_root_dir(
    state: tauri::State<'_, crate::AppState>,
    root_dir: String,
) -> CmdResult<InitRootDirResult> {
    init_root_dir(&state.pool, root_dir).await
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

    // ---------- validate_root_dir ----------

    #[tokio::test]
    async fn init_rejects_empty_root_dir() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "".into()).await.expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = init_root_dir(&pool, "   ".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn init_rejects_relative_path() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "relative/path".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = init_root_dir(&pool, "./also/relative".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    #[tokio::test]
    async fn init_rejects_parent_dir_component() {
        let pool = setup().await;
        let err = init_root_dir(&pool, "/tmp/../etc/evil".into())
            .await
            .expect_err("should fail");
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- init_root_dir 正常路径 ----------

    #[tokio::test]
    async fn init_creates_root_and_six_subdirs() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        let res = init_root_dir(&pool, root_str.clone())
            .await
            .expect("init ok");

        assert_eq!(res.root_dir, root_str);
        assert_eq!(res.created.len(), 6, "should create 6 subdirs");

        // 6 个子目录真实存在
        for sub in TYPE_SUBDIRS.iter() {
            let p = root.join(sub);
            assert!(p.is_dir(), "missing subdir: {}", p.display());
            // created 列表里的路径应与之一致
            let s = p.to_string_lossy().to_string();
            assert!(res.created.contains(&s), "created missing {}", s);
        }
    }

    #[tokio::test]
    async fn init_is_idempotent_second_call_returns_empty_created() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        let first = init_root_dir(&pool, root_str.clone())
            .await
            .expect("first ok");
        assert_eq!(first.created.len(), 6);

        let second = init_root_dir(&pool, root_str.clone())
            .await
            .expect("second ok");
        assert_eq!(second.root_dir, root_str);
        assert!(second.created.is_empty(), "second call must be idempotent");

        // 子目录依然存在
        for sub in TYPE_SUBDIRS.iter() {
            assert!(root.join(sub).is_dir());
        }
    }

    #[tokio::test]
    async fn init_skips_preexisting_subdirs() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        // 预先创建 root 与其中一个子目录
        std::fs::create_dir_all(root.join("Code")).expect("pre-create");

        let root_str = root.to_string_lossy().to_string();
        let res = init_root_dir(&pool, root_str).await.expect("init ok");

        assert_eq!(res.created.len(), 5, "Code 已存在，应只新建 5 个");
        let code_path = root.join("Code").to_string_lossy().to_string();
        assert!(!res.created.contains(&code_path));
    }

    // ---------- get_root_dir ----------

    #[tokio::test]
    async fn get_root_dir_uninitialized_returns_none() {
        let pool = setup().await;
        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir, None);
        assert!(!s.initialized);
    }

    #[tokio::test]
    async fn get_root_dir_after_init_returns_path_and_initialized() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        init_root_dir(&pool, root_str.clone()).await.expect("init ok");

        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(s.initialized);
    }

    #[tokio::test]
    async fn get_root_dir_returns_uninitialized_when_dir_missing() {
        let pool = setup().await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        init_root_dir(&pool, root_str.clone()).await.expect("init ok");

        // 删除根目录（模拟外部清理）
        std::fs::remove_dir_all(&root).expect("remove root");

        let s = get_root_dir(&pool).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(!s.initialized, "目录不存在 → initialized=false");
    }

    // ---------- 配置持久化 ----------

    #[tokio::test]
    async fn root_dir_persists_across_pool_reopen() {
        // 用文件库模拟"重启应用"：drop 池后用同一文件重开
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let root = tmp.path().join("my-root");
        let root_str = root.to_string_lossy().to_string();

        {
            let pool = crate::db::init_pool_with_file(&db_path)
                .await
                .expect("init pool");
            init_root_dir(&pool, root_str.clone()).await.expect("init ok");
            pool.close().await;
        }

        // 重新打开同一文件
        let pool2 = crate::db::init_pool_with_file(&db_path)
            .await
            .expect("reopen pool");
        let s = get_root_dir(&pool2).await.expect("get ok");
        assert_eq!(s.root_dir.as_deref(), Some(root_str.as_str()));
        assert!(s.initialized);
    }

    // ---------- 序列化契约 ----------

    #[test]
    fn root_dir_status_serializes_camel_case() {
        let s = RootDirStatus {
            root_dir: Some("/tmp/x".into()),
            initialized: true,
        };
        let v = serde_json::to_value(&s).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("rootDir"));
        assert!(obj.contains_key("initialized"));
        assert!(!obj.contains_key("root_dir"));

        // rootDir=None 时被 skip
        let s2 = RootDirStatus {
            root_dir: None,
            initialized: false,
        };
        let v2 = serde_json::to_value(&s2).expect("serialize");
        let obj2 = v2.as_object().expect("object");
        assert!(!obj2.contains_key("rootDir"));
        assert_eq!(obj2["initialized"].as_bool(), Some(false));
    }

    #[test]
    fn init_root_dir_result_serializes_camel_case() {
        let r = InitRootDirResult {
            root_dir: "/tmp/x".into(),
            created: vec!["/tmp/x/Code".into()],
        };
        let v = serde_json::to_value(&r).expect("serialize");
        let obj = v.as_object().expect("object");
        assert!(obj.contains_key("rootDir"));
        assert!(obj.contains_key("created"));
        assert_eq!(obj["created"].as_array().expect("arr").len(), 1);
    }
}
