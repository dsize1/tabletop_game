#!/usr/bin/env node
// M4 harness：v2 技能注册表门禁（B8–B20 迁移的「防漏登记/防漏测」底座）。
//
// 数据源：`cargo run -p sanguosha2 --example sgs2-registry -- --json`（Rust 注册表导出，非文本解析）。
//
// 检查：
//   1. code 唯一、name/window 非空、count 与导出声明一致；
//   2. 转换技清单（TRANSFORM_SKILLS）⊆ 注册表，且每行 `transform` 与清单一致；
//   3. 对齐单覆盖：B0–B7 的 skill code **恰好等于**注册表（v2 已迁移批次）；
//      （B8 起迁移后把批次号加入 V2_BATCHES，等于显式登记「这批已迁到 v2」）
//   4. 用例覆盖：每个注册 code 至少出现在一个 `#[test]` 函数体里（间接覆盖走 ALLOW_INDIRECT）。
//
// 用法：node apps/scripts/check-sgs2-registry.mjs [--verbose]

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const verbose = process.argv.includes("--verbose");
const problems = [];
const fail = (m) => problems.push(m);

/** 已迁移到 v2 的批次（B8 起每迁一批就追加；gate 随之要求注册表与对齐单精确一致）。 */
const V2_BATCHES = [0, 1, 2, 3, 4, 5, 6, 7];

/** 间接覆盖：这些 code 由同一批用例的行为断言覆盖（用例里不出现字符串字面量）。 */
const ALLOW_INDIRECT = new Map([
  ["yongsi", "B1 用例通过摸牌/弃牌数行为断言（庸肆），不出现 code 字面量"],
  ["huaiju", "B1 用例通过伤害防止行为断言（怀橘），不出现 code 字面量"],
  ["cunmu", "B1 用例通过牌堆底摸牌行为断言（寸目），不出现 code 字面量"],
]);

// ---------------------------------------------------------------------------
// 1) 导出注册表
// ---------------------------------------------------------------------------

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
  console.error("[sgs2-registry] 导出失败：");
  console.error(run.stderr?.slice(0, 2000) ?? "");
  process.exit(1);
}
const line = (run.stdout ?? "")
  .split(/\r?\n/)
  .map((s) => s.trim())
  .find((s) => s.startsWith("{"));
if (!line) {
  console.error("[sgs2-registry] 未取到 JSON");
  process.exit(1);
}
const reg = JSON.parse(line);

// ---------------------------------------------------------------------------
// 2) 注册表自身一致性
// ---------------------------------------------------------------------------

const seen = new Set();
for (const s of reg.skills) {
  if (seen.has(s.code)) fail(`重复注册 code：${s.code}`);
  seen.add(s.code);
  if (!s.code || !s.name) fail(`code/name 为空：${JSON.stringify(s)}`);
  if (!s.window) fail(`${s.code} 缺少 window`);
  if (s.transform !== reg.transform_skills.includes(s.code)) {
    fail(`${s.code} 的 transform 标记与 TRANSFORM_SKILLS 不一致`);
  }
  if (verbose) {
    console.log(
      `  ${s.code.padEnd(12)} ${s.name}  active=${s.active} triggers=${JSON.stringify(s.triggers)} window=${s.window}`,
    );
  }
}
if (reg.count !== reg.skills.length) {
  fail(`count=${reg.count} 与 skills 条数 ${reg.skills.length} 不一致`);
}
for (const code of reg.transform_skills) {
  if (!seen.has(code)) fail(`TRANSFORM_SKILLS 中的 ${code} 不在注册表`);
}

// ---------------------------------------------------------------------------
// 3) 对齐单覆盖（已迁移批次 ↔ 注册表精确一致）
// ---------------------------------------------------------------------------

const alignCodes = new Set();
for (const b of V2_BATCHES) {
  const file = path.join(projectRoot, "docs/games/sanguosha/align", `B${b}.md`);
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    fail(`缺少对齐单 B${b}.md`);
    continue;
  }
  for (const m of text.matchAll(/^##\s+([^（(]+)（`([a-z0-9_]+)`/gm)) {
    alignCodes.add(m[2]);
  }
}
for (const code of alignCodes) {
  if (!seen.has(code)) fail(`对齐单（B${V2_BATCHES.join("/")}）的 ${code} 未在 v2 注册`);
}
for (const code of seen) {
  if (!alignCodes.has(code)) fail(`v2 注册了 ${code}，但对齐单（已迁移批次）没有该技能`);
}

// ---------------------------------------------------------------------------
// 4) 用例覆盖（每个 code 必须出现在某个 #[test] 函数体里）
// ---------------------------------------------------------------------------

const testsDir = path.join(projectRoot, "crates/engines/sanguosha2/tests");
const testBlocks = [];
for (const file of readdirSync(testsDir)) {
  if (!file.endsWith(".rs")) continue;
  const text = readFileSync(path.join(testsDir, file), "utf8");
  let i = 0;
  while ((i = text.indexOf("#[test]", i)) !== -1) {
    const start = text.indexOf("{", i);
    if (start < 0) break;
    let depth = 0;
    let k = start;
    for (; k < text.length; k += 1) {
      if (text[k] === "{") depth += 1;
      else if (text[k] === "}") {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    testBlocks.push(text.slice(start, k + 1));
    i = k + 1;
  }
}
for (const s of reg.skills) {
  const hit = testBlocks.some((b) => b.includes(`"${s.code}"`));
  if (!hit && !ALLOW_INDIRECT.has(s.code)) {
    fail(`技能 ${s.code}（${s.name}）没有对应用例（请在 tests/b*.rs 增加至少 1 条正/反例）`);
  }
  if (!hit && ALLOW_INDIRECT.has(s.code) && verbose) {
    console.log(`  · ${s.code} 间接覆盖：${ALLOW_INDIRECT.get(s.code)}`);
  }
}

// ---------------------------------------------------------------------------
// 汇总
// ---------------------------------------------------------------------------

console.log(
  `[sgs2-registry] 注册 ${reg.skills.length} 个技能 / 转换技 ${reg.transform_skills.length} / ` +
    `已迁移批次 B${V2_BATCHES.join("、B")}（对齐单 ${alignCodes.size} 个 code）/ 用例块 ${testBlocks.length}`,
);
if (problems.length) {
  console.error(`[sgs2-registry] 未通过（${problems.length}）：`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[sgs2-registry] OK");
