#!/usr/bin/env node
// spec-kit 一致性门禁：规格 ↔ 代码 ↔ 用例。
//
// 用法：
//   node apps/scripts/check-spec-conformance.mjs            # 校验
//   node apps/scripts/check-spec-conformance.mjs --verbose  # 打印每条规则
//   node apps/scripts/check-spec-conformance.mjs --write    # 重新生成 contracts/skills.json
//
// 校验项：
//   1. 每条规则必须有用例锚点（`anchor: tests/<file>::<fn>`），函数不存在 → 红
//   2. 每条 `forbid`（glob + 正则）在源码内必须 0 命中（注释行不计）→ 否则红
//   3. `contracts/skills.json` 必须与 Rust 注册表导出（sgs2-registry --json）完全一致
//
// 规格正文：specs/sanguosha/{constitution.md,state/,interaction/,legality/,skills/}

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(".");
const SPECS = path.join(ROOT, "specs", "sanguosha");
const CONTRACTS = path.join(SPECS, "contracts");
const CRATE = path.join(ROOT, "crates", "engines", "sanguosha2");
const VERBOSE = process.argv.includes("--verbose");
const WRITE = process.argv.includes("--write");

const problems = [];
const fail = (msg) => problems.push(msg);

// ---------------------------------------------------------------------------
// 1) 用例锚点
// ---------------------------------------------------------------------------

/** 收集所有 `fn <name>`（tests/ 与 src/ 下的 .rs 文件）。 */
function collectFns() {
  const out = new Map(); // "tests/x.rs" -> Set(fn)
  const add = (rel, text) => {
    const set = out.get(rel) ?? new Set();
    for (const m of text.matchAll(/^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z0-9_]+)/gm)) {
      set.add(m[1]);
    }
    out.set(rel, set);
  };
  const testsDir = path.join(CRATE, "tests");
  for (const f of readdirSync(testsDir)) {
    if (!f.endsWith(".rs")) continue;
    add(`tests/${f}`, readFileSync(path.join(testsDir, f), "utf8"));
  }
  const walk = (dir, relBase) => {
    for (const entry of readdirSync(dir)) {
      const p = path.join(dir, entry);
      const rel = `${relBase}/${entry}`;
      if (statSync(p).isDirectory()) walk(p, rel);
      else if (entry.endsWith(".rs")) add(rel, readFileSync(p, "utf8"));
    }
  };
  walk(path.join(CRATE, "src"), "src");
  return out;
}

function checkAnchor(fns, ruleId, anchor) {
  const idx = anchor.indexOf("::");
  if (idx < 0) {
    fail(`[${ruleId}] anchor 格式应为 tests/<file>::<fn>：${anchor}`);
    return;
  }
  const file = anchor.slice(0, idx);
  const fn = anchor.slice(idx + 2);
  const set = fns.get(file);
  if (!set) {
    fail(`[${ruleId}] 锚点文件不存在：${file}`);
    return;
  }
  if (!set.has(fn)) fail(`[${ruleId}] 锚点用例不存在：${anchor}`);
}

// ---------------------------------------------------------------------------
// 2) 禁止模式（forbid）
// ---------------------------------------------------------------------------

function listFiles(globRel) {
  // 支持 `src/skills/**` 与 `src/kernel/card_play.rs` 两种形态。
  const base = path.join(CRATE, globRel.replace(/\/\*\*$/, "").replace(/^src/, "src"));
  if (globRel.endsWith("/**")) {
    const out = [];
    const walk = (dir) => {
      for (const e of readdirSync(dir)) {
        const p = path.join(dir, e);
        if (statSync(p).isDirectory()) walk(p);
        else if (e.endsWith(".rs")) out.push(p);
      }
    };
    if (existsSync(base)) walk(base);
    return out;
  }
  return existsSync(base) ? [base] : [];
}

/** 去掉行注释（保留行号结构），避免注释里的示例触发误报。 */
function stripComments(text) {
  return text
    .split("\n")
    .map((ln) => (ln.trimStart().startsWith("//") ? "" : ln.replace(/\/\/.*$/, "")))
    .join("\n");
}

function checkForbid(ruleId, forbid) {
  for (const { glob, pattern, note } of forbid) {
    const re = new RegExp(pattern, "m");
    const files = listFiles(glob);
    if (files.length === 0) {
      fail(`[${ruleId}] forbid 的 glob 没有匹配任何文件：${glob}`);
      continue;
    }
    for (const f of files) {
      const text = stripComments(readFileSync(f, "utf8"));
      const m = re.exec(text);
      if (!m) continue;
      const line = text.slice(0, m.index).split("\n").length;
      const rel = path.relative(CRATE, f).replace(/\\/g, "/");
      fail(
        `[${ruleId}] 命中禁止模式 ${rel}:${line}（${note ?? pattern}）\n      ${m[0]
          .trim()
          .split("\n")[0]}`,
      );
    }
  }
}

// ---------------------------------------------------------------------------
// 3) 技能注册表快照
// ---------------------------------------------------------------------------

function rustRegistry() {
  const raw = execFileSync(
    "cargo",
    [
      "run",
      "-q",
      "--manifest-path",
      path.join(ROOT, "crates", "Cargo.toml"),
      "-p",
      "sanguosha2",
      "--example",
      "sgs2-registry",
      "--",
      "--json",
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  const line = raw
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.startsWith("{"))
    .pop();
  if (!line) throw new Error("sgs2-registry 未输出 JSON");
  return JSON.parse(line);
}

function checkSkills() {
  const file = path.join(CONTRACTS, "skills.json");
  let exported;
  try {
    exported = rustRegistry();
  } catch (err) {
    fail(`[skills] 无法读取 Rust 注册表：${err.message}`);
    return;
  }
  const rows = (exported.skills ?? exported.items ?? exported ?? []).map((s) => ({
    code: s.code,
    kind: s.kind ?? "",
  }));
  if (rows.length === 0) {
    fail("[skills] 注册表导出为空");
    return;
  }
  rows.sort((a, b) => String(a.code).localeCompare(String(b.code)));
  if (WRITE) {
    const body = {
      id: "skills",
      title: `技能注册表快照（${rows.length} 条：code + kind）`,
      generated: true,
      note: "由 `node apps/scripts/check-spec-conformance.mjs --write` 从 Rust 注册表生成；不得手改。",
      skills: rows,
    };
    writeFileSync(file, `${JSON.stringify(body, null, 1)}\n`, "utf8");
    console.log(`[spec] 已写入 ${path.relative(ROOT, file)}（${rows.length} 条）`);
    return;
  }
  if (!existsSync(file)) {
    fail("[skills] 缺少 contracts/skills.json（先跑 --write）");
    return;
  }
  const snapshot = JSON.parse(readFileSync(file, "utf8")).skills ?? [];
  const a = new Map(snapshot.map((s) => [String(s.code), String(s.kind ?? "")]));
  const b = new Map(rows.map((s) => [String(s.code), String(s.kind ?? "")]));
  for (const [code, kind] of b) {
    if (!a.has(code)) fail(`[skills] 规格快照缺少技能：${code}（${kind}）—— 先跑 --write 并补契约/用例`);
    else if (a.get(code) !== kind) fail(`[skills] 技能 ${code} 的 kind 不一致：规格 ${a.get(code)} ≠ 代码 ${kind}`);
  }
  for (const code of a.keys()) {
    if (!b.has(code)) fail(`[skills] 规格快照多出技能：${code}（源码已删除？）`);
  }
  if (VERBOSE) console.log(`[skills] 注册表 ${rows.length} 条与快照一致`);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

const fns = collectFns();
const contractFiles = readdirSync(CONTRACTS).filter((f) => f.endsWith(".json") && f !== "skills.json");
let ruleCount = 0;
for (const f of contractFiles) {
  const spec = JSON.parse(readFileSync(path.join(CONTRACTS, f), "utf8"));
  for (const rule of spec.rules ?? []) {
    ruleCount += 1;
    if (rule.anchor) checkAnchor(fns, rule.id, rule.anchor);
    else fail(`[${rule.id}] 缺少 anchor（每条规格规则都必须有用例锚点）`);
    checkForbid(rule.id, rule.forbid ?? []);
    if (VERBOSE) console.log(`  ✓ ${spec.id}/${rule.id}`);
  }
}
checkSkills();

if (problems.length > 0) {
  console.error(`[spec] 规格一致性失败（${problems.length} 项）：`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log(
  `[spec] OK：${contractFiles.length} 个契约 / ${ruleCount} 条规则 / 锚点与禁止模式全部通过${
    WRITE ? "" : "（技能快照一致）"
  }`,
);
