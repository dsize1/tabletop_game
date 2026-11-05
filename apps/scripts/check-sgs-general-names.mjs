#!/usr/bin/env node
// 武将名一致性/错别字门禁（编辑距离 + 官方名单比对）。
//
// 背景：官网抓取数据偶有错别字（如 gid=55「张昭张弘」应为「张昭张纮」），
// 而 `prefer_jiexian`（界优先去重）按**名字**比对 → 错别字会让原版与界版无法配对，
// 原版错误进池、并作为独立武将进入验证批次（真机发现）。
//
// 检查：
// 1) 232 将名称必须在官方名录（`heroes.json`）中，或在勘误表（`lib/sgs-name-fixes.mjs`）里；
// 2) 每个「界X」应能在官方名录找到 X；找不到时，若存在**同长度、编辑距离 1** 的官方名 → 判为错别字嫌疑（失败）；
// 3) 232 内部近似名对（长度≥3、距离 ≤1）必须在白名单（合法近似：夏侯/诸葛/蔡/皇后等）。
//
// 用法：node apps/scripts/check-sgs-general-names.mjs [--verbose]

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fixedGeneralName, GENERAL_NAME_FIX_NOTES } from "./lib/sgs-name-fixes.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "../..");
const verbose = process.argv.includes("--verbose");

const heroes = JSON.parse(
  readFileSync(resolve(root, "docs/games/sanguosha/data/heroes.json"), "utf8"),
).heroes;
const dataRs = readFileSync(resolve(root, "crates/engines/sanguosha/src/data.rs"), "utf8");
const gens = [...dataRs.matchAll(
  /GeneralDef \{ code: "(sgs_hero_\d+)", gid: (\d+), name: "([^"]+)", pack: "([^"]+)", kingdom: "([^"]+)", max_hp: (\d+), gender: "[^"]+", is_lord: (?:true|false), skills: &\[([^\]]*)\],\}/g,
)].map((m) => ({
  code: m[1],
  gid: Number(m[2]),
  name: m[3],
  pack: m[4],
  kingdom: m[5],
  maxHp: Number(m[6]),
  skills: m[7]
    .split(",")
    .map((x) => x.trim().replace(/^"|"$/g, ""))
    .filter(Boolean),
}));

const official = [...new Set(heroes.map((h) => fixedGeneralName(h)))];
const officialSet = new Set(official);
const officialByGid = new Map(heroes.map((h) => [h.gid, h]));
const officialSkillNames = new Set(
  heroes.flatMap((h) => (Array.isArray(h.skills) ? h.skills.map((s) => s.name) : [])),
);

/** 合法近似名对白名单（复核过：均为不同武将；key 用字典序）。 */
const pairKey = (a, b) => [a, b].sort().join("|");
const NEAR_DUP_ALLOW = new Set(
  [
    ["夏侯惇", "夏侯渊"],
    ["夏侯惇", "夏侯氏"],
    ["夏侯渊", "夏侯氏"],
    ["诸葛亮", "卧龙诸葛亮"],
    ["诸葛亮", "诸葛瞻"],
    ["卧龙诸葛亮", "诸葛瞻"],
    ["伏皇后", "郭皇后"],
    ["蔡文姬", "蔡夫人"],
    ["孙尚香", "孙鲁班"],
    ["公孙渊", "公孙瓒"],
    ["辛宪英", "黄月英"],
  ].map(([a, b]) => pairKey(a, b)),
);
/** 官方名录里没有对应基础版的界将（合法：基础版不在已收录包）。 */
const JIE_NO_BASE_ALLOW = new Set(["李典"]);

function levenshtein(a, b) {
  const dp = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 0; j <= b.length; j += 1) dp[0][j] = j;
  for (let i = 1; i <= a.length; i += 1) {
    for (let j = 1; j <= b.length; j += 1) {
      dp[i][j] = Math.min(
        dp[i - 1][j] + 1,
        dp[i][j - 1] + 1,
        dp[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1),
      );
    }
  }
  return dp[a.length][b.length];
}

const problems = [];
const notes = [];

// 1) 名称必须在官方名录/勘误表
for (const g of gens) {
  if (!officialSet.has(g.name)) problems.push(`${g.code} ${g.name} 不在官方名录（heroes.json）`);
}

// 1b) 逐 gid 与官方数据对照（名称/技能名/体力）：能发现「名字看起来合法但不是该 gid」的错误
let gidMismatch = 0;
for (const g of gens) {
  const h = officialByGid.get(g.gid);
  if (!h) {
    gidMismatch += 1;
    problems.push(`${g.code} gid=${g.gid} 在官方数据中不存在`);
    continue;
  }
  const want = fixedGeneralName(h);
  if (want !== g.name) {
    gidMismatch += 1;
    problems.push(`gid=${g.gid} 名称不一致：data.rs「${g.name}」≠ 官方「${want}」`);
  }
  const os = (Array.isArray(h.skills) ? h.skills : []).map((x) => x.name);
  const missing = os.filter((n) => !g.skills.includes(n));
  const extra = g.skills.filter((n) => !os.includes(n));
  if (missing.length || extra.length) {
    gidMismatch += 1;
    problems.push(
      `gid=${g.gid}「${g.name}」技能名与官方不一致：缺[${missing.join("、")}] 多[${extra.join("、")}]`,
    );
  }
  if (Number(h.hp) !== g.maxHp) {
    gidMismatch += 1;
    problems.push(`gid=${g.gid}「${g.name}」体力不一致：${g.maxHp} ≠ 官方 ${h.hp}`);
  }
  for (const sk of g.skills) {
    if (!officialSkillNames.has(sk)) {
      gidMismatch += 1;
      problems.push(`gid=${g.gid}「${g.name}」技能名「${sk}」不在官方技能名集合（疑似错别字）`);
    }
  }
}

// 2) 界版 base 检查
const jie = gens.filter((g) => g.pack === "jiexian");
for (const g of jie) {
  const base = g.name.replace(/^界/, "");
  if (officialSet.has(base) || JIE_NO_BASE_ALLOW.has(base)) continue;
  const sameLen = official.filter((n) => n.length === base.length);
  const near = sameLen
    .map((n) => ({ n, d: levenshtein(n, base) }))
    .filter((x) => x.d === 1)
    .map((x) => x.n);
  problems.push(
    near.length
      ? `${g.name} 的基础版「${base}」不在官方名录；等长距离 1 候选：${near.join("、")}（疑似错别字，请在 lib/sgs-name-fixes.mjs 修正）`
      : `${g.name} 的基础版「${base}」不在官方名录（若确不存在，加入 JIE_NO_BASE_ALLOW）`,
  );
}

// 3) 232 内部近似名对
const pairs = [];
for (let i = 0; i < gens.length; i += 1) {
  for (let j = i + 1; j < gens.length; j += 1) {
    const a = gens[i];
    const b = gens[j];
    const an = a.name.replace(/^界/, "");
    const bn = b.name.replace(/^界/, "");
    if (an === bn) continue;
    if (an.length < 3 || bn.length < 3) continue;
    if (levenshtein(an, bn) !== 1) continue;
    const key = pairKey(an, bn);
    if (pairs.some((p) => p.key === key)) continue;
    pairs.push({ key, a: an, b: bn });
  }
}
for (const p of pairs) {
  if (NEAR_DUP_ALLOW.has(p.key)) continue;
  problems.push(`近似名对未登记白名单：${p.a} ↔ ${p.b}（若为不同武将，加入 NEAR_DUP_ALLOW 并注明理由）`);
}

if (verbose) {
  notes.push(`232 将；界版 ${jie.length}；官方名录 ${officialSet.size} 个名字`);
  for (const f of GENERAL_NAME_FIX_NOTES) notes.push(`勘误：gid=${f.gid} ${f.from} → ${f.to}（${f.reason}）`);
  for (const p of pairs) notes.push(`近似名对（已允许/已登记）：${p.a} ↔ ${p.b}`);
}

if (problems.length) {
  for (const p of problems) console.error(`[check:sgs-general-names] ❌ ${p}`);
  process.exit(1);
}
for (const n of notes) console.log(`[check:sgs-general-names] ${n}`);
console.log(
  `[check:sgs-general-names] OK：${gens.length} 将逐 gid 与官方一致（名称/技能名/体力；界版 ${jie.length}，近似名对 ${pairs.length} 均在白名单）`,
);
