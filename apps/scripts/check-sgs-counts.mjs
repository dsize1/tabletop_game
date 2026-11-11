#!/usr/bin/env node
// 门禁：**计数类事实**（将数 / 排除数 / 技能数 / 预设人数）必须与 catalog 一致。
//
// 背景（2026-11-06）：文档长期写「149 将 / 83 位排除」，实际为 **148 / 84**
// （「张昭张纮」与界版同名勘误后多排除 1 位）。手写计数是「文档漂移」的头号来源，
// 因此本项目约定：**任何人数/技能数都不得手写**，一律由脚本生成或由本门禁校验。
//
// 校验点：
//   1. `general-audit.md`：标题将数、默认选将池人数与算式、§4 排除人数；
//   2. `batch-playbook.md`：验证批次将数、排除人数；
//   3. `legal-actions.md`：`#武将包` 预设「可用人数去重后 = ...」列表；
//   4. 陈旧字面量扫描（仅当前态文档；历史记录类文档不扫）。
//
// 用法：node apps/scripts/check-sgs-counts.mjs [--verbose]

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { loadCatalog } from "./lib/sgs-catalog.mjs";

const verbose = process.argv.includes("--verbose") || process.argv.includes("-v");
const PACK_ORDER = ["standard", "wind_fire_forest_mountain", "yijiang", "jiexian"];

const DOCS = {
  audit: resolve("docs", "games", "sanguosha", "general-audit.md"),
  playbook: resolve("docs", "games", "sanguosha", "batch-playbook.md"),
  legal: resolve("docs", "games", "sanguosha", "legal-actions.md"),
};

/** 选将池 = 包过滤 → 同名去重（界优先），与 `lib.rs::prefer_jiexian` 同口径。 */
function poolOf(generals, mode) {
  const packs = new Set(mode.split("+"));
  const codes = generals.filter((g) => packs.has(g.pack));
  const jieNames = new Set(
    codes.filter((g) => g.pack === "jiexian").map((g) => g.name.replace(/^(界|将)/, "")),
  );
  // 候选名也要去掉「界/将」前缀再比对（否则「将华雄」不会被「界华雄」去重 → 多算 1 位）。
  return codes.filter(
    (g) => g.pack === "jiexian" || !jieNames.has(g.name.replace(/^(界|将)/, "")),
  );
}

/** 预设枚举（与 `lib.rs::general_pack_presets` 同序：按包数升序）。 */
function presetsOf() {
  const others = PACK_ORDER.filter((k) => k !== "standard");
  const out = [];
  for (let mask = 0; mask < 1 << others.length; mask++) {
    const combo = ["standard"];
    others.forEach((p, i) => {
      if (mask & (1 << i)) combo.push(p);
    });
    out.push(combo.join("+"));
  }
  return out.sort((a, b) => a.split("+").length - b.split("+").length);
}

const catalog = loadCatalog();
const generals = catalog.generals;
const audited = catalog.audited.length;
const excluded = catalog.excluded.length;
const total = generals.length;
const skills = new Set();
for (const g of catalog.audited) for (const s of g.skills) if (s.implemented) skills.add(s.code);

const presetCounts = presetsOf().map((mode) => poolOf(generals, mode).length);

const problems = [];
const info = [];
const fail = (msg) => problems.push(msg);
const expect = (text, re, want, what) => {
  const m = re.exec(text);
  if (!m) return fail(`${what}：未找到锚点 ${re}`);
  const got = m.slice(1).map(Number).filter((n) => !Number.isNaN(n));
  if (want.some((w, i) => got[i] !== w)) {
    fail(`${what}：文档 ${JSON.stringify(got)} ≠ catalog ${JSON.stringify(want)}`);
  } else if (verbose) {
    info.push(`${what}：${JSON.stringify(got)} ✓`);
  }
};

const [auditMd, playbookMd, legalMd] = await Promise.all([
  readFile(DOCS.audit, "utf8"),
  readFile(DOCS.playbook, "utf8"),
  readFile(DOCS.legal, "utf8"),
]);

// 1. general-audit.md
expect(auditMd, /全武将验证矩阵（(\d+) 将/, [audited], "general-audit.md 标题将数");
expect(
  auditMd,
  /；(\d+) 位 = 默认选将池（(\d+) − (\d+)）/,
  [audited, total, excluded],
  "general-audit.md 口径算式",
);
expect(auditMd, /## 4\. 未纳入批次的武将（(\d+) 位/, [excluded], "general-audit.md §4 排除人数");

// 2. batch-playbook.md
expect(playbookMd, /\*\*(\d+) 将 \/ B0–B20\*\*/, [audited], "batch-playbook.md 验证批次将数");
expect(
  playbookMd,
  /有界版同名的原版 (\d+) 位不入批次/,
  [excluded],
  "batch-playbook.md 排除人数",
);

// 3. legal-actions.md 预设人数列表
{
  const m = /可用人数去重后 = ([\d\s/]+)；/.exec(legalMd);
  if (!m) {
    fail("legal-actions.md：未找到「可用人数去重后 = ...」列表");
  } else {
    const got = m[1]
      .split("/")
      .map((s) => Number(s.trim()))
      .filter((n) => !Number.isNaN(n));
    if (got.length !== presetCounts.length || got.some((n, i) => n !== presetCounts[i])) {
      fail(
        `legal-actions.md 预设人数 [${got.join("/")}] ≠ catalog [${presetCounts.join("/")}]`,
      );
    } else if (verbose) {
      info.push(`legal-actions.md 预设人数：${got.join("/")} ✓`);
    }
  }
}

// 4. 陈旧字面量扫描（仅当前态文档；historical 记录不改）
const STALE = [
  { re: /149 将/g, hint: "应为 148 将" },
  { re: /149 位 = 默认选将池/g, hint: "应为 148 位" },
  { re: /（83 位，有界版同名）/g, hint: "应为 84 位" },
  { re: /\b83 排除/g, hint: "应为 84 排除" },
];
for (const [name, text] of [
  ["general-audit.md", auditMd],
  ["batch-playbook.md", playbookMd],
]) {
  for (const { re, hint } of STALE) {
    const hits = text.match(re);
    if (hits) fail(`${name}：陈旧计数「${hits[0]}」（${hint}）`);
  }
}

if (verbose) for (const line of info) console.log(`[counts] ${line}`);

if (problems.length) {
  console.error("[counts] 失败：");
  for (const p of problems) console.error(`  - ${p}`);
  console.error(
    `[counts] 口径：总数 ${total} − 排除 ${excluded} = 审计 ${audited}；技能 ${skills.size}；预设 [${presetCounts.join("/")}]`,
  );
  process.exit(1);
}
console.log(
  `[counts] OK：${total} 将 − ${excluded} 排除 = ${audited} 审计；技能 ${skills.size}；预设 [${presetCounts.join("/")}]`,
);
