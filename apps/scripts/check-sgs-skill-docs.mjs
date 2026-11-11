#!/usr/bin/env node
// 门禁（S6-b）：`skills.md` 与 **v2 技能注册表**一致（文档 ↔ 代码不再依赖 v1 crate）。
//
// 口径：
// - 代码侧：v2 `skills::SKILLS`（`cargo run --example sgs2-registry -- --json`）的 code 集合；
// - 文档侧：
//   a) 全部表格行里的 `` `code` ``（§2–§3 技能表）；
//   b) §4.1「授予技能」表（来源技能 / 技能 / code / 类型 / 状态）；
// - 断言：
//   1. 文档提到的每个 code 都在 v2 注册表内（防文档写错/写旧）；
//   2. §4.1 每个 code 都在注册表内；
//   3. v2 中**字面量** `grant_skill(g, ..., "code")` 的每个 code 都必须在 §4.1 登记；
//   4. 注册表里存在但文档完全没提的 code → 警告（不阻断，便于分批补文档）。
//
// 用法：node apps/scripts/check-sgs-skill-docs.mjs [--verbose]

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const verbose = process.argv.includes("--verbose");
const SKILLS_MD = path.join(projectRoot, "docs/games/sanguosha/skills.md");
const SKILLS_DIR = path.join(projectRoot, "crates/engines/sanguosha2/src/skills");

// 1) v2 注册表（编译产物 JSON；与 `check:sgs2-registry` 同源）。
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
  { cwd: projectRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
if (run.status !== 0) {
  console.error("[skill-docs] 读取 v2 注册表失败：");
  console.error((run.stderr ?? "").split("\n").slice(-10).join("\n"));
  process.exit(1);
}
const line = (run.stdout ?? "").split(/\r?\n/).reverse().find((l) => l.trim().startsWith("{"));
if (!line) {
  console.error("[skill-docs] 注册表 JSON 为空");
  process.exit(1);
}
const registry = JSON.parse(line);
const registryCodes = new Set((registry.skills ?? []).map((s) => s.code));

// 2) 文档侧 code 集合。
const md = readFileSync(SKILLS_MD, "utf8");
// 只提取「技能表行」：5 列且第 3 列是 code、第 4 列是技能类型。
// （其它表格里的 `` `hook` `` 名称不是技能，如 damage_after / hand_limit。）
const SKILL_KINDS = /^(普通|锁定|主公|限定|觉醒|转换|觉醒技|锁定技|主公技|限定技)$/;
const docCodes = new Set();
for (const line of md.split(/\r?\n/)) {
  if (!line.startsWith("|")) continue;
  const cells = line.split("|").slice(1, -1).map((c) => c.trim());
  if (cells.length < 4) continue;
  const code = /^`([a-z0-9_]+)`$/.exec(cells[2])?.[1];
  if (!code) continue;
  if (!SKILL_KINDS.test(cells[3])) continue;
  docCodes.add(code);
}
// §4.1 表格（`| 来源技能 | 技能 | `code` | 类型 | 状态 |`）
const grantedSection = (() => {
  const start = md.indexOf("### 4.1");
  if (start < 0) return "";
  const end = md.indexOf("\n## ", start + 1);
  return md.slice(start, end < 0 ? md.length : end);
})();
const grantedCodes = new Set();
const grantedRows = [];
for (const line of grantedSection.split(/\r?\n/)) {
  if (!line.startsWith("|")) continue;
  const cells = line.split("|").slice(1, -1).map((c) => c.trim());
  if (cells.length < 5) continue;
  const code = /^`([a-z0-9_]+)`$/.exec(cells[2])?.[1];
  if (!code) continue;
  grantedCodes.add(code);
  grantedRows.push({ source: cells[0], name: cells[1], code, kind: cells[3], status: cells[4] });
}

// 3) v2 里字面量授予点。
const literalGrants = new Set();
let files = [];
try {
  files = readdirSync(SKILLS_DIR).filter((f) => f.endsWith(".rs"));
} catch (e) {
  console.error(`[skill-docs] 读 v2 技能目录失败：${e.message}`);
  process.exit(1);
}
for (const f of files) {
  const text = readFileSync(path.join(SKILLS_DIR, f), "utf8");
  for (const m of text.matchAll(/grant_skill\(\s*[^,]+,\s*[^,]+,\s*"([a-z0-9_]+)"\s*\)/g)) {
    literalGrants.add(m[1]);
  }
}

// 4) 断言。
const problems = [];
for (const code of docCodes) {
  if (!registryCodes.has(code)) problems.push(`skills.md 提到未注册技能「${code}」`);
}
for (const code of grantedCodes) {
  if (!registryCodes.has(code)) problems.push(`§4.1 授予技「${code}」未在 v2 注册表`);
}
for (const code of literalGrants) {
  if (!grantedCodes.has(code)) {
    problems.push(`v2 代码授予「${code}」但 §4.1 未登记`);
  }
}
const undocumented = [...registryCodes].filter((c) => !docCodes.has(c));

if (verbose) {
  console.log(`[skill-docs] 注册表 ${registryCodes.size} / 文档 ${docCodes.size} / §4.1 ${grantedCodes.size} / 字面量授予 ${literalGrants.size}`);
  for (const r of grantedRows) console.log(`  ${r.source} → ${r.name}(${r.code}) ${r.status}`);
  if (undocumented.length) console.log(`  未文档化：${undocumented.join("、")}`);
}

if (problems.length) {
  console.error(`[check] 失败：${problems.length} 处文档-代码不一致`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log(
  `[check] OK：skills.md ↔ v2 注册表一致（注册表 ${registryCodes.size} 技能 / §4.1 授予技 ${grantedCodes.size} / 字面量授予点 ${literalGrants.size}）`,
);
if (undocumented.length) {
  console.log(`[check] 提示：${undocumented.length} 个注册表技能尚未写入 skills.md（不阻断）：${undocumented.slice(0, 12).join("、")}${undocumented.length > 12 ? "…" : ""}`);
}
