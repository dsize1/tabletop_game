//! 开发文档预览资源 API（docs-mcp 授权链接的读取端）。
//!
//! - `GET /resources/docs/tree?token=xxx`：返回全部配置根的文件树 JSON。
//! - `GET /resources/docs/file?token=xxx&root=<name>&path=<rel>`：返回 `.md` 内容。
//!
//! 安全：token 校验 + 资源范围必须为 `docs:all`；root 白名单（来自配置）；
//! 路径规范化 + 前缀校验防穿越；exclude 目录过滤；仅 `.md` 可读。

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::AppState;
use shared::docs_config::{is_excluded, is_md_name, is_safe_rel, normalize_rel, DocsConfig};

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileQuery {
    pub token: Option<String>,
    pub root: Option<String>,
    pub path: Option<String>,
}

/// 文件树节点（path 为相对所属根的 `/` 分隔路径）。
#[derive(Debug, Serialize)]
pub struct TreeNode {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    md: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<TreeNode>>,
}

/// 一个文档根的子树。
#[derive(Debug, Serialize)]
pub struct RootTree {
    name: String,
    path: String,
    children: Vec<TreeNode>,
}

fn project_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// 校验 token 且资源范围必须为 `docs:all`。无效 401 / 越权 403。
async fn verify_docs_token(state: &AppState, token: Option<&str>) -> Result<(), StatusCode> {
    let token = token.ok_or(StatusCode::UNAUTHORIZED)?;
    let rec = state.auth.verify(token).await.map_err(|e| match e {
        auth::AuthError::Invalid => StatusCode::UNAUTHORIZED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    if rec.resource != "docs:all" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

/// 递归构建目录树；返回 None 表示目录下无任何可展示条目（空目录不显示）。
fn build_dir(
    cfg: &DocsConfig,
    root_path: &str,
    abs_dir: &Path,
    rel_dir: &str,
    depth: usize,
) -> Option<Vec<TreeNode>> {
    if depth > 16 {
        return None;
    }
    let entries = std::fs::read_dir(abs_dir).ok()?;
    let mut dirs: Vec<TreeNode> = Vec::new();
    let mut files: Vec<TreeNode> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let rel = if rel_dir.is_empty() {
            name.clone()
        } else {
            format!("{rel_dir}/{name}")
        };
        // exclude 以「仓库根相对路径」匹配
        let repo_rel = format!("{root_path}/{rel}");
        if is_excluded(&repo_rel, &cfg.exclude) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if let Some(children) = build_dir(cfg, root_path, &entry.path(), &rel, depth + 1) {
                if !children.is_empty() {
                    dirs.push(TreeNode {
                        name,
                        kind: "dir",
                        path: rel,
                        md: None,
                        children: Some(children),
                    });
                }
            }
        } else if file_type.is_file() {
            let md = is_md_name(&name);
            files.push(TreeNode {
                name,
                kind: "file",
                path: rel,
                md: Some(md),
                children: None,
            });
        }
    }
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    if dirs.is_empty() {
        None
    } else {
        Some(dirs)
    }
}

/// 文件树 JSON（全部配置根）。
pub async fn docs_tree(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
) -> Result<Json<Vec<RootTree>>, StatusCode> {
    verify_docs_token(&state, q.token.as_deref()).await?;
    let project = project_root();
    let mut roots = Vec::new();
    for root in &state.docs.roots {
        let abs = project.join(&root.path);
        if !abs.is_dir() {
            continue;
        }
        let children =
            build_dir(&state.docs, &root.path, &abs, "", 0).unwrap_or_default();
        roots.push(RootTree {
            name: root.name.clone(),
            path: root.path.clone(),
            children,
        });
    }
    Ok(Json(roots))
}

/// `.md` 文件内容（text/markdown）。
pub async fn docs_file(
    State(state): State<AppState>,
    Query(q): Query<FileQuery>,
) -> Result<Response, StatusCode> {
    verify_docs_token(&state, q.token.as_deref()).await?;
    let root_name = q.root.ok_or(StatusCode::BAD_REQUEST)?;
    // 第一道防线：相对路径语义安全（非绝对、无 `..`）
    let raw_path = q.path.as_deref().unwrap_or("");
    if !is_safe_rel(raw_path) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let rel = normalize_rel(raw_path);

    let root = state
        .docs
        .roots
        .iter()
        .find(|r| r.name == root_name)
        .ok_or(StatusCode::FORBIDDEN)?;

    let repo_rel = format!("{}/{}", root.path, rel);
    if is_excluded(&repo_rel, &state.docs.exclude) {
        return Err(StatusCode::FORBIDDEN);
    }

    let project = project_root();
    let base = project.join(&root.path);
    let candidate = base.join(&rel);

    // 路径穿越防护：规范化后必须仍在根目录内（挡 `..`、绝对路径、symlink 逃逸）
    let base_canon = std::fs::canonicalize(&base).map_err(|_| StatusCode::NOT_FOUND)?;
    let cand_canon = std::fs::canonicalize(&candidate).map_err(|_| StatusCode::NOT_FOUND)?;
    if !cand_canon.starts_with(&base_canon) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !cand_canon.is_file() || !is_md_name(&cand_canon.to_string_lossy()) {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    let content = tokio::fs::read_to_string(&cand_canon)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        content,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_rejected_by_safe_rel() {
        assert!(!is_safe_rel("../../etc/passwd"));
        assert!(!is_safe_rel("..\\..\\secret.md"));
        assert!(is_safe_rel("a/b.md"));
    }

    #[test]
    fn build_dir_skips_excluded_and_dotfiles() {
        let tmp = std::env::temp_dir().join(format!("docstest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("a.md"), "x").unwrap();
        std::fs::write(tmp.join("b.txt"), "x").unwrap();
        std::fs::write(tmp.join(".hidden"), "x").unwrap();
        std::fs::create_dir_all(tmp.join("artifacts")).unwrap();
        std::fs::write(tmp.join("artifacts/big.jsonl"), "x").unwrap();

        let cfg = DocsConfig {
            roots: vec![],
            exclude: vec!["rootpath/artifacts".to_string()],
        };
        let children = build_dir(&cfg, "rootpath", &tmp, "", 0).unwrap();
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"a.md"));
        assert!(names.contains(&"b.txt"));
        assert!(!names.contains(&".hidden"));
        assert!(!names.contains(&"artifacts"));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
