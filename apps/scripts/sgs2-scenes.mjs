#!/usr/bin/env node
// 生成人工抽检用的「可复现场景」网页（自包含 HTML，双击即可在浏览器打开）。
//
// 数据来源：`cargo run -p sanguosha2 --example sgs2-scenes` 输出的 JSON（真实跑一遍新内核）。
// 产物：
//   docs/games/sanguosha/scenes/scenes.json   机器可读
//   docs/games/sanguosha/scenes/index.html    人读（步骤 / pending / 日志 / 断言）
//
// 用法：node apps/scripts/sgs2-scenes.mjs [--check]
//   --check：只校验全绿（CI 用），不写文件。

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const checkOnly = process.argv.includes("--check");
const OUT_DIR = resolve("docs", "games", "sanguosha", "scenes");

const raw = execFileSync(
  "cargo",
  ["run", "-q", "--manifest-path", "crates/Cargo.toml", "-p", "sanguosha2", "--example", "sgs2-scenes"],
  { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
const data = JSON.parse(raw.trim());

if (!data.all_ok) {
  console.error("[scenes] 场景断言失败：");
  for (const sc of data.scenes) {
    for (const c of sc.checks) if (!c.ok) console.error(`  ✗ ${sc.id} · ${c.desc}`);
  }
  process.exit(1);
}
if (checkOnly) {
  console.log(`[scenes] OK：${data.scenes.length} 个场景全绿（${data.scenes.map((s) => s.id).join(" / ")}）`);
  process.exit(0);
}

const esc = (s) =>
  String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

function renderPending(p) {
  if (!p) return `<div class="muted">（无 pending）</div>`;
  const opts = (p.options ?? []).map((o) => `<code>${esc(o)}</code>`).join(" ");
  return `<div><b>${esc(p.skill ?? "-")}</b> · 等待 <code>${esc(p.actor)}</code>${p.escape ? ` · 出口 <code>${esc(p.escape)}</code>` : ""}</div><div class="opts">${opts}</div>`;
}

const sceneHtml = data.scenes
  .map(
    (sc) => `
  <section class="scene ${sc.ok ? "ok" : "bad"}">
    <h2>${sc.ok ? "✔" : "✘"} ${esc(sc.name)} <span class="id">${esc(sc.id)}</span></h2>
    <ul class="checks">${sc.checks.map((c) => `<li>${c.ok ? "✔" : "✘"} ${esc(c.desc)}</li>`).join("")}</ul>
    <ol class="steps">
      ${sc.steps
        .map(
          (st) => `
        <li>
          <div class="row"><b>${esc(st.label)}</b>
            ${st.action ? `<span class="act">${esc(st.actor)} → <code>${esc(st.action)}</code></span>` : ""}
            ${st.error ? `<span class="err">错误：${esc(st.error)}</span>` : ""}
          </div>
          <div class="cols">
            <div><div class="k">动作前 pending</div>${renderPending(st.pending_before)}</div>
            <div><div class="k">动作后 pending</div>${renderPending(st.pending_after)}</div>
          </div>
          <div class="cols">
            <div><div class="k">状态</div>
              <div class="muted">回合 ${esc(st.state.turn)} · 阶段 ${esc(st.state.phase)} · 体力 ${esc(st.state.hp.join("/"))} · 手牌 ${esc(st.state.hands.join("/"))} · void ${esc(st.state.void)} · 弃牌堆 ${esc(st.state.discard)}</div>
            </div>
            <div><div class="k">日志 / 展示</div>
              <div class="muted">${(st.log ?? []).map(esc).join("<br>")}${st.presentation?.length ? "<br>— " + st.presentation.map(esc).join("<br>— ") : ""}</div>
            </div>
          </div>
        </li>`,
        )
        .join("")}
    </ol>
  </section>`,
  )
  .join("\n");

const html = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>三国杀 · 内核 v2 抽检场景（B0/B1）</title>
<style>
  :root { color-scheme: light dark; }
  body { font: 14px/1.6 -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif; margin: 0; padding: 24px; background:#faf9f7; color:#1d1b19; }
  @media (prefers-color-scheme: dark) { body { background:#16161a; color:#e6e6ea; } .scene{background:#1e1e24;border-color:#333} code{background:#2a2a33} }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .sub { color:#777; margin-bottom: 20px; }
  .scene { background:#fff; border:1px solid #e6e2dd; border-radius:10px; padding:16px 18px; margin:0 0 18px; }
  .scene.bad { border-color:#d9534f; }
  .scene h2 { font-size:16px; margin:0 0 8px; }
  .id { color:#999; font-weight:400; font-size:12px; margin-left:6px; }
  .checks { margin:8px 0 12px; padding-left:18px; }
  .checks li { list-style:none; }
  .checks li::before { content:""; }
  .steps { padding-left: 18px; }
  .steps > li { margin-bottom:14px; }
  .row { display:flex; gap:10px; align-items:baseline; flex-wrap:wrap; }
  .act { color:#0a7a3f; }
  .err { color:#c0392b; }
  .cols { display:grid; grid-template-columns:1fr 1fr; gap:12px; margin-top:6px; }
  @media (max-width: 820px){ .cols{ grid-template-columns:1fr; } }
  .k { font-size:12px; color:#888; margin-bottom:2px; }
  .muted { color:#666; }
  .opts { margin-top:4px; display:flex; gap:6px; flex-wrap:wrap; }
  code { background:#f1efec; padding:1px 5px; border-radius:4px; font-size:12px; }
</style></head>
<body>
  <h1>三国杀 · 内核 v2 抽检场景</h1>
  <div class="sub">由 <code>sgs2-scenes</code> 真实执行新内核生成（${esc(data.generated_at)}）·
    ${data.all_ok ? "全部断言通过" : "存在失败"} ·
    共 ${data.scenes.length} 个场景 · 逐步骤展示 pending / 状态 / 日志</div>
  ${sceneHtml}
</body></html>`;

mkdirSync(OUT_DIR, { recursive: true });
writeFileSync(resolve(OUT_DIR, "scenes.json"), JSON.stringify(data, null, 2) + "\n");
writeFileSync(resolve(OUT_DIR, "index.html"), html);
console.log(`[scenes] 已生成：docs/games/sanguosha/scenes/index.html（${data.scenes.length} 场景，全绿）`);
