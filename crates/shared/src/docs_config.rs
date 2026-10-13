//! 开发文档预览配置（roots + exclude）读取。
//!
//! 配置文件默认 `config/dev-docs.json`，可用环境变量 `DOCS_CONFIG` 覆盖；
//! 由 docs-mcp 与 server 两处共读，保证「配置集中一处」。
//!
//! ```json
//! { "roots": [{ "name": "docs", "path": "docs" }], "exclude": ["memory_palace/artifacts"] }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 一个可浏览的文档根（name = 展示/资源标识，path = 仓库根相对路径）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocsRoot {
    pub name: String,
    pub path: String,
}

/// 文档预览配置：roots（可浏览根）+ exclude（排除目录，仓库根相对）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocsConfig {
    pub roots: Vec<DocsRoot>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            roots: vec![
                DocsRoot {
                    name: "docs".into(),
                    path: "docs".into(),
                },
                DocsRoot {
                    name: "memory_palace".into(),
                    path: "memory_palace".into(),
                },
                DocsRoot {
                    name: "prompts".into(),
                    path: ".pi/prompts".into(),
                },
                DocsRoot {
                    name: "skills".into(),
                    path: ".pi/skills".into(),
                },
            ],
            exclude: vec!["memory_palace/artifacts".into()],
        }
    }
}

/// 配置文件路径（`DOCS_CONFIG` 覆盖，默认 `config/dev-docs.json`）。
pub fn config_path() -> PathBuf {
    std::env::var("DOCS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/dev-docs.json"))
}

/// 读取配置；文件不存在或解析失败时回退内置默认（打印到 stderr）。
pub fn load() -> DocsConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<DocsConfig>(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "[docs-config] 解析 {} 失败，回退默认配置：{e}",
                    path.display()
                );
                DocsConfig::default()
            }
        },
        Err(_) => DocsConfig::default(),
    }
}

/// 统一为 `/` 分隔的相对路径（Windows 反斜杠归一，去首尾斜杠）。
pub fn normalize_rel(p: &str) -> String {
    p.replace('\\', "/").trim_matches('/').to_string()
}

/// 判断仓库根相对路径是否命中排除目录（前缀匹配，含目录本身）。
pub fn is_excluded(rel: &str, exclude: &[String]) -> bool {
    let rel = normalize_rel(rel);
    exclude.iter().any(|e| {
        let e = normalize_rel(e);
        !e.is_empty() && (rel == e || rel.starts_with(&format!("{e}/")))
    })
}

/// 相对路径是否安全：非空、非绝对（Unix `/`、Windows 盘符/UNC）、无 `..` 路径段、无 NUL。
/// 作为 server 资源 API 的第一道防线（canonicalize 前缀校验为第二道）。
pub fn is_safe_rel(rel: &str) -> bool {
    let raw = rel.trim();
    if raw.is_empty() || raw.contains('\0') {
        return false;
    }
    // 绝对路径：Unix 前导 `/` 或 `\`；Windows 盘符 / UNC 含 `:`
    if raw.starts_with('/') || raw.starts_with('\\') || raw.contains(':') {
        return false;
    }
    let n = normalize_rel(raw);
    if n.is_empty() {
        return false;
    }
    !n.split('/').any(|seg| seg == ".." || seg.is_empty())
}

/// 是否为可预览的 markdown 文件名（`.md`，大小写不敏感）。
pub fn is_md_name(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_four_roots() {
        let c = DocsConfig::default();
        assert_eq!(c.roots.len(), 4);
        assert!(c.exclude.contains(&"memory_palace/artifacts".to_string()));
    }

    #[test]
    fn exclude_prefix_matching() {
        let ex = vec!["memory_palace/artifacts".to_string()];
        assert!(is_excluded("memory_palace/artifacts", &ex));
        assert!(is_excluded("memory_palace/artifacts/x.jsonl", &ex));
        assert!(is_excluded("memory_palace\\artifacts\\x.jsonl", &ex));
        assert!(!is_excluded("memory_palace/plan/a.md", &ex));
        assert!(!is_excluded("memory_palace/artifacts_extra", &ex));
    }

    #[test]
    fn normalize_backslashes() {
        assert_eq!(normalize_rel("a\\b\\c"), "a/b/c");
        assert_eq!(normalize_rel("/a/b/"), "a/b");
    }

    #[test]
    fn safe_rel_paths() {
        assert!(is_safe_rel("conventions/mcp-primitives.md"));
        assert!(is_safe_rel("a.md"));
        assert!(!is_safe_rel(""));
        assert!(!is_safe_rel("../secret.md"));
        assert!(!is_safe_rel("a/../../b.md"));
        assert!(!is_safe_rel("..\\..\\secret.md"));
        assert!(!is_safe_rel("/etc/passwd"));
        assert!(!is_safe_rel("C:/Windows/x.md"));
        assert!(!is_safe_rel("a//b.md"));
    }

    #[test]
    fn md_name_detection() {
        assert!(is_md_name("a.md"));
        assert!(is_md_name("A.MD"));
        assert!(is_md_name("a.b.md"));
        assert!(!is_md_name("a.txt"));
        assert!(!is_md_name("md"));
        assert!(!is_md_name("a"));
    }
}
