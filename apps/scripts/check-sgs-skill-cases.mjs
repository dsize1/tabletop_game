#!/usr/bin/env node
// P4.1-D 门禁 / P4.4 扩围：「已实现武将」每武将 ≥3 条规则用例。
//
// 统计口径：扫描 `crates/engines/sanguosha/src/tests/` 下所有 `#[test]` 函数体，
// 若测试体出现该武将的 `sgs_hero_<gid>` 或该武将任一技能 code（带引号），
// 即认为该用例覆盖该武将。
//
// 覆盖范围（P4.4）：从 `skills.rs` 解析「已注册 + 全部 implemented」的武将，
// 从 `data.rs` 解析 gid → 武将名。未实现的武将不要求用例（也不进选将池）。
//
// 用法：node apps/scripts/check-sgs-skill-cases.mjs [--min 3] [--verbose]

import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

const TESTS_DIR = resolve("crates", "engines", "sanguosha", "src", "tests");
const SRC_DIR = resolve("crates", "engines", "sanguosha", "src");
const SKILLS_RS = resolve("crates", "engines", "sanguosha", "src", "skills.rs");
const DATA_RS = resolve("crates", "engines", "sanguosha", "src", "data.rs");

function parseArgs(argv) {
  const opts = { min: 3, verbose: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--min") opts.min = Number(argv[++i]);
    else if (argv[i] === "--verbose" || argv[i] === "-v") opts.verbose = true;
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log("用法：node apps/scripts/check-sgs-skill-cases.mjs [--min 3] [--verbose]");
      process.exit(0);
    } else {
      console.error(`未知参数：${argv[i]}`);
      process.exit(1);
    }
  }
  if (!Number.isInteger(opts.min) || opts.min < 1) {
    console.error("--min 必须为正整数");
    process.exit(1);
  }
  return opts;
}

const TEST_RE = /#\[test\][\s\S]*?\n\s*fn\s+([A-Za-z0-9_]+)\s*\(/g;

async function collectTests() {
  const files = (await readdir(TESTS_DIR)).filter((f) => f.endsWith(".rs"));
  const tests = [];
  for (const file of files) {
    const text = await readFile(resolve(TESTS_DIR, file), "utf8");
    const marks = [...text.matchAll(TEST_RE)].map((m) => ({
      name: m[1],
      start: m.index,
    }));
    for (let i = 0; i < marks.length; i++) {
      const start = marks[i].start;
      const end = i + 1 < marks.length ? marks[i + 1].start : text.length;
      tests.push({ file, name: marks[i].name, body: text.slice(start, end) });
    }
  }
  return tests;
}

/** gid → 武将名（data.rs）。 */
/**
 * 觉醒授予技能检查：`<skill>_awaken` 中的 `grant_skill(st, seat, "<extra>")`，
 * 若授予的技能未实现（`implemented:false`），输出告警（`--strict-granted` 时失败）。
 */
async function loadGrantedInactive() {
  const skillsRs = await readFile(SKILLS_RS, "utf8");
  const extraRe = /pub static EXTRA_SKILLS:\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\];/;
  const m = extraRe.exec(skillsRs);
  const extras = new Map();
  if (m) {
    for (const e of m[1].split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
      const name = /name:\s*"([^"]+)"/.exec(e)?.[1];
      if (!code || !name) continue;
      extras.set(code, { name, implemented: /implemented:\s*true/.test(e) });
    }
  }
  if (extras.size === 0) return [];
  const files = (await readdir(SRC_DIR)).filter(
    (f) => f.startsWith("resolve") && f.endsWith(".rs"),
  );
  const out = [];
  for (const f of files) {
    const text = await readFile(resolve(SRC_DIR, f), "utf8");
    const fnRe = /fn\s+([a-z0-9_]+)\s*\([\s\S]*?\n\}/g;
    for (const fm of text.matchAll(fnRe)) {
      const fnRaw = fm[1];
      const skillCode = /^([a-z0-9]+)_(awaken|awakening)$/.exec(fnRaw)?.[1];
      if (!skillCode) continue;
      for (const gm of fm[0].matchAll(/grant_skill\(st,\s*[^,]+,\s*"([a-z0-9_]+)"\)/g)) {
        const granted = extras.get(gm[1]);
        if (granted && !granted.implemented) {
          out.push({ skillCode, extra: gm[1], extraName: granted.name, file: f });
        }
      }
    }
  }
  return out;
}

async function loadGeneralNames() {
  const text = await readFile(DATA_RS, "utf8");
  const names = new Map();
  for (const m of text.matchAll(/code: "(sgs_hero_\d+)", gid: (\d+), name: "([^"]+)"/g)) {
    names.set(Number(m[2]), m[3]);
  }
  return names;
}

/**
 * 解析 skills.rs：
 * - `const XXX: &[SkillDef] = &[...]` → { code, implemented } 列表；
 * - `"sgs_hero_<gid>" => XXX` → gid → 技能列表。
 * 只返回「全部 implemented」的武将。
 */
async function loadImplementedGenerals() {
  const text = await readFile(SKILLS_RS, "utf8");
  const consts = new Map();
  for (const m of text.matchAll(
    /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g,
  )) {
    const entries = [];
    for (const e of m[2].split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
      if (!code) continue;
      entries.push({ code, implemented: /implemented:\s*true/.test(e) });
    }
    consts.set(m[1], entries);
  }
  const generals = new Map();
  for (const m of text.matchAll(/"sgs_hero_(\d+)"\s*=>\s*([A-Z0-9_]+)/g)) {
    const gid = Number(m[1]);
    const skills = consts.get(m[2]);
    if (!skills || skills.length === 0) continue;
    if (!skills.every((s) => s.implemented)) continue;
    generals.set(gid, skills.map((s) => s.code));
  }
  return generals;
}

function covers(body, gid, skills) {
  if (body.includes(`sgs_hero_${gid}`)) return true;
  return skills.some((code) => body.includes(`"${code}"`));
}

const opts = parseArgs(process.argv.slice(2));
const tests = await collectTests();
if (!tests.length) {
  console.error(`[check] 未找到任何 #[test]：${TESTS_DIR}`);
  process.exit(1);
}

const names = await loadGeneralNames();
const generals = await loadImplementedGenerals();
if (generals.size === 0) {
  console.error("[check] 未解析到任何已实现武将（skills.rs）");
  process.exit(1);
}

const counts = {};
const covered = {};
for (const gid of generals.keys()) {
  counts[gid] = 0;
  covered[gid] = [];
}
for (const t of tests) {
  for (const [gid, skills] of generals) {
    if (covers(t.body, gid, skills)) {
      counts[gid] += 1;
      covered[gid].push(`${t.file}::${t.name}`);
    }
  }
}

const missing = [];
for (const gid of generals.keys()) {
  if (opts.verbose || counts[gid] < opts.min) {
    console.log(`  ${counts[gid]}\t${names.get(gid) ?? gid} (sgs_hero_${gid})`);
  }
  if (counts[gid] < opts.min) missing.push(gid);
}

if (missing.length) {
  console.error(
    `[check] 失败：${missing.length} 位武将用例不足 ${opts.min} 条：` +
      missing.map((gid) => `${names.get(gid) ?? gid}(sgs_hero_${gid})=${counts[gid]}`).join("、"),
  );
  for (const gid of missing) {
    console.error(`  ${names.get(gid) ?? gid} 现有用例：${covered[gid].join(", ") || "（无）"}`);
  }
  process.exit(1);
}

// 转换技（ADR-0028）：要求「阳 / 阴」两面均有用例覆盖。
// 判定口径：测试体**写入** `flags.insert("yj:<code>", true)` → 阴面用例；
// 其余引用该技能 code 的用例 → 阳面用例（显式 `flags.remove("yj:<code>")` 视为阳面）。
const TRANSFORM_SKILLS = ["juzhan", "shenshi", "chenglue", "zhenliang"];
const transformMissing = [];
console.log("[check] 转换技两面覆盖：");
for (const code of TRANSFORM_SKILLS) {
  const registered = [...generals.values()].some((skills) => skills.includes(code));
  if (!registered) continue;
  const yinMarks = [
    `flags.insert("yj:${code}".into(), true)`,
    `flags.insert("yj:${code}", true)`,
  ];
  const refs = tests.filter(
    (t) =>
      t.body.includes(`"${code}"`) ||
      t.body.includes(`choose:${code}`) ||
      t.body.includes(`skill:${code}`) ||
      t.body.includes(`yj:${code}`),
  );
  const isYin = (body) => yinMarks.some((m) => body.includes(m));
  const yin = refs.filter((t) => isYin(t.body));
  const yang = refs.filter((t) => !isYin(t.body));
  const ok = yang.length > 0 && yin.length > 0;
  console.log(`  ${ok ? "OK " : "FAIL"} ${code}：阳 ${yang.length} 条 / 阴 ${yin.length} 条`);
  if (!ok) transformMissing.push(`${code}(阳=${yang.length},阴=${yin.length})`);
}
if (transformMissing.length) {
  console.error(
    `[check] 失败：转换技两面覆盖不足：${transformMissing.join("、")}` +
      '（阴面用例需在测试内写 `flags.insert("yj:<code>", true)`）',
  );
  process.exit(1);
}

// 觉醒授予技能已实现性（提示；`--strict-granted` 时作为失败项）。
const strictGranted = process.argv.includes("--strict-granted");
const inactiveGranted = await loadGrantedInactive();
if (inactiveGranted.length > 0) {
  console.log("[check] 觉醒授予技能：");
  for (const g of inactiveGranted) {
    console.log(
      `  ${strictGranted ? "FAIL" : "WARN"} ${g.skillCode} 授予 ${g.extra}（${g.extraName}）未实现` +
        "（待补官方文本；见 skills.md §4.1）",
    );
  }
  if (strictGranted) {
    console.error("[check] 失败：存在未实现的授予技能（--strict-granted）");
    process.exit(1);
  }
}

const min = Math.min(...Object.values(counts));
console.log(
  `[check] OK：${tests.length} 条测试 / ${generals.size} 位已实现武将均 ≥${opts.min} 条（最低 ${min}）`,
);
