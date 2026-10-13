// markdown(GFM) → HTML 渲染。
//
// 本期在 web 端渲染（先 markdownToHtml）；后续 game/dev 引擎将以 MCP 能力
// 暴露统一的「web 渲染原语」，届时此模块迁移为调用统一原语。

import { marked } from "marked";

/** 文档树节点（path 为相对所属根的 `/` 分隔路径）。 */
export interface DocsTreeNode {
  name: string;
  type: "dir" | "file";
  path: string;
  md?: boolean;
  children?: DocsTreeNode[];
}

/** 一个文档根的子树。 */
export interface DocsRootTree {
  name: string;
  path: string;
  children: DocsTreeNode[];
}

const ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESCAPE_MAP[c] ?? c);
}

/** 渲染 markdown（GFM：表格 / 复选框 / 代码块）。 */
export function renderMarkdown(md: string): string {
  return marked.parse(md, { gfm: true, breaks: false, async: false }) as string;
}

function encodeQuery(s: string): string {
  return encodeURIComponent(s);
}

function renderNodes(nodes: DocsTreeNode[], rootName: string, token: string): string {
  const parts: string[] = [];
  for (const node of nodes) {
    if (node.type === "dir") {
      const inner = node.children?.length ? renderNodes(node.children, rootName, token) : "";
      parts.push(
        `<details class="dir"><summary>${escapeHtml(node.name)}</summary>` +
          `<div class="children">${inner}</div></details>`,
      );
    } else if (node.md) {
      const href =
        `/dev_docs/file?token=${encodeQuery(token)}` +
        `&root=${encodeQuery(rootName)}&path=${encodeQuery(node.path)}`;
      parts.push(`<div class="file"><a href="${href}">${escapeHtml(node.name)}</a></div>`);
    } else {
      // 仅 .md 可导航预览；其它类型只展示名称
      parts.push(`<div class="file muted">${escapeHtml(node.name)}</div>`);
    }
  }
  return parts.join("");
}

/** 文件树 body HTML：原生 <details> 折叠，.md 可点、其它仅显示名称。 */
export function renderDocsTree(roots: DocsRootTree[], token: string): string {
  if (!roots.length) return `<p class="error">暂无可用文档根</p>`;
  return roots
    .map(
      (r) =>
        `<details class="root" open><summary>${escapeHtml(r.name)}</summary>` +
        `<div class="children">${renderNodes(r.children ?? [], r.name, token)}</div></details>`,
    )
    .join("");
}

/** 包裹为独立 HTML 页（移动端优先）。 */
export function renderPage(title: string, bodyHtml: string): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<style>
  :root { color-scheme: light dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
      "Microsoft YaHei", sans-serif;
    line-height: 1.6;
    background: #f7f8fa;
    color: #24292f;
  }
  .container { max-width: 860px; margin: 0 auto; padding: 20px 16px 48px; }
  .markdown-body {
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 10px;
    padding: 20px 24px;
    overflow-wrap: break-word;
  }
  .markdown-body h1, .markdown-body h2, .markdown-body h3 {
    border-bottom: 1px solid #e5e7eb; padding-bottom: .3em; margin-top: 1.2em;
  }
  .markdown-body h1 { font-size: 1.6em; }
  .markdown-body h2 { font-size: 1.35em; }
  .markdown-body h3 { font-size: 1.15em; }
  .markdown-body pre {
    background: #f6f8fa; padding: 12px; border-radius: 8px; overflow-x: auto;
  }
  .markdown-body code {
    background: #f6f8fa; padding: .15em .4em; border-radius: 4px;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: .9em;
  }
  .markdown-body pre code { background: none; padding: 0; }
  .markdown-body table { border-collapse: collapse; width: 100%; margin: 1em 0; }
  .markdown-body th, .markdown-body td {
    border: 1px solid #d0d7de; padding: 6px 12px; text-align: left;
  }
  .markdown-body th { background: #f6f8fa; }
  .markdown-body blockquote {
    margin: 1em 0; padding: .1em 1em; color: #57606a;
    border-left: 4px solid #d0d7de; background: #f6f8fa;
  }
  .markdown-body input[type="checkbox"] { margin-right: 6px; }
  .tree {
    background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
    padding: 12px 18px;
  }
  .tree details { margin: 2px 0; }
  .tree summary { cursor: pointer; padding: 2px 0; font-weight: 600; }
  .tree .children { margin-left: 18px; }
  .tree .file { padding: 2px 0; }
  .tree .file a { color: #0969da; text-decoration: none; }
  .tree .file a:hover { text-decoration: underline; }
  .tree .file.muted { color: #8b949e; }
  .breadcrumb { margin-bottom: 12px; font-size: .9em; color: #57606a; }
  .breadcrumb a { color: #0969da; text-decoration: none; }
  .breadcrumb a:hover { text-decoration: underline; }
  .error { text-align: center; padding: 60px 20px; color: #57606a; }
  .error h1 { font-size: 4em; margin: 0; color: #cf222e; }
  @media (prefers-color-scheme: dark) {
    body { background: #0d1117; color: #c9d1d9; }
    .markdown-body { background: #161b22; border-color: #30363d; }
    .markdown-body h1, .markdown-body h2, .markdown-body h3 { border-color: #30363d; }
    .markdown-body pre, .markdown-body code { background: #0d1117; }
    .markdown-body th { background: #21262d; }
    .markdown-body th, .markdown-body td { border-color: #30363d; }
    .markdown-body blockquote { color: #8b949e; border-color: #30363d; background: #161b22; }
    .tree { background: #161b22; border-color: #30363d; }
    .tree .file a, .breadcrumb a { color: #58a6ff; }
    .tree .file.muted, .breadcrumb { color: #8b949e; }
  }
</style>
</head>
<body>
<main class="container">
<div class="markdown-body">
${bodyHtml}
</div>
</main>
</body>
</html>`;
}

/** 统一错误页（401/403/404/410）。 */
export function renderErrorPage(status: number, message: string): string {
  return renderPage(
    `${status}`,
    `<div class="error"><h1>${status}</h1><p>${escapeHtml(message)}</p></div>`,
  );
}
