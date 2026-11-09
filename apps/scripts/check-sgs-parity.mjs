#!/usr/bin/env node
// M4-2 对拍：v1 ↔ v2 的**数据/属性/技能覆盖**一致性检查（只对白名单外差异报警）。
//
// 为什么只对拍「数据 + 属性」而不是重放 v1 房间：
//   - v2 尚未注册，动作协议/座位编号/身份局参数都不同，逐动作重放不可比；
//   - 真正可比的是**同一份官方数据生成的两套表**（160 张牌、装备范围/坐骑、类别/延时/目标模式）
//     和「v2 已实现的技能必须是 v1 已实现的子集」。
//   逐动作语义已在 M1/M2 的 75+12 条用例 + 场景门禁覆盖（见 tests/cards.rs、tests/m2_wiring.rs）。
//
// 白名单：docs/games/sanguosha/engine-replacement/parity-whitelist.json
// 用法：node apps/scripts/check-sgs-parity.mjs [--verbose]
// 退出码：0 通过（允许白名单差异）；1 非白名单差异 / 白名单过期。

import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const verbose = process.argv.includes("--verbose");
const V1_DATA = path.join(projectRoot, "crates/engines/sanguosha/src/data.rs");
const V2_CARDS = path.join(projectRoot, "crates/engines/sanguosha2/src/cards_generated.rs");
const V2_SKILLS_DIR = path.join(projectRoot, "crates/engines/sanguosha2/src/skills");
const V1_SKILLS = path.join(projectRoot, "crates/engines/sanguosha/src/skills.rs");
const WHITELIST = path.join(
  projectRoot,
  "docs/games/sanguosha/engine-replacement/parity-whitelist.json",
);

if (!existsSync(V1_DATA)) {
  console.log("[parity] 跳过：v1 crate 已删除（对拍只在迁移期有意义）。");
  process.exit(0);
}

const wl = JSON.parse(readFileSync(WHITELIST, "utf8"));
const problems = [];
const notes = [];
const fail = (m) => problems.push(m);

// ---------------------------------------------------------------------------
// 解析 v1 data.rs
// ---------------------------------------------------------------------------

const v1Text = readFileSync(V1_DATA, "utf8");
const v1Defs = new Map();
for (const m of v1Text.matchAll(
  /CardDef \{ code: "([^"]+)", name: "([^"]+)", category: Category::(\w+), equip_slot: (None|Some\(EquipSlot::(\w+)\)), range_value: (None|Some\((-?\d+)\)), range_type: (None|Some\(RangeType::(\w+)\)), target_mode: (None|Some\(TargetMode::(\w+)\)), delayed: (true|false),\}/g,
)) {
  v1Defs.set(m[2], {
    code: m[1],
    name: m[2],
    category: m[3],
    slot: m[5] ?? null,
    range: m[7] === undefined ? null : Number(m[7]),
    rangeType: m[9] ?? null,
    targetMode: m[11] ?? null,
    delayed: m[12] === "true",
  });
}

const v1Decks = [];
for (const block of v1Text.matchAll(
  /pub static (STANDARD_DECK|JUNZHENG_DECK): &\[PhysicalCard\] = &\[([\s\S]*?)\n\];/g,
)) {
  const pack = block[1] === "STANDARD_DECK" ? "standard" : "junzheng";
  for (const c of block[2].matchAll(
    /PhysicalCard \{ pack: "[^"]+", def: "([^"]+)", suit: Suit::(\w+), rank: (\d+) \}/g,
  )) {
    v1Decks.push({ pack, def: c[1], suit: c[2], rank: Number(c[3]) });
  }
}

// ---------------------------------------------------------------------------
// 解析 v2 cards_generated.rs
// ---------------------------------------------------------------------------

const v2Text = readFileSync(V2_CARDS, "utf8");
const v2Defs = new Map();
for (const m of v2Text.matchAll(
  /Row \{ name: "([^"]+)", kind: CardKind::(\w+), slot: (None|Some\(Slot::(\w+)\)), weapon_range: (None|Some\((\d+)\)), horse: (None|Some\((\d+)\)), damage_kind: DamageKind::\w+, damage_trick: (?:true|false), effect: Effect::\w+, armor: ArmorKind::\w+, weapon: WeaponKind::\w+, target_mode: TargetMode::(\w+), respond: [^,]+, sha: [^,]+, shan: [^,]+, tao: [^,]+, jiu: [^,]+, wuxie: [^,]+, duel: [^,]+, delayed: (true|false), lianhuan: [^}]+ \}/g,
)) {
  v2Defs.set(m[1], {
    name: m[1],
    kind: m[2],
    slot: m[4] ?? null,
    weaponRange: m[6] === undefined ? null : Number(m[6]),
    horse: m[8] === undefined ? null : Number(m[8]),
    targetMode: m[9],
    delayed: m[10] === "true",
  });
}
const v2Deck = [...v2Text.matchAll(/Phy \{ name: "([^"]+)", suit: Suit::(\w+), rank: (\d+) \}/g)].map(
  (m) => ({ name: m[1], suit: m[2], rank: Number(m[3]) }),
);

// ---------------------------------------------------------------------------
// 1) 物理牌堆（160 张：名称/花色/点数/顺序）
// ---------------------------------------------------------------------------

const v1DefByCode = new Map([...v1Defs.values()].map((d) => [d.code, d]));
const v1DeckAsNames = v1Decks.map((c) => ({
  name: v1DefByCode.get(c.def)?.name ?? c.def,
  suit: c.suit,
  rank: c.rank,
}));
if (v1DeckAsNames.length !== 160) fail(`v1 牌堆应为 160 张，实际 ${v1DeckAsNames.length}`);
if (v2Deck.length !== 160) fail(`v2 牌堆应为 160 张，实际 ${v2Deck.length}`);
const deckDiffs = [];
const nDeck = Math.max(v1DeckAsNames.length, v2Deck.length);
for (let i = 0; i < nDeck; i += 1) {
  const a = v1DeckAsNames[i];
  const b = v2Deck[i];
  if (!a || !b) {
    deckDiffs.push(`#${i}: v1=${JSON.stringify(a)} v2=${JSON.stringify(b)}`);
    continue;
  }
  if (a.name !== b.name || a.suit !== b.suit || a.rank !== b.rank) {
    deckDiffs.push(
      `#${i}: v1=${a.name}/${a.suit}/${a.rank} v2=${b.name}/${b.suit}/${b.rank}`,
    );
  }
}
for (const d of deckDiffs.slice(0, 10)) fail(`牌堆不一致 ${d}`);
if (deckDiffs.length > 10) fail(`牌堆不一致共 ${deckDiffs.length} 处（只列前 10）`);
if (deckDiffs.length === 0) notes.push("牌堆 160 张（名称/花色/点数/顺序）一致");

// ---------------------------------------------------------------------------
// 2) 牌定义（类别/槽位/范围/延时/目标模式）
// ---------------------------------------------------------------------------

const CATEGORY_EXPECT = {
  Basic: "Basic",
  Trick: "Trick",
  DelayedTrick: "Trick",
  Weapon: "Equip",
  Armor: "Equip",
  Horse: "Equip",
};
const targetExpected = (v1mode) =>
  v1mode === "Aoe" ? "AllAlive" : v1mode === "Single" ? "Chosen" : "SelfOnly";

const defDiffs = [];
for (const [name, v1] of v1Defs) {
  const v2 = v2Defs.get(name);
  if (!v2) {
    defDiffs.push(`${name}: v2 缺少定义`);
    continue;
  }
  if (CATEGORY_EXPECT[v1.category] !== v2.kind) {
    defDiffs.push(`${name}: 类别 v1=${v1.category} v2=${v2.kind}`);
  }
  if ((v1.slot ?? null) !== (v2.slot ?? null)) {
    defDiffs.push(`${name}: 槽位 v1=${v1.slot} v2=${v2.slot}`);
  }
  const isWeapon = v1.slot === "Weapon";
  if (isWeapon && (v1.range ?? null) !== (v2.weaponRange ?? null)) {
    defDiffs.push(`${name}: 武器范围 v1=${v1.range} v2=${v2.weaponRange}`);
  }
  const isHorse = v1.slot === "HorseOffense" || v1.slot === "HorseDefense";
  if (isHorse && (v1.range ?? null) !== (v2.horse ?? null)) {
    defDiffs.push(`${name}: 坐骑修正 v1=${v1.range} v2=${v2.horse}`);
  }
  if (v1.delayed !== v2.delayed) {
    defDiffs.push(`${name}: 延时 v1=${v1.delayed} v2=${v2.delayed}`);
  }
  const want = targetExpected(v1.targetMode);
  if (want !== v2.targetMode && !wl.target_mode?.[name]) {
    defDiffs.push(`${name}: 目标模式 v1=${v1.targetMode}→预期 ${want} v2=${v2.targetMode}`);
  } else if (want !== v2.targetMode) {
    if (verbose) notes.push(`${name}: 目标模式白名单（v1=${v1.targetMode} → v2=${v2.targetMode}）`);
  }
}
for (const [name] of v2Defs) {
  if (!v1Defs.has(name) && !wl.v2_extra_defs?.[name]) {
    defDiffs.push(`${name}: v2 额外定义（不在白名单）`);
  }
}
for (const d of defDiffs) fail(`牌定义差异 ${d}`);
if (defDiffs.length === 0) notes.push(`牌定义 ${v1Defs.size} 条一致（白名单 ${Object.keys(wl.target_mode ?? {}).length} 项目标模式差异）`);

// ---------------------------------------------------------------------------
// 3) 技能覆盖（v2 已实现 ⊆ v1 已实现）
// ---------------------------------------------------------------------------

const v1Skills = readFileSync(V1_SKILLS, "utf8");
const v1Implemented = new Set();
for (const chunk of v1Skills.split("SkillDef {").slice(1)) {
  const code = /code:\s*"([a-z0-9_]+)"/.exec(chunk)?.[1];
  const implemented = /implemented:\s*true/.test(chunk);
  if (code && implemented) v1Implemented.add(code);
}
const v2Codes = new Set();
for (const file of readdirSync(V2_SKILLS_DIR)) {
  if (!file.endsWith(".rs")) continue;
  const text = readFileSync(path.join(V2_SKILLS_DIR, file), "utf8");
  for (const m of text.matchAll(/code:\s*"([a-z0-9_]+)"/g)) v2Codes.add(m[1]);
}
const v2Only = [...v2Codes].filter((c) => !v1Implemented.has(c));
for (const c of v2Only) fail(`技能 ${c} 在 v2 已实现，但 v1 未标记 implemented（对拍不应出现新技能）`);
notes.push(`技能：v2 注册 ${v2Codes.size} 个 / v1 implemented ${v1Implemented.size} 个（v2 子集${v2Only.length ? "：✗" : "：✓"}）`);

// ---------------------------------------------------------------------------
// 4) 白名单有效性（防止白名单过期/腐烂）
// ---------------------------------------------------------------------------

for (const entry of wl.v1_known_defects ?? []) {
  const file = path.join(projectRoot, entry.file);
  if (!existsSync(file)) {
    fail(`白名单 ${entry.id}: 文件不存在 ${entry.file}（过期，请在删除 v1 时归档）`);
    continue;
  }
  const text = readFileSync(file, "utf8");
  for (const needle of entry.needles ?? []) {
    if (!text.includes(needle)) {
      fail(`白名单 ${entry.id}: 未找到片段 ${JSON.stringify(needle)}（v1 可能已修复 → 请更新白名单）`);
    }
  }
}

// ---------------------------------------------------------------------------
// 汇总
// ---------------------------------------------------------------------------

console.log(
  `[parity] v1 定义 ${v1Defs.size} / v2 定义 ${v2Defs.size}；v1 牌堆 ${v1DeckAsNames.length} / v2 牌堆 ${v2Deck.length}`,
);
for (const n of notes) console.log(`  · ${n}`);
if (problems.length) {
  console.error(`[parity] 未通过（${problems.length}）：`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[parity] OK：白名单外无差异");
