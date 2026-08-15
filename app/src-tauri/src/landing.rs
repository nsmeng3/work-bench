//! M3 落地子系统 · 落地路径计算领域逻辑（纯函数层）。
//!
//! 职责：给定引用类型 + 资源根目录 + 源文件名，计算建议目标路径；
//! 检测目标冲突；给出冲突改名建议。
//!
//! 本模块只交付纯函数，不接 Tauri 命令、不写库、不动文件系统。
//! 详细设计说明书 §2 `ManagedPlan{ proposedTarget, conflicts, ... }` 的前置依赖。
//!
//! 当前任务（3.1）只交付纯函数与单测，3.2 / 3.4 才会接入命令层，
//! 因此暂时允许 dead_code。

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// 6 个类型 → 子目录名映射（与详细设计 §2 一致：类型首字母大写，部分复数）。
///
/// | ref_type  | subdir      |
/// |-----------|-------------|
/// | code      | Code        |
/// | document  | Documents   |
/// | data      | Data        |
/// | artifact  | Artifacts   |
/// | tool      | Tools       |
/// | media     | Media       |
///
/// 未知类型返回 `COMMON_INVALID_PARAM`。
pub fn type_to_subdir(ref_type: &str) -> Result<&'static str, AppError> {
    match ref_type {
        "code" => Ok("Code"),
        "document" => Ok("Documents"),
        "data" => Ok("Data"),
        "artifact" => Ok("Artifacts"),
        "tool" => Ok("Tools"),
        "media" => Ok("Media"),
        other => Err(AppError::invalid_param(format!(
            "未知引用类型: {}",
            other
        ))),
    }
}

/// 计算建议目标路径：`{root_dir}/{TypeSubdir}/{source_name}`。
///
/// 纯路径拼接，不做存在性检查、不读写文件系统。
pub fn propose_target(
    root_dir: &Path,
    ref_type: &str,
    source_name: &str,
) -> Result<PathBuf, AppError> {
    let subdir = type_to_subdir(ref_type)?;
    Ok(root_dir.join(subdir).join(source_name))
}

/// 冲突改名建议：在 `existing_name` 基础上追加 `" (n)"` 直到不与 `taken` 中任何项冲突。
///
/// 规则：
/// - 从 `n = 2` 开始递增（`name` → `"name (2)"` → `"name (3)"` ...）。
/// - 对带扩展名的文件，插入到**主名之后、扩展名之前**：
///   `report.pdf` → `"report (2).pdf"`。
/// - 多段扩展名（如 `a.tar.gz`）按**最后一段**为扩展名处理：
///   `a.tar.gz` → `"a.tar (2).gz"`。
/// - 无扩展名（含以 `.` 开头的隐藏文件主名）直接在末尾追加：
///   `archive` → `"archive (2)"`。
/// - 若 `existing_name` 本身不在 `taken` 中，仍然返回 `"name (2)"` 形式
///   （调用方语义：仅在已确认冲突时调用）。
/// - `taken` 为空列表时，直接返回 `"name (2)"` 形式。
pub fn suggest_rename(existing_name: &str, taken: &[&str]) -> String {
    let (stem, ext) = split_stem_ext(existing_name);

    let mut n: u32 = 2;
    loop {
        let candidate = match ext {
            Some(e) => format!("{} ({}).{}", stem, n, e),
            None => format!("{} ({})", stem, n),
        };
        if !taken.iter().any(|t| *t == candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// 拆分为 (主名, 扩展名)。
///
/// - `"report.pdf"` → `("report", Some("pdf"))`
/// - `"a.tar.gz"`  → `("a.tar", Some("gz"))`（最后一段为扩展名）
/// - `"archive"`   → `("archive", None)`
/// - `".gitkeep"`  → `(".gitkeep", None)`（点前无字符视为无扩展名）
fn split_stem_ext(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        // 点在首位（隐藏文件）或不存在 → 无扩展名
        None | Some(0) => (name, None),
        Some(idx) => (&name[..idx], Some(&name[idx + 1..])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ---------- type_to_subdir ----------

    #[test]
    fn type_to_subdir_maps_all_six_types() {
        assert_eq!(type_to_subdir("code").unwrap(), "Code");
        assert_eq!(type_to_subdir("document").unwrap(), "Documents");
        assert_eq!(type_to_subdir("data").unwrap(), "Data");
        assert_eq!(type_to_subdir("artifact").unwrap(), "Artifacts");
        assert_eq!(type_to_subdir("tool").unwrap(), "Tools");
        assert_eq!(type_to_subdir("media").unwrap(), "Media");
    }

    #[test]
    fn type_to_subdir_rejects_unknown_type() {
        let err = type_to_subdir("unknown").unwrap_err();
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        let err = type_to_subdir("").unwrap_err();
        assert_eq!(err.code, "COMMON_INVALID_PARAM");

        // 大小写敏感：大写形式视为未知
        let err = type_to_subdir("Code").unwrap_err();
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- propose_target ----------

    #[test]
    fn propose_target_joins_root_subdir_name() {
        let root = Path::new("/home/user/resources");
        let p = propose_target(root, "document", "report.pdf").unwrap();
        assert_eq!(
            p,
            PathBuf::from("/home/user/resources/Documents/report.pdf")
        );
    }

    #[test]
    fn propose_target_works_for_all_types() {
        let root = Path::new("/r");
        assert_eq!(
            propose_target(root, "code", "main.rs").unwrap(),
            PathBuf::from("/r/Code/main.rs")
        );
        assert_eq!(
            propose_target(root, "data", "x.csv").unwrap(),
            PathBuf::from("/r/Data/x.csv")
        );
        assert_eq!(
            propose_target(root, "artifact", "a.bin").unwrap(),
            PathBuf::from("/r/Artifacts/a.bin")
        );
        assert_eq!(
            propose_target(root, "tool", "t.sh").unwrap(),
            PathBuf::from("/r/Tools/t.sh")
        );
        assert_eq!(
            propose_target(root, "media", "m.png").unwrap(),
            PathBuf::from("/r/Media/m.png")
        );
    }

    #[test]
    fn propose_target_propagates_unknown_type_error() {
        let root = Path::new("/r");
        let err = propose_target(root, "nope", "f.txt").unwrap_err();
        assert_eq!(err.code, "COMMON_INVALID_PARAM");
    }

    // ---------- suggest_rename ----------

    #[test]
    fn suggest_rename_no_conflict_returns_first_candidate() {
        // existing_name 不在 taken 中：仍然返回 "name (2)" 形式
        assert_eq!(suggest_rename("report.pdf", &[]), "report (2).pdf");
        assert_eq!(
            suggest_rename("report.pdf", &["other.pdf"]),
            "report (2).pdf"
        );
    }

    #[test]
    fn suggest_rename_single_conflict() {
        let taken = &["report.pdf", "report (2).pdf"];
        assert_eq!(suggest_rename("report.pdf", taken), "report (3).pdf");
    }

    #[test]
    fn suggest_rename_multiple_conflicts() {
        let taken = &[
            "report.pdf",
            "report (2).pdf",
            "report (3).pdf",
            "report (4).pdf",
        ];
        assert_eq!(suggest_rename("report.pdf", taken), "report (5).pdf");
    }

    #[test]
    fn suggest_rename_no_extension() {
        assert_eq!(suggest_rename("archive", &[]), "archive (2)");
        assert_eq!(
            suggest_rename("archive", &["archive (2)"]),
            "archive (3)"
        );
    }

    #[test]
    fn suggest_rename_multi_segment_extension() {
        // 固化决策：多段扩展名按"最后一段"处理 → "a.tar (2).gz"
        assert_eq!(suggest_rename("a.tar.gz", &[]), "a.tar (2).gz");
        assert_eq!(
            suggest_rename("a.tar.gz", &["a.tar (2).gz"]),
            "a.tar (3).gz"
        );
    }

    #[test]
    fn suggest_rename_empty_taken_list() {
        assert_eq!(suggest_rename("x.txt", &[]), "x (2).txt");
    }

    #[test]
    fn suggest_rename_hidden_file_treated_as_no_extension() {
        // `.gitkeep` 点前无字符 → 视为无扩展名
        assert_eq!(suggest_rename(".gitkeep", &[]), ".gitkeep (2)");
    }
}
