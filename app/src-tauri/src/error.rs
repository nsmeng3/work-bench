//! 统一错误模型（详细设计 §2.2）
//!
//! 所有 Tauri Command 失败时返回统一结构：
//! `{ code, message, details, retryable }`。
//!
//! 错误码命名规则：`{域}_{原因}`，全大写下划线。

use serde::Serialize;
use serde_json::Value;

/// 统一错误结构，前端据此决定提示与降级。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            retryable: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    // ---- 通用错误码构造器 ----

    pub fn invalid_param(message: impl Into<String>) -> Self {
        Self::new("COMMON_INVALID_PARAM", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("COMMON_NOT_FOUND", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("COMMON_CONFLICT", message)
    }

    pub fn db(message: impl Into<String>) -> Self {
        Self::new("COMMON_DB", message).retryable(true)
    }

    #[allow(dead_code)]
    pub fn io(message: impl Into<String>) -> Self {
        Self::new("COMMON_IO", message).retryable(true)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// 不向外暴露 sqlx 内部错误：统一映射为 COMMON_DB。
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::db(format!("数据库错误: {}", err))
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(err: sqlx::migrate::MigrateError) -> Self {
        AppError::db(format!("迁移错误: {}", err))
    }
}

/// 命令返回类型别名。
pub type CmdResult<T> = Result<T, AppError>;
