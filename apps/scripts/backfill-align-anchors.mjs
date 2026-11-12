#!/usr/bin/env node
// 对齐单锚点回填（M10 legacy）：给 `align/B*.json` 的 questions[].test 补上用例锚点。
//
// 用法：
//   node apps/scripts/backfill-align-anchors.mjs --batch B0,B1,B2        # 回填（写文件）
//   node apps/scripts/backfill-align-anchors.mjs --batch B0 --dry-run    # 只看会补什么
//   node apps/scripts/backfill-align-anchors.mjs --batch all             # 全部批次（766 条欠账）
//
// 选锚点策略（保守，宁可留空也不乱指）：
//   1) 测试函数名包含 skill code → 最高优先；
//   2) 测试函数体包含 `"<code>"` 字面量；
//   3) 否则跳过（打印「未找到候选」，留给人走查时补）。
//
// 同一技能的多个 question 共用同一个锚点是允许的（锚点语义 = 「这条裁定由该用例守着」）。

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(".");
const ALIGN = path.join(ROOT, "docs", "games", "sanguosha", "align");
const TESTS = path.join(ROOT, "crates", "engines", "sanguosha2", "tests");

const arg = (name, dflt) => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
};
const DRY = process.argv.includes("--dry-run");
const batchArg = arg("batch", "B0,B1,B2");

/** 收集 tests/*.rs 里所有 `#[test]` 函数（file / fn / body）。 */
let allTests = null;
function allTestFns() {
  if (allTests) return allTests;
  allTests = [];
  for (const f of readdirSync(TESTS).filter((x) => x.endsWith(".rs"))) {
    const text = readFileSync(path.join(TESTS, f), "utf8");
    for (const block of text.split(/#\[test\]/).slice(1)) {
      const m = /^\s*fn\s+([A-Za-z0-9_]+)/.exec(block);
      if (m) allTests.push({ file: f, fn: m[1], body: block });
    }
  }
  return allTests;
}

const files =
  batchArg === "all"
    ? readdirSync(ALIGN).filter((f) => /^B[0-9a-z]+\.json$/.test(f))
    : batchArg.split(",").map((b) => `${b.trim()}.json`);

let filled = 0;
let missing = 0;
const report = [];
for (const f of files) {
  const p = path.join(ALIGN, f);
  if (!existsSync(p)) {
    console.error(`[anchor] 找不到 ${f}`);
    continue;
  }
  const data = JSON.parse(readFileSync(p, "utf8"));
  const batch = data.batch ?? f.replace(/\.json$/, "");
  const fns = allTestFns();
  let changed = false;
  for (const g of data.generals ?? []) {
    for (const s of g.skills ?? []) {
      const qs = s.questions ?? [];
      // 候选优先级：函数名含 code（同批次优先）> 函数体含 "code" > 函数体含中文技能名
      const sameBatch = (t) => t.file === `${batch}.rs`;
      const named = fns.filter((t) => t.fn.includes(s.code));
      const mentioned = fns.filter((t) => t.body.includes(`"${s.code}"`));
      const byName = fns.filter((t) => s.name && t.body.includes(s.name));
      const ordered = [
        ...named.filter(sameBatch),
        ...named.filter((t) => !sameBatch(t)),
        ...mentioned.filter((t) => !named.includes(t) && sameBatch(t)),
        ...mentioned.filter((t) => !named.includes(t) && !sameBatch(t)),
        ...byName.filter((t) => !named.includes(t) && !mentioned.includes(t)),
      ];
      let pick = 0;
      if (qs.length === 0 && !s.test) {
        // 无裁定项的技能：补一个「技能级」锚点（contract 里也算 covered）
        const cand = ordered[0];
        if (!cand) {
          missing += 1;
          report.push(`  ✗ ${batch}/${s.code}：无候选用例（需人工补）`);
          continue;
        }
        const ref = `${cand.file.replace(/\.rs$/, "")}::${cand.fn}`;
        s.test = ref;
        changed = true;
        filled += 1;
        report.push(`  ✓ ${batch}/${s.code}（技能级） → ${ref}`);
        continue;
      }
      for (const q of qs) {
        if (q.test) continue;
        const cand = ordered[pick] ?? ordered[0];
        if (!cand) {
          missing += 1;
          report.push(`  ✗ ${batch}/${s.code}#${q.id}：无候选用例（需人工补）`);
          continue;
        }
        const ref = `${cand.file.replace(/\.rs$/, "")}::${cand.fn}`;
        q.test = ref;
        changed = true;
        filled += 1;
        report.push(`  ✓ ${batch}/${s.code}#${q.id} → ${ref}`);
        pick = Math.min(pick + 1, Math.max(0, ordered.length - 1));
      }
    }
  }
  if (changed && !DRY) writeFileSync(p, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

console.log(
  `[anchor] ${DRY ? "（dry-run）" : ""}回填 ${filled} 条，未找到候选 ${missing} 条`,
);
for (const line of report) console.log(line);
