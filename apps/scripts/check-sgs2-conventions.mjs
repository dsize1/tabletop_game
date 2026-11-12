#!/usr/bin/env node
// V2 开发约定门禁（C1–C6：迁移欠账 / 白名单类）。
//
// **规则声明在数据里**：`specs/sanguosha/contracts/conventions.json`
// 本脚本只提供各规则的**检测实现**，并校验「JSON 规则 id ↔ 实现」一一对应
// （少实现或多声明都红）——避免规则只活在脚本里、文档/规格与实现漂移。
//
// 语义不变量（I1 询问完整性 / I2 存活语义）不在这里：
//   → `contracts/interaction.json` / `contracts/state.json`，由 `check:spec-conformance` 执行。
//
// 用法：
//   node apps/scripts/check-sgs2-conventions.mjs                 # 校验
//   node apps/scripts/check-sgs2-conventions.mjs --report        # 只报告（不非零退出）
//   node apps/scripts/check-sgs2-conventions.mjs --write-allowlist  # 重写白名单（需人工复核理由）
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(".");
const SK = path.join(ROOT, "crates/engines/sanguosha2/src/skills");
const KERNEL = path.join(ROOT, "crates/engines/sanguosha2/src/kernel");
const VIEW = path.join(ROOT, "crates/engines/sanguosha2/src/view.rs");
const ALLOW = path.join(ROOT, "docs/games/sanguosha/conventions-allowlist.json");
const SPEC = path.join(ROOT, "specs/sanguosha/contracts/conventions.json");
const REPORT = process.argv.includes("--report");
const WRITE = process.argv.includes("--write-allowlist");

const spec = JSON.parse(fs.readFileSync(SPEC, "utf8"));
const allow = fs.existsSync(ALLOW) ? JSON.parse(fs.readFileSync(ALLOW, "utf8")) : {};
const has = (list, pred) => (allow[list] ?? []).some(pred);

const files = fs.readdirSync(SK).filter((f) => f.endsWith(".rs"));
const src = new Map(files.map((f) => [f, fs.readFileSync(path.join(SK, f), "utf8")]));
const view = fs.readFileSync(VIEW, "utf8");
const visible = new Set(
  [
    ...(
      /pub const VISIBLE_COUNTER_MARKS[^=]*=\s*&\[([\s\S]*?)\];/.exec(view)?.[1] ?? ""
    ).matchAll(/"([a-z0-9_]+)"/g),
  ].map((m) => m[1]),
);
const skillCodes = new Set();
for (const [, text] of src) {
  for (const m of text.matchAll(/code:\s*"([a-z0-9_]+)"/g)) skillCodes.add(m[1]);
}

const violations = [];
const add = (check, file, key, detail) => violations.push({ check, file, key, detail });

// ---------------------------------------------------------------------------
// 各规则的检测实现（key = 规则 id，与 contracts/conventions.json 一一对应）
// ---------------------------------------------------------------------------
const detectors = {
  // 体力直写：技能层 `x.hp = / += / -=`（排除比较运算与注释行）
  "C1-hp": () => {
    for (const [f, text] of src) {
      text.split("\n").forEach((ln, i) => {
        if (/\.hp\s*(=|\+=|-=)/.test(ln) && !/==|!=|<=|>=/.test(ln) && !/^\s*\/\//.test(ln)) {
          add("C1-hp", f, ln.trim(), `第 ${i + 1} 行`);
        }
      });
    }
  },
  // 内核出现技能 code 字面量
  "C2-kernel-skill": () => {
    for (const f of fs.readdirSync(KERNEL).filter((x) => x.endsWith(".rs"))) {
      fs.readFileSync(path.join(KERNEL, f), "utf8")
        .split("\n")
        .forEach((ln, i) => {
          for (const c of skillCodes) {
            if (ln.includes(`"${c}"`)) add("C2-kernel-skill", f, c, `第 ${i + 1} 行`);
          }
        });
    }
  },
  // 座位标记未进视图投影
  "C3-mark": () => {
    for (const [f, text] of src) {
      text.split("\n").forEach((ln, i) => {
        const mark = /(?:marks|counters)\.insert\(\s*"([a-z0-9_]+)"/.exec(ln);
        if (mark && !visible.has(mark[1])) add("C3-mark", f, mark[1], `第 ${i + 1} 行`);
      });
    }
  },
  // 全局计数器缺作用域前缀
  "C4-counter": () => {
    for (const [f, text] of src) {
      text.split("\n").forEach((ln, i) => {
        const counter = /g\.(?:counters\.insert|bump|counter)\(\s*"([a-z0-9_]+)"/.exec(ln);
        if (counter && !/^(turn|phase|round|total)\./.test(counter[1])) {
          add("C4-counter", f, counter[1], `第 ${i + 1} 行`);
        }
      });
    }
  },
  // 「跳过/不弃置」类出口
  "C5-escape": () => {
    for (const [f, text] of src) {
      text.split("\n").forEach((ln, i) => {
        const m = /\.(escape|escape_with)\(\s*"([^"]*(?:跳过|不弃置)[^"]*)"/.exec(ln);
        if (m) add("C5-escape", f, m[2], `第 ${i + 1} 行`);
      });
    }
  },
  // kind 一致性：locked 无主动入口；limited/awaken 有 used 守卫
  "C6-kind": () => {
    for (const [f, text] of src) {
      for (const m of text.matchAll(/SkillDef \{([\s\S]*?)\};/g)) {
        const b = m[1];
        const code = /code:\s*"([a-z0-9_]+)"/.exec(b)?.[1];
        const kind = /kind:\s*SkillKind::(\w+)/.exec(b)?.[1];
        if (!code) continue;
        if (kind === "Locked" && /can_active:\s*Some/.test(b)) {
          add("C6-kind", f, code, "锁定技不得有主动入口");
        }
        if ((kind === "Limited" || kind === "Awaken") && !text.includes(`"${code}_used"`) && !text.includes("_used")) {
          add("C6-kind", f, code, `${kind} 缺 used 守卫`);
        }
      }
    }
  },
};

// 规则声明 ↔ 实现 一一对应
const declared = (spec.rules ?? []).map((r) => r.id);
const implemented = Object.keys(detectors);
const missingImpl = declared.filter((id) => !implemented.includes(id));
const missingDecl = implemented.filter((id) => !declared.includes(id));
if (missingImpl.length > 0) {
  console.error(`[sgs2-conventions] FAIL：规格声明了未实现的规则 ${missingImpl.join(",")}`);
  process.exit(1);
}
if (missingDecl.length > 0) {
  console.error(`[sgs2-conventions] FAIL：实现了未在规格声明的规则 ${missingDecl.join(",")}`);
  process.exit(1);
}

for (const id of declared) detectors[id]();

// ---------------------------------------------------------------------------
// 白名单写入（人工复核理由）
// ---------------------------------------------------------------------------
if (WRITE) {
  const reasons = Object.fromEntries((spec.rules ?? []).map((r) => [r.id, r.reason]));
  const out = {
    generatedAt: new Date().toISOString(),
    note: "显式白名单：新增违反必须补机制或登记理由（规则声明见 specs/sanguosha/contracts/conventions.json）",
  };
  const buckets = { kernelLiterals: [], internalMarks: [], internalCounters: [], escapes: [], kindWaivers: [] };
  for (const v of violations) {
    const base = { reason: reasons[v.check] };
    if (v.check === "C2-kernel-skill") buckets.kernelLiterals.push({ ...base, file: v.file, code: v.key });
    else if (v.check === "C3-mark") buckets.internalMarks.push({ ...base, key: v.key });
    else if (v.check === "C4-counter") buckets.internalCounters.push({ ...base, key: v.key });
    else if (v.check === "C5-escape") buckets.escapes.push({ ...base, file: v.file, label: v.key });
    else if (v.check === "C6-kind") buckets.kindWaivers.push({ ...base, file: v.file, code: v.key });
  }
  const uniq = (arr, k) => [...new Map(arr.map((x) => [k(x), x])).values()];
  out.kernelLiterals = uniq(buckets.kernelLiterals, (x) => x.file + x.code);
  out.internalMarks = uniq(buckets.internalMarks, (x) => x.key);
  out.internalCounters = uniq(buckets.internalCounters, (x) => x.key);
  out.escapes = uniq(buckets.escapes, (x) => x.file + x.label);
  out.kindWaivers = uniq(buckets.kindWaivers, (x) => x.file + x.code);
  fs.writeFileSync(ALLOW, `${JSON.stringify(out, null, 2)}\n`);
  console.log(
    `[sgs2-conventions] 已写入白名单 ${path.relative(ROOT, ALLOW)}（${violations.length} 条）`,
  );
  process.exit(0);
}

const allowed = (v) =>
  has(v.check === "C2-kernel-skill" ? "kernelLiterals" : "", (e) => e.file === v.file && e.code === v.key) ||
  has(v.check === "C3-mark" ? "internalMarks" : "", (e) => e.key === v.key) ||
  has(v.check === "C4-counter" ? "internalCounters" : "", (e) => e.key === v.key) ||
  has(v.check === "C5-escape" ? "escapes" : "", (e) => e.file === v.file && e.label === v.key) ||
  has(v.check === "C6-kind" ? "kindWaivers" : "", (e) => e.file === v.file && e.code === v.key);

const bad = violations.filter((v) => !allowed(v));
const byCheck = {};
for (const v of violations) byCheck[v.check] = (byCheck[v.check] ?? 0) + 1;
console.log(
  `[sgs2-conventions] 规则 ${declared.length} 条（${declared.join("/")}）· 命中：`,
  byCheck,
  `· 白名单内 ${violations.length - bad.length}`,
);
console.log(
  `[sgs2-conventions] I1/I2 不变量不在此脚本：见 interaction.json / state.json（check:spec-conformance）`,
);
for (const v of bad.slice(0, 40)) console.log(`  ✗ ${v.check} ${v.file} ${v.key} ${v.detail}`);
if (bad.length > 0) {
  console.log(
    `[sgs2-conventions] FAIL：${bad.length} 处未登记（新增违反请补机制或显式登记白名单+理由）`,
  );
  process.exit(REPORT ? 0 : 1);
}
console.log("[sgs2-conventions] OK");
