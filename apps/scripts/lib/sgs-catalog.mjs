// 三国杀武将/技能名录解析（验证批次、审计脚本、文档门禁共用单一事实源）。
//
// 数据来源：
// - docs/games/sanguosha/data/engine-catalog.json → 232 将自己（code/gid/name/pack/kingdom/max_hp/gender/is_lord/skills）
//   **生成物**（`npm run gen:sgs-engine-data`）；v1 `data.rs` 退役后仍是唯一机器源。
// - crates/engines/sanguosha/src/skills.rs → 技能实现表（code/name/kind/implemented）+ 武将→技能映射 + EXTRA_SKILLS
//   （v1 删除前保留；v2 注册表接管后迁移，见计划 M4）
//
// 口径（用户 2026-11-05 裁定「方案 A」）：
// 有界版同名武将的原版不进验证批次（界优先去重），剩 148 位 = 默认选将池（232 − 84）。

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const projectRoot = path.resolve(__dirname, "../../..");
const CATALOG_JSON = path.join(
  projectRoot,
  "docs/games/sanguosha/data/engine-catalog.json",
);
const SKILLS_RS = path.join(projectRoot, "crates/engines/sanguosha/src/skills.rs");


const SKILL_CONST_RE =
  /const\s+([A-Z0-9_]+):\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\s*\}?\];/g;
const SKILL_MAP_RE = /"sgs_hero_(\d+)"\s*=>\s*([A-Z0-9_]+)/g;
const EXTRA_RE = /pub static EXTRA_SKILLS:\s*&\[SkillDef\]\s*=\s*&\[([\s\S]*?)\n\];/;

function parseSkillEntries(block) {
  const out = [];
  for (const chunk of block.split("SkillDef {").slice(1)) {
    const code = /code:\s*"([a-z0-9_]+)"/.exec(chunk)?.[1];
    const name = /name:\s*"([^"]+)"/.exec(chunk)?.[1];
    if (!code || !name) continue;
    const kind =
      /kind:\s*SkillType::(\w+)/.exec(chunk)?.[1]?.toLowerCase() ?? "unknown";
    out.push({ code, name, kind, implemented: /implemented:\s*true/.test(chunk) });
  }
  return out;
}

/** 读取生成的名录 JSON（`npm run gen:sgs-engine-data`）。 */
export function loadEngineCatalog() {
  return JSON.parse(readFileSync(CATALOG_JSON, "utf8"));
}

/** 解析全部武将（232；数据源 = engine-catalog.json）。 */
export function loadGenerals() {
  return loadEngineCatalog().generals.map((g) => ({
    code: g.code,
    gid: g.gid,
    name: g.name,
    pack: g.pack,
    kingdom: g.kingdom,
    maxHp: g.max_hp,
    gender: g.gender,
    isLord: g.is_lord,
    skills: g.skills,
    skillNames: g.skills,
  }));
}

/** 解析技能注册表：{ consts, byGid, extras }。 */
export function loadSkills() {
  const text = readFileSync(SKILLS_RS, "utf8");
  const consts = new Map();
  for (const m of text.matchAll(SKILL_CONST_RE)) {
    const entries = parseSkillEntries(m[2]);
    if (entries.length) consts.set(m[1], entries);
  }
  const byGid = new Map();
  for (const m of text.matchAll(SKILL_MAP_RE)) {
    const defs = consts.get(m[2]);
    if (defs) byGid.set(Number(m[1]), defs);
  }
  const extraBlock = EXTRA_RE.exec(text)?.[1] ?? "";
  return { consts, byGid, extras: parseSkillEntries(extraBlock) };
}

/**
 * 完整名录：232 将 + 技能 + 147 审计范围 + 85 排除清单（232 − 85 = 147）。
 *
 * `audited` = 有界版同名的原版被剔除后的武将（顺序同 engine-catalog.json：standard → wind… → yijiang → jiexian）。
 * 同名判定剔除前缀「界」「将」（2026-11-06 裁定：「将华雄」与「界华雄」重复，剔除前缀后同名即视为重复）。
 */
export function loadCatalog() {
  const generals = loadGenerals();
  const { byGid, extras } = loadSkills();

  const jieBase = new Map();
  // 去重口径：剔除名字前缀「界」「将」后同名 → 视为重复（方案 A，2026-11-06 起含「将X」）。
  const dedupeName = (name) => name.replace(/^(?:界|将)/, "");
  for (const g of generals) {
    if (g.pack !== "jiexian") continue;
    jieBase.set(dedupeName(g.name), g);
  }

  const excluded = [];
  const audited = [];
  for (const g of generals) {
    const skills = byGid.get(g.gid) ?? [];
    const entry = {
      ...g,
      isJiexian: g.pack === "jiexian",
      skills,
      implemented: skills.length > 0 && skills.every((s) => s.implemented),
    };
    if (g.pack !== "jiexian" && jieBase.has(dedupeName(g.name))) {
      excluded.push({ ...entry, excludedBy: jieBase.get(dedupeName(g.name)).code });
    } else {
      audited.push(entry);
    }
  }

  const byCode = new Map(generals.map((g) => [g.code, g]));
  const skillCodes = new Set();
  for (const g of audited) for (const s of g.skills) skillCodes.add(s.code);
  for (const s of extras) skillCodes.add(s.code);

  return {
    generals,
    audited,
    excluded,
    byCode,
    extraSkills: extras,
    skillCount: skillCodes.size,
    stats: {
      generals: generals.length,
      audited: audited.length,
      excluded: excluded.length,
      skills: skillCodes.size,
      packs: [...new Set(generals.map((g) => g.pack))],
    },
  };
}

/** 名字列表 → 武将（校验用；找不到直接抛错，避免批次表与 engine-catalog.json 漂移）。 */
export function resolveNames(generals, names, label = "批次") {
  const byName = new Map(generals.map((g) => [g.name, g]));
  const out = [];
  const missing = [];
  for (const name of names) {
    const g = byName.get(name);
    if (!g) missing.push(name);
    else out.push(g);
  }
  if (missing.length) {
    throw new Error(`${label} 中的武将名在 engine-catalog.json 找不到：${missing.join("、")}`);
  }
  return out;
}
