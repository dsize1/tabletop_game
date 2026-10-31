// 三国杀脚本共用数据读取（P4.4.2 harness）。
//
// 供 `sgs-batch-status.mjs` / `sgs-batch-scaffold.mjs` 复用：
// - skills.rs 已实现武将
// - data.rs 武将元数据
// - development-plan.md 批次表
// - p67「一将成名已实现数」断言值

import { readFileSync } from "node:fs";

export const PATHS = {
  skillsRs: "crates/engines/sanguosha/src/skills.rs",
  dataRs: "crates/engines/sanguosha/src/data.rs",
  planMd: "docs/games/sanguosha/development-plan.md",
  p67Rs: "crates/engines/sanguosha/src/tests/p67.rs",
};

export const PACK_LABEL = {
  standard: "标准",
  wind_fire_forest_mountain: "风林火山",
  yijiang: "一将成名",
  jiexian: "界限突破",
};

export function readFile(path) {
  return readFileSync(path, "utf8");
}

/** skills.rs：gid → 技能 code 列表（技能非空且全部 implemented:true）。 */
export function loadReadyGenerals() {
  const text = readFile(PATHS.skillsRs);
  const consts = new Map();
  const defRe = /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g;
  for (const m of text.matchAll(defRe)) {
    const entries = [];
    for (const e of m[2].split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(e)?.[1];
      if (!code) continue;
      entries.push({ code, implemented: /implemented:\s*true/.test(e) });
    }
    consts.set(m[1], entries);
  }
  const ready = new Map();
  for (const m of text.matchAll(/"sgs_hero_(\d+)"\s*=>\s*([A-Z0-9_]+)/g)) {
    const gid = Number(m[1]);
    const skills = consts.get(m[2]);
    if (!skills || skills.length === 0) continue;
    if (!skills.every((s) => s.implemented)) continue;
    ready.set(gid, skills.map((s) => s.code));
  }
  return ready;
}

/** data.rs：gid → { code, gid, name, pack, kingdom, max_hp, gender, skills[] }。 */
export function loadGenerals() {
  const text = readFile(PATHS.dataRs);
  const out = new Map();
  const re =
    /GeneralDef \{ code: "(sgs_hero_\d+)", gid: (\d+), name: "([^"]+)", pack: "([a-z_]+)", kingdom: "([a-z]+)", max_hp: (\d+), gender: "([a-z]+)", is_lord: (true|false), skills: &\[([^\]]*)\]/g;
  for (const m of text.matchAll(re)) {
    out.set(Number(m[2]), {
      code: m[1],
      gid: Number(m[2]),
      name: m[3],
      pack: m[4],
      kingdom: m[5],
      max_hp: Number(m[6]),
      gender: m[7],
      is_lord: m[8] === "true",
      skills: [...m[9].matchAll(/"([^"]+)"/g)].map((s) => s[1]),
    });
  }
  return out;
}

/** development-plan.md：批次表行（`| Y3 | 187 王异、... |`）。 */
export function loadBatches() {
  const text = readFile(PATHS.planMd);
  const batches = [];
  for (const line of text.split(/\r?\n/)) {
    const m = /^\|\s*(Y\d+)(\s*✅)?\s*\|\s*(.+?)\s*\|\s*$/.exec(line);
    if (!m) continue;
    const entries = [];
    for (const e of m[3].matchAll(/(\d{3})\s+([^、|（(]+)/g)) {
      entries.push({ gid: Number(e[1]), name: e[2].trim() });
    }
    if (entries.length === 0) continue;
    batches.push({ batch: m[1], done: Boolean(m[2]), entries });
  }
  return batches;
}

/** p67：「一将成名已实现 N 位」断言值。 */
export function loadP67Count() {
  const text = readFile(PATHS.p67Rs);
  const m = /assert_eq!\(\s*yijiang\.len\(\),\s*(\d+)/.exec(text);
  return m ? Number(m[1]) : null;
}

/** 批次完成度报告 + 下一批。 */
export function batchReport() {
  const ready = loadReadyGenerals();
  const batches = loadBatches();
  const report = batches.map((b) => {
    const done = b.entries.filter((e) => ready.has(e.gid));
    const missing = b.entries.filter((e) => !ready.has(e.gid));
    return { ...b, doneCount: done.length, total: b.entries.length, missing };
  });
  return {
    ready,
    report,
    next: report.find((b) => b.doneCount < b.total) ?? null,
  };
}
