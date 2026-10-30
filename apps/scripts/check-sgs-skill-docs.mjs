#!/usr/bin/env node
// 门禁：`skills.md` 已实现武将技能表 ↔ `skills.rs` 注册表一致性（P4.4.1 harness）。
//
// 目的：防止「代码加了技能但文档漏登记」或「文档写了技能但代码未注册/名字漂移」。
// 口径（ADR-0028 / skills.md §2–§2.5）：
//   - 代码侧：`const X: &[SkillDef] = &[...]` + `"sgs_hero_<gid>" => X`，仅比较 implemented:true；
//   - 文档侧：markdown 表格行 `| 武将 `sgs_hero_N` | 技能 | `code` | 类型 | ... |`；
//   - 类型：普通=normal / 锁定=locked / 主公=lord / 限定=limited / 觉醒=awaken。
//
// 用法：node apps/scripts/check-sgs-skill-docs.mjs [--verbose]

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const SKILLS_RS = resolve("crates", "engines", "sanguosha", "src", "skills.rs");
const DATA_RS = resolve("crates", "engines", "sanguosha", "src", "data.rs");
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
  "主公/觉醒": ["lord", "awaken"],
};

const verbose = process.argv.includes("--verbose") || process.argv.includes("-v");

async function loadCodeSkills() {
  const text = await readFile(SKILLS_RS, "utf8");
  const consts = new Map();
  for (const m of text.matchAll(
    /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g,
  )) {
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

async function loadDocRows() {
  const text = await readFile(SKILLS_MD, "utf8");
  const rows = new Map(); // gid → [{name, code, kindLabel}]
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
const names = await loadGeneralNames();

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
  if (verbose) console.log(`  ${label}：${codeSkills.length} 技能`);
}

if (problems.length) {
  console.error(`[check] 失败：${problems.length} 处文档-代码不一致`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}

console.log(
  `[check] OK：${checked} 条技能登记与 skills.md 一致（${code.size} 位已实现武将）`,
);
