#!/usr/bin/env node
// V3 内核机制门禁：`align/mechanics.json`（46 条机制词表）↔ 实现锚点 ↔ 用例锚点。
//
// 背景（2026-11-15 用户裁定）：「B0 前置项 P1–P6 已完成」被误读为「内核完备」，
// 实际缺 20 条机制（pindian / hand-freeze / virtual-card / armor-points …）。
// 本门禁把「机制完备」变成可机器判定的事实：每条机制必须有实现锚点 + 用例锚点，
// 锚点符号必须真实存在（防手写 typo / 改名漏改），并生成人读审计表。
//
// 用法：
//   node apps/scripts/check-sgs3-kernel.mjs            # 报告缺口（不失败）
//   node apps/scripts/check-sgs3-kernel.mjs --strict    # 有缺口即退出 1（接入 check:sgs3）
//   node apps/scripts/check-sgs3-kernel.mjs --write     # 重新生成 v3-kernel-audit.md
//   node apps/scripts/check-sgs3-kernel.mjs --check     # 校验审计表是否最新 + 锚点存在
//
// 退出码：0 = 全部锚点有效；1 = 有缺口/锚点失效/映射与词表不一致。

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { MECHANICS } from "./lib/sgs3-mechanics.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const SRC = path.join(root, "crates/engines/sanguosha3/src");
const TESTS = path.join(root, "crates/engines/sanguosha3/tests");
const MECHANICS_JSON = path.join(root, "docs/games/sanguosha/align/mechanics.json");
const AUDIT_MD = path.join(root, "docs/games/sanguosha/v3-kernel-audit.md");

const args = new Set(process.argv.slice(2));

const vocab = JSON.parse(readFileSync(MECHANICS_JSON, "utf8"));
const vocabIds = vocab.mechanics.map((m) => m.id);
const vocabName = new Map(vocab.mechanics.map((m) => [m.id, m.name]));

const failures = [];

// 映射 ↔ 词表一致（双向）。
for (const id of vocabIds) {
  if (!MECHANICS[id]) failures.push(`机制 ${id} 未在 apps/scripts/lib/sgs3-mechanics.mjs 登记`);
}
for (const id of Object.keys(MECHANICS)) {
  if (!vocabIds.includes(id)) failures.push(`映射里的机制 ${id} 不在 align/mechanics.json（未知机制 id）`);
}

/** 锚点符号是否存在（fn/struct/enum/const/type/模式）。 */
function symbolExists(file, symbol) {
  if (!existsSync(file)) return { ok: false, why: `文件不存在：${path.relative(root, file)}` };
  const src = readFileSync(file, "utf8");
  const leaf = symbol.split("::").pop();
  const re = new RegExp(
    `\\b(fn|struct|enum|const|type|static|mod|union)\\s+${leaf.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`,
  );
  if (re.test(src)) return { ok: true };
  // 变体/字段锚点（Zone::Mark 之类）：退化为「符号作为标识符出现」。
  const ident = new RegExp(`\\b${leaf.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`);
  if (ident.test(src)) return { ok: true };
  return { ok: false, why: `符号未找到：${symbol}` };
}

function checkAnchor(anchor, base) {
  const [rel, symbol] = anchor.split("::");
  if (!rel || !symbol) return { ok: false, why: `锚点格式应为 <文件>::<符号>：${anchor}` };
  if (!rel.endsWith(".rs")) return { ok: false, why: `锚点必须指向 .rs 文件：${anchor}` };
  return symbolExists(path.join(base, rel), symbol);
}

const rows = [];
for (const id of vocabIds) {
  const m = MECHANICS[id];
  if (!m) continue;
  const impl = m.impl ?? [];
  const tests = m.tests ?? [];
  const status = impl.length && tests.length ? "✅" : impl.length ? "🟡" : "❌";
  for (const a of impl) {
    const r = checkAnchor(a, SRC);
    if (!r.ok) failures.push(`[${id}] impl 锚点失效：${a}（${r.why}）`);
  }
  for (const a of tests) {
    const r = checkAnchor(a, path.join(root, "crates/engines/sanguosha3"));
    if (!r.ok) failures.push(`[${id}] test 锚点失效：${a}（${r.why}）`);
  }
  rows.push({ id, name: vocabName.get(id) ?? id, status, impl, tests, note: m.note ?? "" });
}

const done = rows.filter((r) => r.status === "✅").length;
const partial = rows.filter((r) => r.status === "🟡").length;
const missing = rows.filter((r) => r.status === "❌").length;
const gaps = rows.filter((r) => r.status !== "✅");

function renderDoc() {
  const lines = [];
  lines.push("# V3 内核机制完备审计（`align/mechanics.json` 46 条）");
  lines.push("");
  lines.push("> **生成物，勿手改**：`npm run check:sgs3-kernel -- --write`。");
  lines.push("> 事实源：`apps/scripts/lib/sgs3-mechanics.mjs`（机制 → 实现锚点 + 用例锚点）。");
  lines.push("> 门禁：`npm run check:sgs3-kernel`（锚点存在性 + 词表双向一致）。");
  lines.push("");
  lines.push("## 为什么有这份表（2026-11-15 用户裁定）");
  lines.push("");
  lines.push("`B0 前置项 P1–P6` 只覆盖「回合/事件/交互/卡牌/装备/生命周期」的**基础流程**，");
  lines.push("不等于「46 条机制词表全部进内核」。上一轮把「P1–P6 完成」当成「内核完备」并开始武将批次，");
  lines.push("属于**严重漂移**。V3 的口径是：**内核未完备不开始武将批次**；本表就是完备判据。");
  lines.push("");
  lines.push("## 汇总");
  lines.push("");
  lines.push(`- 词表机制：**${rows.length}**`);
  lines.push(`- ✅ 已具备（实现 + 用例锚点）：**${done}**`);
  lines.push(`- 🟡 部分（只有实现锚点）：**${partial}**`);
  lines.push(`- ❌ 缺失：**${missing}**`);
  lines.push("");
  lines.push("## 明细");
  lines.push("");
  lines.push("| # | 机制 | 名称 | 状态 | 实现锚点 | 用例锚点 | 口径要点 |");
  lines.push("|---|---|---|---|---|---|---|");
  rows.forEach((r, i) => {
    const impl = r.impl.length ? r.impl.map((a) => `\`${a}\``).join("<br>") : "—";
    const tests = r.tests.length ? r.tests.map((a) => `\`${a}\``).join("<br>") : "—";
    lines.push(`| ${i + 1} | \`${r.id}\` | ${r.name} | ${r.status} | ${impl} | ${tests} | ${r.note.replace(/\|/g, "\\|")} |`);
  });
  lines.push("");
  if (gaps.length) {
    lines.push("## 缺口清单（内核完备前不得开工武将批次）");
    lines.push("");
    for (const r of gaps) lines.push(`- ${r.status} \`${r.id}\`（${r.name}）：${r.note}`);
    lines.push("");
  }
  lines.push("## 机制词表来源");
  lines.push("");
  lines.push(`- \`docs/games/sanguosha/align/mechanics.json\` version=${vocab.version} updated_at=${vocab.updated_at}`);
  lines.push("- 机制定义/不变量变更必须走该文件（bump version 会让已确认技能进入冲突阻断）。");
  lines.push("");
  return lines.join("\n");
}

if (failures.length) {
  console.error("❌ V3 内核机制门禁失败：");
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}

if (args.has("--write")) {
  writeFileSync(AUDIT_MD, renderDoc(), "utf8");
  console.log(`已生成 ${path.relative(root, AUDIT_MD)}`);
}

if (args.has("--check")) {
  if (!existsSync(AUDIT_MD)) {
    console.error(`❌ 缺少 ${path.relative(root, AUDIT_MD)}（先跑 --write）`);
    process.exit(1);
  }
  const cur = readFileSync(AUDIT_MD, "utf8");
  if (cur !== renderDoc()) {
    console.error(`❌ ${path.relative(root, AUDIT_MD)} 不是最新（跑 npm run check:sgs3-kernel -- --write）`);
    process.exit(1);
  }
}

// 人读报告
console.log(`V3 内核机制：${rows.length} 条 → ✅ ${done} / 🟡 ${partial} / ❌ ${missing}`);
for (const r of gaps) console.log(`  ${r.status} ${r.id} — ${r.name}：${r.note}`);

const strict = args.has("--strict");
if (strict && gaps.length) {
  console.error(`❌ 内核不完备：${gaps.length} 条机制缺实现/用例锚点（内核完备前不得开工武将批次）`);
  process.exit(1);
}
