#!/usr/bin/env node
// 对齐单锚点回填（M10 legacy）：给 `align/B*.json` 的 questions[].test 补上用例锚点。
//
// 用法：
//   node apps/scripts/backfill-align-anchors.mjs --batch B0,B1,B2        # 回填（写文件）
//   node apps/scripts/backfill-align-anchors.mjs --batch B0 --dry-run    # 只看会补什么
//   node apps/scripts/backfill-align-anchors.mjs --batch all             # 全部批次（766 条欠账）
//
// 选锚点策略（保守，宁可留空也不乱指）：
//   strong：函数名含 skill code，或函数体含 `"<code>"` 字面量 —— 只在这些里轮转；
//   medium：函数体含**中文技能名**且位于同批次测试文件 —— 只在没有任何 strong 候选时兜底，
//           且同一技能的所有 question 共用同一锚点（不轮转，避免指到别的技能用例）；
//   都没有 → 留空并标 `legacy: true`（显式欠账，交人工走查补）。
//
// `--redo`：重新评估所有锚点（保留 strong 的既有锚点，重算其余）。

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
const REDO = process.argv.includes("--redo");
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
      // strong 候选：函数名含 code（同批次优先）> 函数体含 "code" 字面量
      const sameBatch = (t) => t.file === `${batch}.rs`;
      const named = fns.filter((t) => t.fn.includes(s.code));
      const literal = fns.filter((t) => t.body.includes(`"${s.code}"`));
      const strong = [
        ...named.filter(sameBatch),
        ...named.filter((t) => !sameBatch(t)),
        ...literal.filter((t) => !named.includes(t) && sameBatch(t)),
        ...literal.filter((t) => !named.includes(t) && !sameBatch(t)),
      ];
      // medium 兜底：同批次测试里出现中文技能名（只取第一个，全部 question 共用）
      const medium = s.name
        ? fns.filter((t) => sameBatch(t) && t.body.includes(s.name))
        : [];
      const pool = strong.length > 0 ? strong : medium;
      const isStrong = (ref) => {
        if (!ref) return false;
        const [file, fn] = ref.split("::");
        const t = fns.find((x) => x.file === `${file}.rs` && x.fn === fn);
        if (!t) return false;
        return t.fn.includes(s.code) || t.body.includes(`"${s.code}"`);
      };

      let pick = 0;
      const assign = (holder) => {
        if (holder.test && isStrong(holder.test) && !REDO) return false;
        if (holder.test && isStrong(holder.test) && REDO) return false;
        const cand = pool[Math.min(pick, Math.max(0, pool.length - 1))];
        if (!cand) {
          if (holder.test) {
            delete holder.test;
            if (holder.legacy === undefined) holder.legacy = true;
            missing += 1;
            report.push(`  ✗ ${batch}/${s.code}${holder.id ? `#${holder.id}` : "（技能级）"}：无候选，已清空并标 legacy`);
            return true;
          }
          missing += 1;
          report.push(`  ✗ ${batch}/${s.code}${holder.id ? `#${holder.id}` : "（技能级）"}：无候选用例（需人工补）`);
          if (holder.legacy === undefined) holder.legacy = true;
          return true;
        }
        const ref = `${cand.file.replace(/\.rs$/, "")}::${cand.fn}`;
        holder.test = ref;
        if (holder.legacy) delete holder.legacy;
        filled += 1;
        report.push(`  ✓ ${batch}/${s.code}${holder.id ? `#${holder.id}` : "（技能级）"} → ${ref}`);
        pick = Math.min(pick + 1, Math.max(0, pool.length - 1));
        return true;
      };

      if (qs.length === 0) {
        changed = assign(s) || changed;
      } else {
        for (const q of qs) changed = assign(q) || changed;
      }
    }
  }
  if (changed && !DRY) writeFileSync(p, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

console.log(
  `[anchor] ${DRY ? "（dry-run）" : ""}回填 ${filled} 条，未找到候选 ${missing} 条`,
);
for (const line of report) console.log(line);
