//! m5-5.5 · 敏感文件识别与预览保护（详细设计 §2.7 / §6.9）
//!
//! 在 `inbox_get` 返回条目详情时识别敏感文件（`.env` / 私钥 / 凭据等），
//! 由调用方决定「不返回内容预览，仅返回文件名 + 风险提示」，
//! 防止凭据泄露到 UI。
//!
//! 设计要点：
//! - `SENSITIVE_PATTERNS` 硬编码规则集，**不读 DB**、**不读用户配置**；
//!   规则演进通过代码评审与版本升级管理。
//! - `is_sensitive` 为纯函数：不读写文件系统，仅按路径模式匹配。
//! - 匹配规则三分支：
//!   1. 文件名精确匹配（`.env` / `id_rsa`）；
//!   2. 文件名 glob（`.env.*` / `*.pem` / `*.key` / `*.keystore` / `*.jks`）；
//!   3. 路径子串（`.aws/credentials` / `.ssh/config`），匹配「路径中段」，
//!      避免要求规则必须命中完整路径前缀。
//! - `sensitive_warning` 返回人类可读风险提示，**包含文件名**（前端直接渲染）。

use std::path::Path;

use glob::Pattern;

// ============================================================
// 硬编码规则集
// ============================================================

/// 敏感文件识别规则（硬编码）。
///
/// 三种形态：
/// - 文件名精确：`.env` / `id_rsa` / `id_dsa` / `id_ecdsa` / `id_ed25519`
/// - 文件名 glob：`.env.*` / `*.pem` / `*.key` / `*.p12` / `*.pfx` / `*.keystore` / `*.jks`
/// - 路径子串：`.aws/credentials` / `.ssh/config`（命中路径中段即视为敏感）
pub const SENSITIVE_PATTERNS: &[&str] = &[
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".aws/credentials",
    ".ssh/config",
    "*.keystore",
    "*.jks",
];

// ============================================================
// 匹配（纯函数）
// ============================================================

/// 提取路径文件名（UTF-8 字符串）。无文件名返回 None。
fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|n| n.to_str())
}

/// 把路径转为正斜杠分隔的字符串，供子串匹配使用（跨平台统一）。
fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 单条规则是否命中（纯函数）。
///
/// 判定顺序：
/// 1. 规则含 `/` → 视为路径子串规则，命中路径任一位置即返回 true；
/// 2. 规则含 glob 元字符（`* ? [ ]`）→ 走 `glob::Pattern` 匹配文件名；
/// 3. 否则视为文件名精确匹配。
fn pattern_matches(rule: &str, path: &Path) -> bool {
    // 1. 路径子串规则
    if rule.contains('/') {
        let slash = path_to_slash(path);
        return slash.contains(rule);
    }

    // 2/3. 文件名规则
    let name = match file_name(path) {
        Some(n) => n,
        None => return false,
    };

    if rule.contains(['*', '?', '[', ']']) {
        if let Ok(p) = Pattern::new(rule) {
            return p.matches(name);
        }
        return false;
    }
    name == rule
}

/// 判断路径是否敏感（纯函数）。
///
/// 命中 `SENSITIVE_PATTERNS` 任一规则即返回 true。
/// 不读写文件系统，不读 DB，不依赖任何运行时状态。
pub fn is_sensitive(path: &Path) -> bool {
    SENSITIVE_PATTERNS
        .iter()
        .any(|rule| pattern_matches(rule, path))
}

/// 敏感文件的风险提示文案。
///
/// 包含文件名（前端直接渲染为黄色 Alert 标题/正文）。
/// 对非敏感路径也返回通用文案（调用方应先用 `is_sensitive` 判定）。
pub fn sensitive_warning(path: &Path) -> String {
    let name = file_name(path).unwrap_or("<未知文件>");
    format!(
        "「{}」可能是敏感文件（含凭据 / 密钥 / 配置），已禁用内容预览以防泄露。",
        name
    )
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    // ---------- 敏感：文件名精确 ----------

    #[test]
    fn sensitive_env_exact() {
        assert!(is_sensitive(&p("/home/user/proj/.env")));
    }

    #[test]
    fn sensitive_id_rsa() {
        assert!(is_sensitive(&p("/home/user/.ssh/id_rsa")));
    }

    #[test]
    fn sensitive_id_ed25519() {
        assert!(is_sensitive(&p("/home/user/.ssh/id_ed25519")));
    }

    // ---------- 敏感：文件名 glob ----------

    #[test]
    fn sensitive_env_local() {
        assert!(is_sensitive(&p("/proj/.env.local")));
    }

    #[test]
    fn sensitive_env_production() {
        assert!(is_sensitive(&p("/proj/.env.production")));
    }

    #[test]
    fn sensitive_pem() {
        assert!(is_sensitive(&p("/etc/ssl/server.pem")));
    }

    #[test]
    fn sensitive_key() {
        assert!(is_sensitive(&p("/etc/ssl/private.key")));
    }

    #[test]
    fn sensitive_p12() {
        assert!(is_sensitive(&p("/certs/client.p12")));
    }

    #[test]
    fn sensitive_keystore_jks() {
        assert!(is_sensitive(&p("/app/keystore.jks")));
    }

    #[test]
    fn sensitive_keystore() {
        assert!(is_sensitive(&p("/app/release.keystore")));
    }

    // ---------- 敏感：路径子串 ----------

    #[test]
    fn sensitive_aws_credentials() {
        assert!(is_sensitive(&p("/home/user/.aws/credentials")));
    }

    #[test]
    fn sensitive_ssh_config() {
        assert!(is_sensitive(&p("/home/user/.ssh/config")));
    }

    // ---------- 非敏感 ----------

    #[test]
    fn not_sensitive_pdf() {
        assert!(!is_sensitive(&p("/docs/report.pdf")));
    }

    #[test]
    fn not_sensitive_rs() {
        assert!(!is_sensitive(&p("/proj/src/main.rs")));
    }

    #[test]
    fn not_sensitive_json() {
        assert!(!is_sensitive(&p("/proj/data.json")));
    }

    #[test]
    fn not_sensitive_envrc() {
        // 任务包明确：`.envrc` 不算敏感（除非明确加入规则）
        assert!(!is_sensitive(&p("/proj/.envrc")));
    }

    #[test]
    fn not_sensitive_env_like_substring() {
        // 「env」作为子串不应命中（仅精确 `.env` 或 glob `.env.*`）
        assert!(!is_sensitive(&p("/proj/environment.txt")));
    }

    // ---------- 风险提示文案 ----------

    #[test]
    fn warning_contains_file_name() {
        let msg = sensitive_warning(&p("/home/user/proj/.env"));
        assert!(msg.contains(".env"), "提示应包含文件名: {}", msg);
    }

    #[test]
    fn warning_contains_file_name_pem() {
        let msg = sensitive_warning(&p("/etc/ssl/server.pem"));
        assert!(msg.contains("server.pem"), "提示应包含文件名: {}", msg);
    }

    #[test]
    fn warning_indicates_preview_disabled() {
        let msg = sensitive_warning(&p("/proj/.env"));
        assert!(msg.contains("预览"), "提示应说明预览被禁用: {}", msg);
    }
}
