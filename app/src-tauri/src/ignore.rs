//! m5-5.2 · 忽略规则引擎（详细设计 §5.2「忽略规则前置过滤」）
//!
//! 消费 5.1 的 `WatchEvent` 流，前置过滤掉命中忽略规则的事件，
//! 避免隐藏文件 / 临时文件 / 构建产物进入收件箱。
//!
//! 设计要点：
//! - `should_ignore` 为纯函数：不读写 DB，不碰文件系统；输入路径 + 规则集，输出布尔。
//! - 规则来源：默认规则集（硬编码） + 用户规则（`ignore_rule` 表，M1 已建 schema）。
//! - 规则热更新：用 `Arc<RwLock<Vec<IgnoreRule>>>` 共享；`load_rules` 可被 5.4 触发重载。
//! - `filter_events` 在 5.1 的 `mpsc::Receiver<WatchEvent>` 与下游消费者之间架管道。

use std::path::Path;
use std::sync::{Arc, RwLock};

use glob::Pattern;
use sqlx::sqlite::SqlitePool;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::error::{AppError, CmdResult};
use crate::watch::WatchEvent;

// ============================================================
// 规则模型
// ============================================================

/// 忽略规则种类（与 `ignore_rule.kind` 一致）。
///
/// 注：`ignore_rule` 表 schema 的 CHECK 只允许 `by_ext / by_name / by_dir`，
/// `ByPattern` 仅供默认规则集与内存态使用（用户自定义 pattern 走 `ByName` 的 glob 分支）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreKind {
    /// 按扩展名：".tmp" / ".log"（不区分大小写）
    ByExt,
    /// 按文件名（精确或 glob）：".env" / ".DS_Store" / "node_modules" / ".env.*"
    ByName,
    /// 按目录路径前缀："/tmp/build"
    ByDir,
    /// 自定义完整路径 glob："*.swp" / "**/target/**"
    ByPattern,
}

impl IgnoreKind {
    /// 从 DB 字符串解析；未识别返回 None（容忍脏数据，跳过该行）。
    fn from_db(s: &str) -> Option<Self> {
        match s {
            "by_ext" => Some(Self::ByExt),
            "by_name" => Some(Self::ByName),
            "by_dir" => Some(Self::ByDir),
            // schema CHECK 不含 by_pattern；防御性解析，便于未来扩展
            "by_pattern" => Some(Self::ByPattern),
            _ => None,
        }
    }
}

/// 忽略规则（从 `ignore_rule` 表读出，或来自默认规则集）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreRule {
    pub kind: IgnoreKind,
    pub value: String,
}

// ============================================================
// 默认规则集（硬编码，无需 DB）
// ============================================================

/// 默认规则集：
/// - ByName: `.DS_Store` / `.git` / `.svn` / `node_modules` / `target` / `dist` / `build` / `.next` / `.cache`
/// - ByName（Chromium 系下载临时文件，`.<bundle id>.<随机串>` 隐藏文件）:
///   `.com.google.Chrome.*` / `.com.microsoft.edgemac.*` / `.org.chromium.Chromium.*` /
///   `.com.brave.Browser.*` / `.com.vivaldi.Vivaldi.*` / `.com.operasoftware.Opera.*`
/// - ByExt: `.tmp` / `.swp` / `.log` / `.lock` / `.bak`
/// - ByExt（浏览器下载临时文件）: `.crdownload`(Chrome/Edge) / `.download`(Safari) / `.partial`(Edge 旧版)
/// - ByPattern: `**/*.part` / `**/~*`
///
/// 浏览器下载先落临时文件、完成后改名正式名。临时扩展名前置过滤，
/// 避免临时文件进入收件箱（改名事件由 filter_events 按 new_path 改写放行，
/// 保证正式文件不丢通知）。注意 macOS 上 Chromium 系浏览器下载中的临时文件
/// 是 `.<bundle id>.<随机串>` 形式的隐藏文件（如 `.com.google.Chrome.qXESF8`），
/// 不带 `.crdownload` 扩展名，必须按文件名 glob 拦截。
pub fn default_rules() -> Vec<IgnoreRule> {
    let mut out = Vec::new();

    for v in [
        ".DS_Store",
        ".git",
        ".svn",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".cache",
    ] {
        out.push(IgnoreRule {
            kind: IgnoreKind::ByName,
            value: v.to_string(),
        });
    }

    for v in [".tmp", ".swp", ".log", ".lock", ".bak"] {
        out.push(IgnoreRule {
            kind: IgnoreKind::ByExt,
            value: v.to_string(),
        });
    }

    // 浏览器下载临时文件：Chrome/Edge → .crdownload，Safari → .download，
    // Edge 旧版/IE → .partial（Firefox 的 .part 由下方 ByPattern 覆盖）
    for v in [".crdownload", ".download", ".partial"] {
        out.push(IgnoreRule {
            kind: IgnoreKind::ByExt,
            value: v.to_string(),
        });
    }

    // Chromium 系浏览器下载中的临时文件（macOS）：`.<bundle id>.<随机串>` 隐藏文件，
    // 如 `.com.google.Chrome.qXESF8`，不带 .crdownload 扩展名，按文件名 glob 拦截。
    // bundle id 对应：Chrome / Edge / Chromium / Brave / Vivaldi / Opera
    for v in [
        ".com.google.Chrome.*",
        ".com.microsoft.edgemac.*",
        ".org.chromium.Chromium.*",
        ".com.brave.Browser.*",
        ".com.vivaldi.Vivaldi.*",
        ".com.operasoftware.Opera.*",
    ] {
        out.push(IgnoreRule {
            kind: IgnoreKind::ByName,
            value: v.to_string(),
        });
    }

    for v in ["**/*.part", "**/~*"] {
        out.push(IgnoreRule {
            kind: IgnoreKind::ByPattern,
            value: v.to_string(),
        });
    }

    out
}

// ============================================================
// 匹配（纯函数）
// ============================================================

/// 提取路径扩展名（小写，不带点）。无扩展名返回 None。
fn lower_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

/// 提取路径文件名。无文件名返回 None。
fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|n| n.to_str())
}

/// 把路径转为正斜杠分隔的字符串，供 glob 匹配使用（跨平台统一）。
fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 单条规则匹配（纯函数）。
fn matches(rule: &IgnoreRule, path: &Path) -> bool {
    match rule.kind {
        IgnoreKind::ByExt => {
            // 规则 value 形如 ".tmp"（带点）；路径扩展名不带点。
            let want = rule.value.trim_start_matches('.').to_lowercase();
            if want.is_empty() {
                return false;
            }
            match lower_ext(path) {
                Some(ext) => ext == want,
                None => false,
            }
        }
        IgnoreKind::ByName => {
            let name = match file_name(path) {
                Some(n) => n,
                None => return false,
            };
            // 精确匹配优先；含 glob 元字符时走 glob
            if name == rule.value {
                return true;
            }
            if rule.value.contains(['*', '?', '[', ']']) {
                if let Ok(p) = Pattern::new(&rule.value) {
                    return p.matches(name);
                }
            }
            false
        }
        IgnoreKind::ByDir => {
            // 路径前缀匹配：规则 value 视为目录路径
            let dir = Path::new(&rule.value);
            path.starts_with(dir)
        }
        IgnoreKind::ByPattern => {
            let p = match Pattern::new(&rule.value) {
                Ok(p) => p,
                Err(_) => return false,
            };
            let slash = path_to_slash(path);
            // 同时尝试原始字符串与正斜杠形式，提升跨平台命中
            p.matches(&slash) || p.matches_path(path)
        }
    }
}

/// 路径命中任一规则返回 true。
///
/// 纯函数：不读写 DB，不碰文件系统。
pub fn should_ignore(path: &Path, rules: &[IgnoreRule]) -> bool {
    rules.iter().any(|r| matches(r, path))
}

// ============================================================
// 规则加载（DB → 合并默认）
// ============================================================

/// 从 DB 读用户规则 + 合并默认规则。
///
/// 用户规则在前（优先匹配），默认规则在后。
/// 未识别的 kind 行静默跳过（防御脏数据）。
pub async fn load_rules(pool: &SqlitePool) -> CmdResult<Vec<IgnoreRule>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, value FROM ignore_rule ORDER BY created_at ASC")
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;

    let mut out: Vec<IgnoreRule> = rows
        .into_iter()
        .filter_map(|(k, v)| IgnoreKind::from_db(&k).map(|kind| IgnoreRule { kind, value: v }))
        .collect();

    out.extend(default_rules());
    Ok(out)
}

// ============================================================
// 事件过滤管道（5.1 接入点）
// ============================================================

/// 在 `WatchEvent` 流上应用过滤：从 `rx` 读事件，命中规则则丢弃，否则转发到 `tx`。
///
/// 规则通过 `Arc<RwLock<Vec<IgnoreRule>>>` 共享，5.4 可触发 `load_rules` 重载后写入。
/// 读锁中毒时按"无规则"放行（避免阻塞事件流）。
///
/// 改名事件的特例（浏览器下载临时文件场景）：`Renamed` 且携带 `new_path` 时，
/// from 路径命中忽略规则（如 `.crdownload`）不代表整条事件该丢——
/// 正式文件路径（new_path）未命中规则时，把事件改写为 `Created(new_path)` 放行，
/// 保证「临时文件改名正式名」后正式文件仍能进收件箱（跨平台兜底：
/// Windows ReadDirectoryChangesW 的 RenameMode::Both 只有一条事件，
/// from 被忽略就把整条吞掉会导致正式文件永远无通知）。
///
/// 返回 `JoinHandle`，调用方可选择 `await` 或 detach。
pub fn filter_events(
    mut rx: mpsc::Receiver<WatchEvent>,
    tx: mpsc::Sender<WatchEvent>,
    rules: Arc<RwLock<Vec<IgnoreRule>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(mut ev) = rx.recv().await {
            let (path_ignored, new_path_ignored) = {
                let guard = rules.read().ok();
                match guard {
                    Some(g) => (
                        should_ignore(&ev.path, &g),
                        ev.new_path.as_ref().map(|p| should_ignore(p, &g)),
                    ),
                    None => (false, None), // 锁中毒：放行
                }
            };
            if path_ignored {
                // from 被忽略但 to 未忽略：改写为 Created(to) 放行正式文件
                match (ev.new_path.clone(), new_path_ignored) {
                    (Some(to), Some(false)) => {
                        ev.kind = crate::watch::WatchEventKind::Created;
                        ev.path = to;
                        ev.new_path = None;
                    }
                    _ => continue, // 无 new_path 或 to 也被忽略：整条丢弃
                }
            }
            // 下游关闭则退出
            if tx.send(ev).await.is_err() {
                break;
            }
        }
    })
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rule(kind: IgnoreKind, value: &str) -> IgnoreRule {
        IgnoreRule {
            kind,
            value: value.to_string(),
        }
    }

    // ---------- 默认规则集覆盖 ----------

    #[test]
    fn default_rules_ignore_ds_store() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/a/b/.DS_Store"), &rules));
        assert!(should_ignore(Path::new(".DS_Store"), &rules));
    }

    #[test]
    fn default_rules_ignore_node_modules_file() {
        let rules = default_rules();
        // node_modules 命中的是「该目录本身」；其下文件不会被 ByName 命中
        // （ByName 只匹配文件名，不匹配路径中间目录）。
        // 因此 node_modules/x.js 不命中 ByName("node_modules")；
        // 但 node_modules 目录本身的事件应被忽略。
        assert!(should_ignore(Path::new("/proj/node_modules"), &rules));
        // 子文件不命中（符合预期：5.1 不递归监听被忽略目录，由上层控制）
        assert!(!should_ignore(Path::new("/proj/node_modules/x.js"), &rules));
    }

    #[test]
    fn default_rules_ignore_target_dir() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/proj/target"), &rules));
        assert!(should_ignore(Path::new("/proj/dist"), &rules));
        assert!(should_ignore(Path::new("/proj/build"), &rules));
        assert!(should_ignore(Path::new("/proj/.next"), &rules));
        assert!(should_ignore(Path::new("/proj/.cache"), &rules));
        assert!(should_ignore(Path::new("/proj/.git"), &rules));
        assert!(should_ignore(Path::new("/proj/.svn"), &rules));
    }

    #[test]
    fn default_rules_ignore_tmp_swp_log_lock_bak() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/a/x.tmp"), &rules));
        assert!(should_ignore(Path::new("/a/x.swp"), &rules));
        assert!(should_ignore(Path::new("/a/x.log"), &rules));
        assert!(should_ignore(Path::new("/a/x.lock"), &rules));
        assert!(should_ignore(Path::new("/a/x.bak"), &rules));
    }

    #[test]
    fn default_rules_ignore_part_glob() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/a/b/c.part"), &rules));
        assert!(should_ignore(Path::new("c.part"), &rules));
    }

    #[test]
    fn default_rules_ignore_tilde_backup() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/a/b/~notes.txt"), &rules));
        assert!(should_ignore(Path::new("~draft"), &rules));
    }

    #[test]
    fn default_rules_ignore_browser_download_temps() {
        let rules = default_rules();
        // Chrome / Edge / Brave
        assert!(should_ignore(Path::new("/downloads/report.zip.crdownload"), &rules));
        // Safari
        assert!(should_ignore(Path::new("/downloads/report.zip.download"), &rules));
        // Edge 旧版 / IE
        assert!(should_ignore(Path::new("/downloads/report.zip.partial"), &rules));
        // 大小写不敏感
        assert!(should_ignore(Path::new("/downloads/report.zip.CRDOWNLOAD"), &rules));
        // 完成后的正式文件不命中
        assert!(!should_ignore(Path::new("/downloads/report.zip"), &rules));
    }

    #[test]
    fn default_rules_ignore_chromium_hidden_download_temps() {
        let rules = default_rules();
        // 用户实测：macOS Chrome 下载中的临时文件
        assert!(should_ignore(
            Path::new("/Users/x/Downloads/.com.google.Chrome.qXESF8"),
            &rules
        ));
        // 其他 Chromium 系浏览器同形态
        assert!(should_ignore(Path::new("/dl/.com.microsoft.edgemac.Ab12Cd"), &rules));
        assert!(should_ignore(Path::new("/dl/.org.chromium.Chromium.Zz"), &rules));
        assert!(should_ignore(Path::new("/dl/.com.brave.Browser.12345"), &rules));
        // 不误伤：无随机后缀的同名目录/文件、正常隐藏文件、正常文件
        assert!(!should_ignore(Path::new("/dl/.com.google.Chrome"), &rules));
        assert!(!should_ignore(Path::new("/dl/.gitignore"), &rules));
        assert!(!should_ignore(Path::new("/dl/report.zip"), &rules));
    }

    // ---------- 用户规则：ByExt / ByName / ByDir / ByPattern 各一例 ----------

    #[test]
    fn user_rule_by_ext() {
        let rules = vec![rule(IgnoreKind::ByExt, ".psd")];
        assert!(should_ignore(Path::new("/a/design.psd"), &rules));
        assert!(!should_ignore(Path::new("/a/design.pdf"), &rules));
    }

    #[test]
    fn user_rule_by_name_exact() {
        let rules = vec![rule(IgnoreKind::ByName, ".env")];
        assert!(should_ignore(Path::new("/proj/.env"), &rules));
        assert!(!should_ignore(Path::new("/proj/.env.local"), &rules));
    }

    #[test]
    fn user_rule_by_name_glob() {
        let rules = vec![rule(IgnoreKind::ByName, ".env.*")];
        assert!(should_ignore(Path::new("/proj/.env.local"), &rules));
        assert!(should_ignore(Path::new("/proj/.env.production"), &rules));
        assert!(!should_ignore(Path::new("/proj/.env"), &rules));
    }

    #[test]
    fn user_rule_by_dir_prefix() {
        let rules = vec![rule(IgnoreKind::ByDir, "/tmp/build")];
        assert!(should_ignore(Path::new("/tmp/build"), &rules));
        assert!(should_ignore(Path::new("/tmp/build/out.bin"), &rules));
        assert!(!should_ignore(Path::new("/tmp/buildx"), &rules));
        assert!(!should_ignore(Path::new("/var/tmp/build"), &rules));
    }

    #[test]
    fn user_rule_by_pattern_glob() {
        let rules = vec![rule(IgnoreKind::ByPattern, "**/target/**")];
        assert!(should_ignore(Path::new("/proj/target/debug/app"), &rules));
        assert!(!should_ignore(Path::new("/proj/src/main.rs"), &rules));
    }

    // ---------- 不命中：正常文件 ----------

    #[test]
    fn normal_files_not_ignored() {
        let rules = default_rules();
        assert!(!should_ignore(Path::new("/a/report.pdf"), &rules));
        assert!(!should_ignore(Path::new("/a/main.rs"), &rules));
        assert!(!should_ignore(Path::new("/a/README.md"), &rules));
        assert!(!should_ignore(Path::new("/a/photo.jpg"), &rules));
    }

    // ---------- 大小写不敏感 ----------

    #[test]
    fn ext_case_insensitive() {
        let rules = default_rules();
        assert!(should_ignore(Path::new("/a/x.TMP"), &rules));
        assert!(should_ignore(Path::new("/a/x.Log"), &rules));
        assert!(should_ignore(Path::new("/a/x.SWP"), &rules));
        assert!(should_ignore(Path::new("/a/x.BAK"), &rules));
    }

    #[test]
    fn user_ext_case_insensitive() {
        let rules = vec![rule(IgnoreKind::ByExt, ".PsD")];
        assert!(should_ignore(Path::new("/a/x.psd"), &rules));
        assert!(should_ignore(Path::new("/a/x.PSD"), &rules));
        assert!(should_ignore(Path::new("/a/x.Psd"), &rules));
    }

    // ---------- glob 复杂模式 ----------

    #[test]
    fn glob_double_star_build() {
        let rules = vec![rule(IgnoreKind::ByPattern, "**/build/**")];
        assert!(should_ignore(Path::new("/proj/build/out.js"), &rules));
        assert!(should_ignore(Path::new("/a/b/build/c/d.txt"), &rules));
        assert!(!should_ignore(Path::new("/proj/build"), &rules)); // 目录本身不命中 **/build/**
    }

    #[test]
    fn glob_min_js() {
        let rules = vec![rule(IgnoreKind::ByPattern, "**/*.min.js")];
        assert!(should_ignore(Path::new("/proj/dist/app.min.js"), &rules));
        assert!(should_ignore(Path::new("app.min.js"), &rules));
        assert!(!should_ignore(Path::new("/proj/dist/app.js"), &rules));
    }

    // ---------- 边界：无扩展名 / 无文件名 ----------

    #[test]
    fn no_extension_not_matched_by_ext() {
        let rules = vec![rule(IgnoreKind::ByExt, ".tmp")];
        assert!(!should_ignore(Path::new("/a/Makefile"), &rules));
        assert!(!should_ignore(Path::new("/a/.gitignore"), &rules)); // 无扩展名（dotfile）
    }

    #[test]
    fn empty_rules_nothing_ignored() {
        let rules: Vec<IgnoreRule> = Vec::new();
        assert!(!should_ignore(Path::new("/a/.DS_Store"), &rules));
        assert!(!should_ignore(Path::new("/a/x.tmp"), &rules));
    }

    // ---------- load_rules：DB 合并 ----------

    #[tokio::test]
    async fn load_rules_merges_user_and_default() {
        let pool = SqlitePool::connect(":memory:").await.expect("pool");
        sqlx::query(
            "CREATE TABLE ignore_rule (
                id TEXT PRIMARY KEY,
                kind TEXT CHECK (kind IN ('by_ext', 'by_name', 'by_dir')),
                value TEXT NOT NULL,
                created_at INTEGER
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");

        sqlx::query("INSERT INTO ignore_rule (id, kind, value, created_at) VALUES (?, ?, ?, 0)")
            .bind("r1")
            .bind("by_ext")
            .bind(".psd")
            .execute(&pool)
            .await
            .expect("insert");

        let rules = load_rules(&pool).await.expect("load_rules");
        // 用户规则在前
        assert_eq!(rules[0].kind, IgnoreKind::ByExt);
        assert_eq!(rules[0].value, ".psd");
        // 默认规则在后
        assert!(rules.len() > 1);
        assert!(rules.iter().any(|r| r.value == ".DS_Store"));
        // 合并后既能匹配用户规则也能匹配默认规则
        assert!(should_ignore(Path::new("/a/x.psd"), &rules));
        assert!(should_ignore(Path::new("/a/.DS_Store"), &rules));
    }

    #[tokio::test]
    async fn load_rules_skips_unknown_kind() {
        let pool = SqlitePool::connect(":memory:").await.expect("pool");
        // 不用 CHECK 约束，模拟脏数据
        sqlx::query(
            "CREATE TABLE ignore_rule (
                id TEXT PRIMARY KEY,
                kind TEXT,
                value TEXT NOT NULL,
                created_at INTEGER
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");

        sqlx::query("INSERT INTO ignore_rule (id, kind, value, created_at) VALUES (?, ?, ?, 0)")
            .bind("bad")
            .bind("by_magic")
            .bind("xxx")
            .execute(&pool)
            .await
            .expect("insert");

        let rules = load_rules(&pool).await.expect("load_rules");
        // 未识别 kind 被跳过，只剩默认规则
        assert!(!rules.iter().any(|r| r.value == "xxx"));
        assert!(rules.iter().any(|r| r.value == ".DS_Store"));
    }

    // ---------- filter_events：管道行为 ----------

    #[tokio::test]
    async fn filter_events_drops_ignored_and_passes_rest() {
        let (in_tx, in_rx) = mpsc::channel::<WatchEvent>(16);
        let (out_tx, mut out_rx) = mpsc::channel::<WatchEvent>(16);
        let rules = Arc::new(RwLock::new(default_rules()));

        let _h = filter_events(in_rx, out_tx, rules);

        let mk = |p: &str| WatchEvent {
            kind: crate::watch::WatchEventKind::Created,
            path: PathBuf::from(p),
            new_path: None,
            watch_dir_id: "wd".to_string(),
            at: 0,
        };

        in_tx.send(mk("/a/.DS_Store")).await.unwrap();
        in_tx.send(mk("/a/report.pdf")).await.unwrap();
        in_tx.send(mk("/a/x.tmp")).await.unwrap();
        in_tx.send(mk("/a/main.rs")).await.unwrap();
        drop(in_tx); // 关闭输入，让 filter 退出

        let mut got = Vec::new();
        while let Some(ev) = out_rx.recv().await {
            got.push(ev.path);
        }
        assert_eq!(got.len(), 2);
        assert!(got.contains(&PathBuf::from("/a/report.pdf")));
        assert!(got.contains(&PathBuf::from("/a/main.rs")));
    }

    #[tokio::test]
    async fn filter_events_hot_reload_rules() {
        let (in_tx, in_rx) = mpsc::channel::<WatchEvent>(16);
        let (out_tx, mut out_rx) = mpsc::channel::<WatchEvent>(16);
        let rules = Arc::new(RwLock::new(Vec::new())); // 初始无规则

        let rules_for_writer = Arc::clone(&rules);
        let _h = filter_events(in_rx, out_tx, rules);

        let mk = |p: &str| WatchEvent {
            kind: crate::watch::WatchEventKind::Created,
            path: PathBuf::from(p),
            new_path: None,
            watch_dir_id: "wd".to_string(),
            at: 0,
        };

        // 阶段 1：无规则，.psd 应通过
        in_tx.send(mk("/a/x.psd")).await.unwrap();
        let ev = out_rx.recv().await.expect("应收到 .psd");
        assert_eq!(ev.path, PathBuf::from("/a/x.psd"));

        // 阶段 2：写入规则，.psd 应被丢弃
        {
            let mut g = rules_for_writer.write().unwrap();
            g.push(rule(IgnoreKind::ByExt, ".psd"));
        }
        in_tx.send(mk("/a/y.psd")).await.unwrap();
        in_tx.send(mk("/a/y.pdf")).await.unwrap();
        let ev = out_rx.recv().await.expect("应收到 .pdf");
        assert_eq!(ev.path, PathBuf::from("/a/y.pdf"));
        // 不应再有 .psd
        drop(in_tx);
        while let Some(ev) = out_rx.recv().await {
            assert_ne!(ev.path, PathBuf::from("/a/y.psd"));
        }
    }

    // ---------- filter_events：改名事件 from 被忽略但 to 未忽略 → 改写 Created(to) 放行 ----------

    #[tokio::test]
    async fn filter_rename_from_ignored_to_kept_rewrites_to_created() {
        // Chrome 下载完成改名（Windows RenameMode::Both 形态）：
        // from=.crdownload 命中默认规则，to=正式名未命中。
        // 若整条丢弃，正式文件永远无通知；应改写为 Created(to) 放行。
        let (in_tx, in_rx) = mpsc::channel::<WatchEvent>(16);
        let (out_tx, mut out_rx) = mpsc::channel::<WatchEvent>(16);
        let rules = Arc::new(RwLock::new(default_rules()));

        let _h = filter_events(in_rx, out_tx, rules);

        in_tx
            .send(WatchEvent {
                kind: crate::watch::WatchEventKind::Renamed,
                path: PathBuf::from("/dl/report.zip.crdownload"),
                new_path: Some(PathBuf::from("/dl/report.zip")),
                watch_dir_id: "wd".to_string(),
                at: 42,
            })
            .await
            .unwrap();
        drop(in_tx);

        let ev = out_rx.recv().await.expect("应放行改写后的事件");
        assert_eq!(ev.kind, crate::watch::WatchEventKind::Created);
        assert_eq!(ev.path, PathBuf::from("/dl/report.zip"));
        assert_eq!(ev.new_path, None);
        assert_eq!(ev.watch_dir_id, "wd");
        assert_eq!(ev.at, 42, "改写保留原始时间戳");
        assert!(out_rx.recv().await.is_none(), "只应放行 1 条");
    }

    #[tokio::test]
    async fn filter_rename_both_paths_ignored_is_dropped() {
        // from 和 to 都命中忽略规则（如 .crdownload → .tmp）→ 整条丢弃
        let (in_tx, in_rx) = mpsc::channel::<WatchEvent>(16);
        let (out_tx, mut out_rx) = mpsc::channel::<WatchEvent>(16);
        let rules = Arc::new(RwLock::new(default_rules()));

        let _h = filter_events(in_rx, out_tx, rules);

        in_tx
            .send(WatchEvent {
                kind: crate::watch::WatchEventKind::Renamed,
                path: PathBuf::from("/dl/a.crdownload"),
                new_path: Some(PathBuf::from("/dl/a.tmp")),
                watch_dir_id: "wd".to_string(),
                at: 1,
            })
            .await
            .unwrap();
        drop(in_tx);

        assert!(out_rx.recv().await.is_none(), "双侧都忽略应整条丢弃");
    }

    #[tokio::test]
    async fn filter_rename_without_new_path_from_ignored_is_dropped() {
        // from 命中忽略、无 new_path（Any 单侧）→ 丢弃
        //（to 路径在 macOS FSEvents 下有自己的独立事件，不受影响）
        let (in_tx, in_rx) = mpsc::channel::<WatchEvent>(16);
        let (out_tx, mut out_rx) = mpsc::channel::<WatchEvent>(16);
        let rules = Arc::new(RwLock::new(default_rules()));

        let _h = filter_events(in_rx, out_tx, rules);

        in_tx
            .send(WatchEvent {
                kind: crate::watch::WatchEventKind::Renamed,
                path: PathBuf::from("/dl/a.crdownload"),
                new_path: None,
                watch_dir_id: "wd".to_string(),
                at: 1,
            })
            .await
            .unwrap();
        drop(in_tx);

        assert!(out_rx.recv().await.is_none());
    }
}
