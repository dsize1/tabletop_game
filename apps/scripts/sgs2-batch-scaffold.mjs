#!/usr/bin/env node
// B8–B20 迁移脚手架：读对齐单 + v2 注册表，输出「这批要做什么/缺什么/怎么验」。
//
// 不写任何文件（安全），只打印清单；可选 --json 给机器读。
//
// 用法：
//   node apps/scripts/sgs2-batch-scaffold.mjs B8
//   node apps/scripts/sgs2-batch-scaffold.mjs B8 --json

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const args = process.argv.slice(2);
const batchArg = args.find((a) => /^B\d+$/i.test(a));
const jsonOut = args.includes("--json");
if (!batchArg) {
  console.error("用法：node apps/scripts/sgs2-batch-scaffold.mjs B8 [--json]");
  process.exit(2);
}
const batch = batchArg.toUpperCase();
const batchNum = Number(batch.slice(1));

const file = path.join(projectRoot, "docs/games/sanguosha/align", `${batch}.md`);
if (!existsSync(file)) {
  console.log(`[sgs2:batch] 缺少对齐单：${path.relative(projectRoot, file)}`);
  console.log("先创建骨架（6 段内容由 agent 填，第 6/7 段由用户确认）：");
  console.log(`  node apps/scripts/sgs-align-sheet.mjs --batch ${batch} --new`);
  console.log("然后重新运行：");
  console.log(`  npm run sgs2:batch -- ${batch}`);
  process.exit(0);
}
const text = readFileSync(file, "utf8");

// 注册表快照
const run = spawnSync(
  "cargo",
  [
    "run",
    "-q",
    "--manifest-path",
    "crates/Cargo.toml",
    "-p",
    "sanguosha2",
    "--example",
    "sgs2-registry",
    "--",
    "--json",
  ],
  { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
if (run.status !== 0) {
  console.error("[sgs2:batch] 注册表导出失败：");
  console.error(run.stderr?.slice(0, 1500) ?? "");
  process.exit(1);
}
const line = (run.stdout ?? "")
  .split(/\r?\n/)
  .map((s) => s.trim())
  .find((s) => s.startsWith("{"));
const reg = JSON.parse(line);
const registered = new Set(reg.skills.map((s) => s.code));

// 对齐单：武将 + 技能
const generals = [...text.matchAll(/^#\s+(.+?)\s+`(sgs_hero_\d+)`（(\d+)\s*技能）/gm)].map((m) => ({
  name: m[1],
  code: m[2],
  skillCount: Number(m[3]),
}));
const skills = [...text.matchAll(/^##\s+([^（(]+)（`([a-z0-9_]+)`\s*\/\s*([a-z]+)）(.*)$/gm)].map((m) => ({
  name: m[1],
  code: m[2],
  kind: m[3],
  implementedInDoc: m[4].includes("implemented"),
  inRegistry: registered.has(m[2]),
}));

const missing = skills.filter((s) => !s.inRegistry);
const testFile = path.join(projectRoot, "crates/engines/sanguosha2/tests", `${batch.toLowerCase()}.rs`);
const skillFile = path.join(projectRoot, "crates/engines/sanguosha2/src/skills", `${batch.toLowerCase()}.rs`);

const plan = {
  batch,
  batchNum,
  generals,
  skills,
  missing: missing.map((s) => s.code),
  files: {
    align: path.relative(projectRoot, file),
    skills: path.relative(projectRoot, skillFile),
    tests: path.relative(projectRoot, testFile),
    skillsExists: existsSync(skillFile),
    testsExists: existsSync(testFile),
  },
  next: [
    `1. 在 ${path.relative(projectRoot, skillFile)} 实现技能，并在 src/skills/mod.rs 的 SKILLS 注册`,
    `2. 在 ${path.relative(projectRoot, testFile)} 加正/反例（每技能 ≥1 正例 + ≥1 反例）`,
    `3. 把 ${batchNum} 加入 apps/scripts/check-sgs2-registry.mjs 的 V2_BATCHES（登记「这批已迁到 v2」）`,
    "4. 回写对齐单第 2 段（现实现）为新引擎行为 + impl 锚点",
    "5. 运行 npm run check:sgs2 && npm run sgs:check:fast",
  ],
};

if (jsonOut) {
  console.log(JSON.stringify(plan, null, 2));
  process.exit(missing.length && !plan.files.testsExists ? 0 : 0);
}

console.log(`\n== ${batch} · ${generals.length} 将 / ${skills.length} 技能 ==`);
for (const g of generals) {
  console.log(`  ${g.code}  ${g.name}（${g.skillCount} 技能）`);
}
console.log("\n技能 → v2 注册状态：");
for (const s of skills) {
  const mark = s.inRegistry ? "✅ 已注册" : "⬜ 未注册";
  console.log(`  ${mark}  ${s.code.padEnd(14)} ${s.name}（${s.kind}）`);
}
if (missing.length) {
  console.log(`\n待迁移（${missing.length}）：${missing.map((s) => s.code).join(", ")}`);
} else {
  console.log("\n全部技能已注册（本批可做「回写对齐单 + 用例复查」）。");
}
console.log("\n文件：");
console.log(`  对齐单：${plan.files.align}`);
console.log(`  技能：  ${plan.files.skills}${plan.files.skillsExists ? "" : "（待创建）"}`);
console.log(`  用例：  ${plan.files.tests}${plan.files.testsExists ? "" : "（待创建）"}`);
console.log("\n下一步：");
for (const n of plan.next) console.log(`  ${n}`);
