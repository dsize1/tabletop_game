#!/usr/bin/env node
// 门禁：`skills.md` 已实现武将技能表 ↔ `skills.rs` 注册表一致性（P4.4.1 harness）。
//
// 目的：防止「代码加了技能但文档漏登记」或「文档写了技能但代码未注册/名字漂移」。
// 口径（ADR-0028 / ADR-0029 / skills.md §2–§2.6）：
//   - 代码侧：`const X: &[SkillDef] = &[...]` + `"sgs_hero_<gid>" => X`，仅比较 implemented:true；
//   - 文档侧：markdown 表格行 `| 武将 `sgs_hero_N` | 技能 | `code` | 类型 | ... |`；
//   - 类型：普通=normal / 锁定=locked / 主公=lord / 限定=limited / 觉醒=awaken；
//   - 授予技能（EXTRA_SKILLS，觉醒获得）：必须登记在 §4.1「授予技能」表；
//     未实现者状态须标注「待补」（保留记录、不阻断），且必须有 grant_skill 调用点。
//
// 用法：node apps/scripts/check-sgs-skill-docs.mjs [--verbose]

import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";

const SRC_DIR = resolve("crates", "engines", "sanguosha", "src");
const SKILLS_RS = resolve(SRC_DIR, "skills.rs");
const DATA_RS = resolve(SRC_DIR, "data.rs");
const SKILLS_MD = resolve("docs", "games", "sanguosha", "skills.md");

// 文档类型列 → 允许的代码 kind（「转换」= normal + 转换技标注；「主公/觉醒」= 双类型）。
const DOC_KIND_ALIASES = {
  普通: ["normal"],
  锁定: ["locked"],
  主公: ["lord"],
  主公技: ["lord"],
  限定: ["limited"],
  觉醒: ["awaken"],
  转换: ["normal"],
  // 主公技同时是限定技时（如曹叡·兴衰），允许合并标注。
  "主公/限定": ["lord"],
  "限定/主公": ["lord"],
  "主公/觉醒": ["lord", "awaken"],
};

const verbose = process.argv.includes("--verbose") || process.argv.includes("-v");

/** 由 kind 反查文档类型标签（多映射时取第一个）。 */
function kindLabel(kind) {
  for (const [label, kinds] of Object.entries(DOC_KIND_ALIASES)) {
    if (kinds.includes(kind)) return label;
  }
  return null;
}

async function loadCodeSkills() {
  const text = await readFile(SKILLS_RS, "utf8");
  const consts = new Map();
  const defRe = /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g;
  for (const m of text.matchAll(defRe)) {
    const entries = [];
    for (const e of m[2].split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
      const name = /name:\s*"([^"]+)"/.exec(e)?.[1];
      const kind = /kind:\s*SkillType::(\w+)/.exec(e)?.[1]?.toLowerCase();
      if (!code || !name || !kind) continue;
      entries.push({ code, name, kind, implemented: /implemented:\s*true/.test(e) });
    }
    consts.set(m[1], entries);
  }
  const byGid = new Map();
  for (const m of text.matchAll(/"sgs_hero_(\d+)"\s*=>\s*([A-Z0-9_]+)/g)) {
    const gid = Number(m[1]);
    const skills = consts.get(m[2]);
    if (!skills || skills.length === 0) continue;
    byGid.set(gid, skills.filter((s) => s.implemented));
  }
  return byGid;
}

/** 解析 `EXTRA_SKILLS`（授予技能，如觉醒获得的清侧/怀柔）。 */
async function loadExtraSkills() {
  const text = await readFile(SKILLS_RS, "utf8");
  const re = /pub static EXTRA_SKILLS:\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\];/;
  const m = re.exec(text);
  if (!m) return [];
  const out = [];
  for (const e of m[1].split("SkillDef {").slice(1)) {
    const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
    const name = /name:\s*"([^"]+)"/.exec(e)?.[1];
    const kind = /kind:\s*SkillType::(\w+)/.exec(e)?.[1]?.toLowerCase();
    if (!code || !name || !kind) continue;
    out.push({ code, name, kind, implemented: /implemented:\s*true/.test(e) });
  }
  return out;
}

async function loadDocRows() {
  const text = await readFile(SKILLS_MD, "utf8");
  const rows = new Map();
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith("|")) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((c) => c.trim());
    if (cells.length < 4) continue;
    const gidMatch = /sgs_hero_(\d+)/.exec(cells[0]);
    if (!gidMatch) continue;
    const code = /^`([a-z0-9_]+)`$/.exec(cells[2])?.[1];
    if (!code) continue;
    const gid = Number(gidMatch[1]);
    const list = rows.get(gid) ?? [];
    list.push({ name: cells[1], code, kindLabel: cells[3] });
    rows.set(gid, list);
  }
  return rows;
}

/** 解析 `skills.md` §4.1「授予技能」表：`| 来源技能 | 技能 | code | 类型 | 状态 |`。 */
async function loadGrantedRows() {
  const text = await readFile(SKILLS_MD, "utf8");
  const rows = new Map();
  const start = text.indexOf("### 4.1");
  if (start < 0) return rows;
  const section = text.slice(start);
  const end = section.indexOf("\n## ");
  const body = end >= 0 ? section.slice(0, end) : section;
  for (const line of body.split(/\r?\n/)) {
    if (!line.startsWith("|")) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((c) => c.trim());
    if (cells.length < 5) continue;
    const code = /^`([a-z0-9_]+)`$/.exec(cells[2])?.[1];
    if (!code) continue;
    rows.set(code, { name: cells[1], kindLabel: cells[3], status: cells[4] });
  }
  return rows;
}

/**
 * 扫描 `src/resolve_*.rs`：`grant_skill(st, seat, "<code>")` 及其所属函数名
 * → 〔来源函数 → 授予技能 code 集合〕。
 */
async function loadGrantSites() {
  const files = (await readdir(SRC_DIR)).filter(
    (f) => f.startsWith("resolve") && f.endsWith(".rs"),
  );
  const sites = new Map();
  for (const f of files) {
    const text = await readFile(resolve(SRC_DIR, f), "utf8");
    for (const m of text.matchAll(/grant_skill\(st,\s*[^,]+,\s*"([a-z0-9_]+)"\)/g)) {
      const before = text.slice(0, m.index);
      const fn = [...before.matchAll(/fn\s+([a-z0-9_]+)\s*\(/g)].pop()?.[1] ?? "?";
      const set = sites.get(fn) ?? new Set();
      set.add(m[1]);
      sites.set(fn, set);
    }
  }
  return sites;
}

/**
 * 解析 `data.rs`：`GeneralDef { ..., skills: &["a", "b"], }` → gid → 技能名数组。
 * 用于校验「文档技能名」与该武将的数据技能名一一对应（防 gid 错位/漏登记）。
 */
async function loadDataSkillNames() {
  const text = await readFile(DATA_RS, "utf8");
  const map = new Map();
  const re = /GeneralDef \{ code: "sgs_hero_\d+", gid: (\d+),[^}]*skills: &\[([^\]]*)\]/g;
  for (const m of text.matchAll(re)) {
    const gid = Number(m[1]);
    const names = [...m[2].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
    map.set(gid, names);
  }
  return map;
}

async function loadGeneralNames() {
  const text = await readFile(DATA_RS, "utf8");
  const names = new Map();
  for (const m of text.matchAll(/code: "sgs_hero_\d+", gid: (\d+), name: "([^"]+)"/g)) {
    names.set(Number(m[1]), m[2]);
  }
  return names;
}

const code = await loadCodeSkills();
const docs = await loadDocRows();
const extras = await loadExtraSkills();
const granted = await loadGrantedRows();
const grantSites = await loadGrantSites();
const names = await loadGeneralNames();
const dataSkills = await loadDataSkillNames();

if (code.size === 0) {
  console.error("[check] 未解析到任何已实现武将（skills.rs）");
  process.exit(1);
}
if (docs.size === 0) {
  console.error(`[check] 未解析到任何技能表行（${SKILLS_MD}）`);
  process.exit(1);
}

const problems = [];
let checked = 0;

for (const [gid, codeSkills] of code) {
  const label = `${names.get(gid) ?? gid}(sgs_hero_${gid})`;
  const docSkills = docs.get(gid);
  if (!docSkills) {
    problems.push(`${label}：skills.md 缺少该武将的技能行`);
    continue;
  }
  const docByCode = new Map(docSkills.map((s) => [s.code, s]));
  const codeByCode = new Map(codeSkills.map((s) => [s.code, s]));
  for (const [c, skill] of codeByCode) {
    checked += 1;
    const doc = docByCode.get(c);
    if (!doc) {
      problems.push(`${label}：代码已注册 ${c}（${skill.name}），skills.md 未登记`);
      continue;
    }
    if (doc.name !== skill.name) {
      problems.push(`${label}·${c}：技能名不一致（代码「${skill.name}」/ 文档「${doc.name}」）`);
    }
    const allowed = DOC_KIND_ALIASES[doc.kindLabel];
    if (!allowed) {
      problems.push(`${label}·${c}：文档类型「${doc.kindLabel}」不是已登记的类型标签`);
    } else if (!allowed.includes(skill.kind)) {
      problems.push(
        `${label}·${c}：类型不一致（代码 ${skill.kind} / 文档「${doc.kindLabel}」→ ${allowed.join("/")}）`,
      );
    }
  }
  for (const c of docByCode.keys()) {
    if (!codeByCode.has(c)) {
      problems.push(`${label}：skills.md 登记了 ${c}，代码未注册或未实现`);
    }
  }
  // 文档技能名 ↔ data.rs 该武将的技能名列表（防 gid 错位/漏登记）。
  const dataNames = dataSkills.get(gid);
  if (!dataNames) {
    problems.push(`${label}：data.rs 缺少该武将的技能名列表`);
  } else {
    const docNames = new Set(docSkills.map((s) => s.name));
    for (const name of dataNames) {
      if (!docNames.has(name)) {
        problems.push(`${label}：data.rs 技能「${name}」在 skills.md 未登记（可能 gid 错位）`);
      }
    }
    for (const name of docNames) {
      if (!dataNames.includes(name)) {
        problems.push(`${label}：skills.md 技能「${name}」不在 data.rs 技能名列表（可能 gid 错位）`);
      }
    }
  }
  if (verbose) console.log(`  ${label}：${codeSkills.length} 技能`);
}

// 授予技能（EXTRA_SKILLS）↔ skills.md §4.1；未实现者必须显式登记「待补」并留调用点。
const inactiveGranted = [];
for (const skill of extras) {
  const row = granted.get(skill.code);
  if (!row) {
    problems.push(
      `授予技能 ${skill.code}（${skill.name}）未在 skills.md §4.1 登记（实现状态：${
        skill.implemented ? "已实现" : "未实现"
      }）`,
    );
    continue;
  }
  if (row.name !== skill.name) {
    problems.push(
      `授予技能 ${skill.code}：名称不一致（代码「${skill.name}」/ 文档「${row.name}」）`,
    );
  }
  const expect = kindLabel(skill.kind);
  if (expect && row.kindLabel !== expect) {
    problems.push(`授予技能 ${skill.code}：类型不一致（代码 ${skill.kind} / 文档「${row.kindLabel}」）`);
  }
  if (!skill.implemented) {
    inactiveGranted.push(`${skill.code}（${skill.name}）`);
    if (!row.status.includes("待补")) {
      problems.push(`授予技能 ${skill.code}：未实现，状态应标注「待补文本」（当前「${row.status}」）`);
    }
  }
  const grantedBy = [...grantSites.entries()]
    .filter(([, set]) => set.has(skill.code))
    .map(([fn]) => fn);
  if (grantedBy.length === 0) {
    problems.push(
      `授予技能 ${skill.code}（${skill.name}）没有任何 grant_skill 调用点（未接入觉醒/化身）`,
    );
  } else if (verbose) {
    console.log(`  ${skill.code}：授予自 ${grantedBy.join("、")}`);
  }
}

if (problems.length) {
  console.error(`[check] 失败：${problems.length} 处文档-代码不一致`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}

console.log(
  `[check] OK：${checked} 条技能登记与 skills.md 一致（${code.size} 位已实现武将）`,
);
if (inactiveGranted.length > 0) {
  console.log(
    `[check] 提示：${inactiveGranted.length} 个授予技能待补官方文本（已留记录、不阻断）：` +
      inactiveGranted.join("、"),
  );
}
