// markdown(GFM) → HTML 渲染。
//
// 本期在 web 端渲染（先 markdownToHtml）；后续 game/dev 引擎将以 MCP 能力
// 暴露统一的「web 渲染原语」，届时此模块迁移为调用统一原语。

import { marked } from "marked";

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
