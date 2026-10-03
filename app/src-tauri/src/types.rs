//! M6-6.1 · 监控目录契约类型（详细设计 §2.8）

use serde::{Deserialize, Serialize};

/// 监控目录配置：对齐 `watch_dir` 表 + 前端 `WatchDirConfig`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WatchDirConfig {
    pub id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 监听是否已暂停（1=挂载失败被暂停，0/None=正常）。
    /// 仅出参有意义；作为 `watch_dir_set` 入参时忽略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<i64>,
}

/// 监控事件：created / modified / renamed / removed。
///
/// M6-6.1 仅冻结契约形状；6.2 事件缓冲落地后开始被构造。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    /// created | modified | renamed | removed
    pub kind: String,
    pub path: String,
    /// Unix 秒
    pub at: i64,
}
