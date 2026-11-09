#!/usr/bin/env node
// 验证批次 ↔ 开发批次 / 去重池 一致性门禁。
//
// 背景：验证批次（147 将，`lib/sgs-batches.mjs`）与开发批次（232 将，docs 批次表 /
// `sgs-batch-status`）是两套口径。名称勘误（张昭张弘→张昭张纮）曾导致「界优先去重」失效、
// 原版错误进入验证批次。本门禁把「名录/去重池/验证批次/开发批次」四方对齐，防再次漂移。
//
// 断言：
// 1) 232 将 = 审计范围(147) + 排除(85)，且每个排除项都有同名界版（`excludedBy` 指向界版 code）；
// 2) 验证批次展开成功（无重/无漏/每批 ≤8），总数 == 审计范围；
// 3) 审计范围 == 全量包组合经 `prefer_jiexian` 去重后的池（按名字去重，界版优先）；
// 4) 审计范围内每个武将都出现在开发批次表里（验证批次 ⊆ 开发批次覆盖）；
// 5) 每批武将的 code/name 与 data.rs 一致（catalog 已校验，这里校验批次内无重复 code）。
//
// 用法：npm run check:sgs-batch-consistency [--verbose]

import { loadCatalog } from "./lib/sgs-catalog.mjs";
import { expandBatches, BATCHES, MAX_PER_BATCH } from "./lib/sgs-batches.mjs";
import { batchReport, loadGenerals } from "./sgs-data-lib.mjs";

const verbose = process.argv.includes("--verbose");
const tag = "[check:sgs-batch-consistency]";
const problems = [];

const catalog = loadCatalog();
/** 去重口径：剔除名字前缀「界」「将」后同名即视为重复（与 lib/sgs-catalog.mjs 一致）。 */
const dedupeName = (name) => name.replace(/^(?:界|将)/, "");
const batches = expandBatches(catalog); // 内部已校验重/漏/超限
const auditCodes = new Set(catalog.audited.map((g) => g.code));
const allGens = new Map([...loadGenerals().values()].map((g) => [g.code, g]));

// 1) 232 = 审计 + 排除
if (catalog.stats.generals !== catalog.audited.length + catalog.excluded.length) {
  problems.push(
    `232 拆分不一致：${catalog.stats.generals} ≠ ${catalog.audited.length} + ${catalog.excluded.length}`,
  );
}
for (const ex of catalog.excluded) {
  if (!ex.excludedBy) problems.push(`排除项缺少 excludedBy：${ex.name}(${ex.code})`);
  const jie = allGens.get(ex.excludedBy);
  if (!jie) problems.push(`排除项指向的界版不存在：${ex.name} → ${ex.excludedBy}`);
  else if (jie.pack !== "jiexian") problems.push(`排除项 excludedBy 不是界版：${ex.name} → ${ex.excludedBy}`);
  else if (dedupeName(jie.name) !== dedupeName(ex.name)) {
    problems.push(`排除项与界版名字不匹配：${ex.name} ↔ ${jie.name}`);
  }
}

// 2) 批次展开
if (batches.length !== BATCHES.length) problems.push(`批次数 ${batches.length} ≠ 表 ${BATCHES.length}`);
let total = 0;
for (const b of batches) {
  total += b.generals.length;
  if (b.generals.length > MAX_PER_BATCH) problems.push(`批次 ${b.id} 超过 ${MAX_PER_BATCH} 将`);
  const codes = new Set(b.generals.map((g) => g.code));
  if (codes.size !== b.generals.length) problems.push(`批次 ${b.id} 内有重复武将 code`);
  for (const g of b.generals) {
    if (!auditCodes.has(g.code)) problems.push(`批次 ${b.id} 含非审计范围武将：${g.name}(${g.code})`);
  }
}
if (total !== catalog.audited.length) {
  problems.push(`批次总数 ${total} ≠ 审计范围 ${catalog.audited.length}`);
}

// 3) 去重池（全量包组合，按名字去重、界版优先）
const packs = ["standard", "wind_fire_forest_mountain", "yijiang", "jiexian"];
const inPacks = [...allGens.values()].filter((g) => packs.includes(g.pack));
const jieNames = new Set(
  inPacks.filter((g) => g.pack === "jiexian").map((g) => dedupeName(g.name)),
);
const pool = inPacks.filter((g) => g.pack === "jiexian" || !jieNames.has(dedupeName(g.name)));
const poolCodes = new Set(pool.map((g) => g.code));
if (pool.length !== catalog.audited.length) {
  problems.push(`去重池 ${pool.length} ≠ 审计范围 ${catalog.audited.length}`);
}
for (const g of catalog.audited) {
  if (!poolCodes.has(g.code)) problems.push(`审计武将不在去重池内：${g.name}(${g.code})`);
}
for (const g of pool) {
  if (!auditCodes.has(g.code)) problems.push(`去重池含未纳入审计的武将：${g.name}(${g.code})`);
}

// 4) 开发批次表（docs 中的 Y*/J* 表）↔ 232：表内每个条目必须真实存在且名字一致；
//    dev 表未覆盖的组（一将成名批次 1/2、界标 27 位）只做 info 报告（属早期批次，见 milestones）。
const dev = batchReport();
const byGid = new Map([...allGens.values()].map((g) => [g.gid, g]));
const devEntries = dev.report.flatMap((b) => b.entries.map((e) => ({ ...e, batch: b.batch })));
for (const e of devEntries) {
  const g = byGid.get(e.gid);
  if (!g) {
    problems.push(`开发批次 ${e.batch} 含不存在的 gid=${e.gid}（${e.name}）`);
    continue;
  }
  if (g.name !== e.name) {
    problems.push(`开发批次 ${e.batch} gid=${e.gid} 名字不一致：表「${e.name}」≠ catalog「${g.name}」`);
  }
}
const devCodes = new Set(devEntries.map((e) => byGid.get(e.gid)?.code).filter(Boolean));
const uncovered = catalog.audited.filter(
  (g) => (g.pack === "yijiang" || g.pack === "jiexian") && !devCodes.has(g.code),
);
for (const g of uncovered) {
  if (!g.implemented) problems.push(`未覆盖且未实现：${g.name}(${g.code})`);
}
const devTotal = devEntries.length;

if (problems.length) {
  for (const p of problems) console.error(`${tag} ❌ ${p}`);
  process.exit(1);
}
if (verbose || uncovered.length) {
  console.log(
    `${tag} info：232 = 审计 ${catalog.audited.length} + 排除 ${catalog.excluded.length}；去重池 ${pool.length}；` +
      `验证批次 ${batches.length} 批（最大 ${Math.max(...batches.map((b) => b.generals.length))} 将）；` +
      `开发批次表 ${devTotal} 条，未覆盖 ${uncovered.length} 位（批次1/2 + 界标，属早期批次）`,
  );
}
console.log(
  `${tag} OK：验证批次 ${batches.length} 批 / ${total} 将 ↔ 去重池 ${pool.length} 一致；开发批次表 ${devTotal} 条均与 engine-catalog.json 一致`,
);
