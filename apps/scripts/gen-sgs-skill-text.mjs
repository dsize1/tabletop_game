#!/usr/bin/env node
// 从 `docs/games/sanguosha/data/heroes.json` 生成前端「技能简述」表：
//   apps/web/src/view/sgs/skill-text.ts
//
// 用途：长按技能按钮 → 详情浮层（P3 手势基座）。
// 生成物只含文本，前端按「武将名 + 技能名」查；额外技能按技能名兜底。
//
// 用法：node apps/scripts/gen-sgs-skill-text.mjs [--check]
//   --check：与现有生成物比较，不一致则退出码 1（供 CI/门禁用）。

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const src = resolve(repo, "docs/games/sanguosha/data/heroes.json");
const out = resolve(repo, "apps/web/src/view/sgs/skill-text.ts");

const raw = JSON.parse(readFileSync(src, "utf8"));
const allHeroes = Array.isArray(raw) ? raw : raw.heroes ?? [];

// 只保留引擎已实装武将（engine-catalog.json 的 GeneralDef），避免把 695 位全量文本打进前端 bundle。
const catalog = JSON.parse(
  readFileSync(resolve(repo, "docs/games/sanguosha/data/engine-catalog.json"), "utf8"),
);
const implemented = new Set(catalog.generals.map((g) => g.name));
const heroes = allHeroes.filter((h) => h?.name && implemented.has(h.name));

// 授予技（觉醒/破势等）不在 heroes.json 里，需在此补官方文本；键为技能名。
const GRANTED_SKILL_TEXT = {
  怀柔: "出牌阶段，你可以重铸一张装备牌（弃置并摸一张牌）。",
  清侧: "出牌阶段限一次，你可以弃置一名角色一张牌，然后若你与其的距离大于1，你摸一张牌。",
};

const generals = {};
const byName = { ...GRANTED_SKILL_TEXT };
for (const h of heroes) {
  if (!h?.name) continue;
  const skills = {};
  for (const s of h.skills ?? []) {
    if (!s?.name || !s?.description) continue;
    const desc = String(s.description).replace(/\r\n?/g, "\n").trim();
    if (!desc) continue;
    skills[s.name] = desc;
    if (!byName[s.name]) byName[s.name] = desc;
  }
  if (Object.keys(skills).length) generals[h.name] = skills;
}

const banner = `// 本文件由 apps/scripts/gen-sgs-skill-text.mjs 生成，请勿手改。\n` +
  `// 数据源：docs/games/sanguosha/data/heroes.json（${heroes.length} 位武将）。\n`;
const body = `${banner}\n` +
  `export const SKILL_TEXT_BY_GENERAL: Record<string, Record<string, string>> = ${JSON.stringify(generals)};\n\n` +
  `export const SKILL_TEXT_BY_NAME: Record<string, string> = ${JSON.stringify(byName)};\n\n` +
  `/** 按武将名 + 技能名取简述；找不到时按技能名兜底。 */\n` +
  `export function skillDescription(generalName: string | null | undefined, skillName: string): string {\n` +
  `  const g = generalName ? SKILL_TEXT_BY_GENERAL[generalName] : undefined;\n` +
  `  return g?.[skillName] ?? SKILL_TEXT_BY_NAME[skillName] ?? "";\n` +
  `}\n`;

const check = process.argv.includes("--check");
if (check) {
  const current = existsSync(out) ? readFileSync(out, "utf8") : "";
  if (current !== body) {
    console.error(`[gen-sgs-skill-text] 生成物已过期：${out}`);
    process.exit(1);
  }
  console.log("[gen-sgs-skill-text] OK");
} else {
  writeFileSync(out, body);
  console.log(`[gen-sgs-skill-text] 已写入 ${out}（${Object.keys(generals).length} 位武将 / ${Object.keys(byName).length} 个技能名）`);
}
