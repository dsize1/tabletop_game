#!/usr/bin/env node
// 技能对齐单生成器 / 渲染器 / 单批校验器。
//
// 人读：`docs/games/sanguosha/align/<B>.md`（由本脚本从 json 生成，勿手改）
// 机器：`docs/games/sanguosha/align/<B>.json`（单一事实源）
//
// 用法：
//   node apps/scripts/sgs-align-sheet.mjs --list                 # 列出批次与对齐状态
//   node apps/scripts/sgs-align-sheet.mjs --batch B0 --new      # 生成骨架（已存在则报错）
//   node apps/scripts/sgs-align-sheet.mjs --batch B0 --sync     # json → md
//   node apps/scripts/sgs-align-sheet.mjs --batch B0 --check    # 单批校验
//   node apps/scripts/sgs-align-sheet.mjs --all --sync          # 全部批次渲染
//
// 约定：`--new` 只建骨架；6 段内容由 agent 填，第 6/7 段由用户确认后填。

import { writeFileSync, existsSync } from "node:fs";
import { loadCatalog } from "./lib/sgs-catalog.mjs";
import { expandBatches, BATCHES } from "./lib/sgs-batches.mjs";
import {
  ALIGN_DIR,
  loadMechanics,
  loadSpec,
  listSpecBatches,
  renderMarkdown,
  skeletonSpec,
  specPath,
  validateSpec,
} from "./lib/sgs-align.mjs";

const argv = process.argv.slice(2);
const has = (f) => argv.includes(f);
const val = (f, d = null) => {
  const i = argv.indexOf(f);
  return i >= 0 ? argv[i + 1] : d;
};

const catalog = loadCatalog();
const batches = expandBatches(catalog);
const mechanics = loadMechanics();

if (has("--list") || argv.length === 0) {
  console.log(`[align] 对齐单目录：docs/games/sanguosha/align/（机制词表 v${mechanics.version}）`);
  const done = new Set(listSpecBatches());
  for (const b of batches) {
    const mark = done.has(b.id) ? "有" : "缺";
    let extra = "";
    if (done.has(b.id)) {
      const spec = loadSpec(b.id);
      const total = spec.generals.flatMap((g) => g.skills).length;
      const confirmed = spec.generals
        .flatMap((g) => g.skills)
        .filter((s) => ["confirmed", "implemented", "verified"].includes(s.status)).length;
      const qs = spec.generals.flatMap((g) => g.skills).flatMap((s) => s.questions);
      const answered = qs.filter((q) => q.answer).length;
      extra = `  [${spec.status}] 技能 ${total}，已对齐 ${confirmed}，疑点 ${answered}/${qs.length}`;
    }
    console.log(`  ${mark} ${b.id}  ${String(b.names.length).padStart(2)} 将  ${b.source}${extra}`);
  }
  process.exit(0);
}

const targets = has("--all") ? listSpecBatches() : [val("--batch", "B0")];
let failed = false;

for (const batchId of targets) {
  const file = specPath(batchId);
  if (has("--new")) {
    if (existsSync(file)) {
      console.error(`[align] ${batchId}: 已存在 ${file}，不覆盖（删掉再加 --new 才会重建）`);
      failed = true;
      continue;
    }
    const spec = skeletonSpec(batchId, catalog, batches);
    writeFileSync(file, JSON.stringify(spec, null, 2) + "\n");
    console.log(`[align] ${batchId}: 已生成骨架 ${file}`);
    continue;
  }
  if (!existsSync(file)) {
    console.error(`[align] ${batchId}: 对齐单不存在（先 --new）`);
    failed = true;
    continue;
  }
  const spec = loadSpec(batchId);
  const md = renderMarkdown(spec, catalog, mechanics);
  const mdFile = file.replace(/\.json$/, ".md");
  writeFileSync(mdFile, md);
  const { errors, warnings, conflicts } = validateSpec(spec, catalog, mechanics);
  console.log(`[align] ${batchId}: 已渲染 ${mdFile}（错误 ${errors.length} / 警告 ${warnings.length} / 冲突 ${conflicts.length}）`);
  if (has("--check")) {
    for (const e of errors) console.error(`  ✗ ${e}`);
    for (const w of warnings) console.error(`  ! ${w}`);
    for (const c of conflicts) console.error(`  ⚡ ${c}`);
    if (errors.length) failed = true;
  }
}

if (has("--sync")) console.log(`[align] 目录：${ALIGN_DIR}`);
process.exit(failed ? 1 : 0);
