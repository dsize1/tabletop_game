// web 文档树渲染纯函数单测（无需起服务）。
// 运行：npx tsx test-render.mts
import assert from "node:assert";
import { renderDocsTree } from "./src/render.js";

const roots = [
  {
    name: "docs",
    path: "docs",
    children: [
      {
        name: "conventions",
        type: "dir" as const,
        path: "conventions",
        children: [
          {
            name: "mcp-primitives.md",
            type: "file" as const,
            path: "conventions/mcp-primitives.md",
            md: true,
          },
        ],
      },
      { name: "notes.txt", type: "file" as const, path: "notes.txt", md: false },
    ],
  },
];

const html = renderDocsTree(roots, "tok-1");
assert.ok(html.includes('<details class="root" open><summary>docs</summary>'), "根节点应为打开的 <details>");
assert.ok(html.includes('<details class="dir"><summary>conventions</summary>'), "目录应为可折叠 <details>");
assert.ok(
  html.includes("/dev_docs/file?token=tok-1&root=docs&path=conventions%2Fmcp-primitives.md"),
  ".md 链接应携带 token + 编码后的 root/path",
);
assert.ok(html.includes('class="file muted">notes.txt'), "非 .md 应灰显且不可点");
assert.ok(!html.includes('href="/dev_docs/file?token=tok-1&root=docs&path=notes.txt"'), "非 .md 不应生成链接");

// 名称转义（防 XSS）
const xss = renderDocsTree(
  [
    {
      name: "x",
      path: "x",
      children: [{ name: "<img src=x onerror=alert(1)>", type: "file" as const, path: "a.md", md: false }],
    },
  ],
  "t",
);
assert.ok(xss.includes("&lt;img") && !xss.includes("<img"), "文件名应被 HTML 转义");

// 空根
assert.ok(renderDocsTree([], "t").includes("暂无可用文档根"), "无根时应给出提示");

console.log("test-render 全部通过 ✅");
