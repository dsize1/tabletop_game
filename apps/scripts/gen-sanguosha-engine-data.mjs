#!/usr/bin/env node
// 由 docs/games/sanguosha/data/{cards,heroes}.json 生成引擎内置数据表
// （crates/engines/sanguosha/src/data.rs）。
//
// 用法：
//   node apps/scripts/gen-sanguosha-engine-data.mjs
//   node apps/scripts/gen-sanguosha-engine-data.mjs --check   # 只校验仓库文件是否最新
//
// 口径：
// - 牌：只取 cards.json 的 `deck === "standard"`（108 张），定义码用下方 DEF_CODE 映射；
// - 武将：只取 heroes.json 中标准包 25 将（gid 1..=25），code 与中台导入一致（`sgs_hero_<gid>`）；
// - 「主公将」由技能 `type_guess === "lord"` 推出（官网无该字段）。
// - 输出确定性：同样的输入必然产出逐字节相同的文件。

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const DEFAULT_CARDS = resolve("docs", "games", "sanguosha", "data", "cards.json");
const DEFAULT_HEROES = resolve("docs", "games", "sanguosha", "data", "heroes.json");
const DEFAULT_OUT = resolve("crates", "engines", "sanguosha", "src", "data.rs");

const STANDARD_GIDS = Array.from({ length: 25 }, (_, i) => i + 1);
// 标准包女性武将（官网无性别字段；按官方设定维护）
const FEMALE_GIDS = new Set([7, 13, 21, 24, 25]);

const DEF_CODE = {
  杀: "sha",
  闪: "shan",
  桃: "tao",
  决斗: "juedou",
  过河拆桥: "guohe_chaiqiao",
  顺手牵羊: "shunshou_qianyang",
  无中生有: "wuzhong_shengyou",
  借刀杀人: "jiedao_sharen",
  无懈可击: "wuxie_keji",
  南蛮入侵: "nanman_ruqin",
  万箭齐发: "wanjian_qifa",
  桃园结义: "taoyuan_jieyi",
  五谷丰登: "wugu_fengdeng",
  乐不思蜀: "lebu_sishu",
  闪电: "shandian",
  诸葛连弩: "zhuge_liannu",
  雌雄双股剑: "cixiong_shuanggujian",
  青釭剑: "qinggang_jian",
  青龙偃月刀: "qinglong_yanyue",
  丈八蛇矛: "zhangba_shemao",
  贯石斧: "guanshi_fu",
  方天画戟: "fangtian_huaji",
  麒麟弓: "qilin_gong",
  寒冰剑: "hanbing_jian",
  八卦阵: "bagua_zhen",
  仁王盾: "renwang_dun",
  赤兔: "chitu",
  大宛: "dawan",
  紫骍: "zixing",
  的卢: "dilu",
  绝影: "jueying",
  爪黄飞电: "zhuahuang_feidian",
};

const CATEGORY = {
  basic: "Basic",
  trick: "Trick",
  delayed_trick: "DelayedTrick",
  weapon: "Weapon",
  armor: "Armor",
  horse: "Horse",
};
const SLOT = {
  weapon: "Weapon",
  armor: "Armor",
  horse_offense: "HorseOffense",
  horse_defense: "HorseDefense",
  treasure: "Treasure",
};
const RANGE_TYPE = { attack: "Attack", defense: "Defense" };
const TARGET_MODE = { single: "Single", aoe: "Aoe" };
const SUIT = { "♠": "Spade", "♥": "Heart", "♣": "Club", "♦": "Diamond" };
const RANK = { A: 1, J: 11, Q: 12, K: 13 };

function parseArgs(argv) {
  const opts = { cards: DEFAULT_CARDS, heroes: DEFAULT_HEROES, out: DEFAULT_OUT, check: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--cards") opts.cards = resolve(argv[++i]);
    else if (argv[i] === "--heroes") opts.heroes = resolve(argv[++i]);
    else if (argv[i] === "--out") opts.out = resolve(argv[++i]);
    else if (argv[i] === "--check") opts.check = true;
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log(
        "用法：node apps/scripts/gen-sanguosha-engine-data.mjs [--cards cards.json] [--heroes heroes.json] [--out data.rs] [--check]",
      );
      process.exit(0);
    } else {
      console.error(`未知参数：${argv[i]}`);
      process.exit(1);
    }
  }
  return opts;
}

function rankNum(rank) {
  if (RANK[rank]) return RANK[rank];
  const n = Number(rank);
  if (!Number.isInteger(n) || n < 1 || n > 13) throw new Error(`非法点数：${rank}`);
  return n;
}

function rustStr(s) {
  return JSON.stringify(String(s));
}

function rustOpt(v, map, enumName) {
  if (v === null || v === undefined) return "None";
  const variant = map[v];
  if (!variant) throw new Error(`未知枚举值：${v}`);
  return `Some(${enumName}::${variant})`;
}

function buildCards(payload) {
  const all = Array.isArray(payload.cards) ? payload.cards : [];
  const standard = all.filter((c) => c.deck === "standard");
  if (standard.length !== 108) throw new Error(`标准牌堆应为 108 张，实际 ${standard.length}`);

  const defs = new Map();
  const physical = [];
  for (const c of standard) {
    const code = DEF_CODE[c.name];
    if (!code) throw new Error(`缺少定义码映射：${c.name}`);
    const suit = SUIT[c.suit];
    if (!suit) throw new Error(`非法花色：${c.suit}（${c.name}）`);
    const def = {
      code,
      name: c.name,
      category: CATEGORY[c.category],
      equip_slot: c.equip_slot ?? null,
      range_value: c.range_value ?? null,
      range_type: c.range_type ?? null,
      target_mode: c.target_mode ?? null,
      delayed: Boolean(c.delayed),
    };
    for (const [k, v] of Object.entries(def)) {
      if (v === undefined) throw new Error(`字段缺失 ${c.name}.${k}`);
    }
    const prev = defs.get(code);
    if (prev) {
      const same = ["category", "equip_slot", "range_value", "range_type", "target_mode", "delayed"].every(
        (k) => JSON.stringify(prev[k]) === JSON.stringify(def[k]),
      );
      if (!same) throw new Error(`同名定义字段不一致：${c.name}`);
    } else {
      defs.set(code, def);
    }
    physical.push({ def: code, suit, rank: rankNum(c.rank) });
  }

  const defRows = [...defs.values()].map((d) =>
    [
      "    CardDef {",
      ` code: ${rustStr(d.code)}, name: ${rustStr(d.name)},`,
      ` category: Category::${d.category},`,
      ` equip_slot: ${rustOpt(d.equip_slot, SLOT, "EquipSlot")},`,
      ` range_value: ${d.range_value === null ? "None" : `Some(${d.range_value})`},`,
      ` range_type: ${rustOpt(d.range_type, RANGE_TYPE, "RangeType")},`,
      ` target_mode: ${rustOpt(d.target_mode, TARGET_MODE, "TargetMode")},`,
      ` delayed: ${d.delayed},`,
      "},",
    ].join(""),
  );
  const cardRows = physical.map(
    (c) => `    PhysicalCard { def: ${rustStr(c.def)}, suit: Suit::${c.suit}, rank: ${c.rank} },`,
  );
  return { defs, physical, defRows, cardRows };
}

function buildGenerals(payload) {
  const all = Array.isArray(payload.heroes) ? payload.heroes : [];
  const picked = [];
  for (const gid of STANDARD_GIDS) {
    const h = all.find((x) => x.gid === gid && x.pack === "标准");
    if (!h) throw new Error(`heroes.json 缺少标准包 gid=${gid}`);
    const skills = Array.isArray(h.skills) ? h.skills : [];
    const isLord = skills.some((s) => s.type_guess === "lord");
    picked.push({
      code: `sgs_hero_${gid}`,
      gid,
      name: h.name,
      kingdom: h.kingdom_code,
      max_hp: h.hp,
      gender: FEMALE_GIDS.has(gid) ? 'female' : 'male',
      is_lord: isLord,
      skills: skills.map((s) => s.name),
    });
  }
  if (picked.length !== 25) throw new Error(`标准将应为 25 位，实际 ${picked.length}`);
  const rows = picked.map((g) =>
    [
      "    GeneralDef {",
      ` code: ${rustStr(g.code)}, gid: ${g.gid}, name: ${rustStr(g.name)},`,
      ` kingdom: ${rustStr(g.kingdom)}, max_hp: ${g.max_hp}, gender: ${rustStr(g.gender)}, is_lord: ${g.is_lord},`,
      ` skills: &[${g.skills.map(rustStr).join(", ")}],`,
      "},",
    ].join(""),
  );
  return { picked, rows };
}

function render({ defs, physical, defRows, cardRows, generals, generalRows }) {
  const lines = [];
  lines.push("//! 三国杀引擎内置数据（**自动生成，勿手改**）。");
  lines.push("//!");
  lines.push("//! 生成：`node apps/scripts/gen-sanguosha-engine-data.mjs`");
  lines.push("//! 校验：`node apps/scripts/gen-sanguosha-engine-data.mjs --check`");
  lines.push("//! 来源：`docs/games/sanguosha/data/cards.json`（standard）+ `heroes.json`（标准包 25 将）。");
  lines.push("");
  lines.push("use crate::state::{Category, EquipSlot, RangeType, Suit, TargetMode};");
  lines.push("");
  lines.push(`pub const STANDARD_DECK_SIZE: usize = ${physical.length};`);
  lines.push(`pub const STANDARD_GENERAL_COUNT: usize = ${generals.length};`);
  lines.push("");
  lines.push("/// 牌的定义（同一 `code` 只有一条；物理牌见 `STANDARD_DECK`）。");
  lines.push("#[derive(Debug, Clone, Copy)]");
  lines.push("pub struct CardDef {");
  lines.push("    pub code: &'static str,");
  lines.push("    pub name: &'static str,");
  lines.push("    pub category: Category,");
  lines.push("    pub equip_slot: Option<EquipSlot>,");
  lines.push("    /// 武器攻击距离 / 坐骑距离；非装备为 `None`。");
  lines.push("    pub range_value: Option<i8>,");
  lines.push("    /// 坐骑方向：进攻马 / 防御马。");
  lines.push("    pub range_type: Option<RangeType>,");
  lines.push("    /// 目标模式：单目标 / AOE。");
  lines.push("    pub target_mode: Option<TargetMode>,");
  lines.push("    pub delayed: bool,");
  lines.push("}");
  lines.push("");
  lines.push("#[rustfmt::skip]");
  lines.push(`pub static CARD_DEFS: &[CardDef] = &[`);
  lines.push(...defRows);
  lines.push("];");
  lines.push("");
  lines.push("/// 牌堆中的每一张物理牌（花色 + 点数）。");
  lines.push("#[derive(Debug, Clone, Copy)]");
  lines.push("pub struct PhysicalCard {");
  lines.push("    pub def: &'static str,");
  lines.push("    pub suit: Suit,");
  lines.push("    pub rank: u8,");
  lines.push("}");
  lines.push("");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static STANDARD_DECK: &[PhysicalCard] = &[");
  lines.push(...cardRows);
  lines.push("];");
  lines.push("");
  lines.push("/// 标准包武将（`code` 与中台 `sgs_generals.code` 一致）。");
  lines.push("#[derive(Debug, Clone, Copy)]");
  lines.push("pub struct GeneralDef {");
  lines.push("    pub code: &'static str,");
  lines.push("    pub gid: u32,");
  lines.push("    pub name: &'static str,");
  lines.push("    pub kingdom: &'static str,");
  lines.push("    pub max_hp: i32,");
  lines.push("    /// 性别（male / female；用于离间、雌雄双股剑等）。");
  lines.push("    pub gender: &'static str,");
  lines.push("    /// 是否主公将（技能含主公技）。");
  lines.push("    pub is_lord: bool,");
  lines.push("    /// 技能名（P2.1 仅展示用；技能实现见 P2.4）。");
  lines.push("    pub skills: &'static [&'static str],");
  lines.push("}");
  lines.push("");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static STANDARD_GENERALS: &[GeneralDef] = &[");
  lines.push(...generalRows);
  lines.push("];");
  lines.push("");
  lines.push("pub fn card_def(code: &str) -> Option<&'static CardDef> {");
  lines.push("    CARD_DEFS.iter().find(|d| d.code == code)");
  lines.push("}");
  lines.push("");
  lines.push("pub fn general(code: &str) -> Option<&'static GeneralDef> {");
  lines.push("    STANDARD_GENERALS.iter().find(|g| g.code == code)");
  lines.push("}");
  lines.push("");
  lines.push("/// 全部武将码（按 gid 升序），用于选将池。");
  lines.push("pub fn all_general_codes() -> Vec<&'static str> {");
  lines.push("    STANDARD_GENERALS.iter().map(|g| g.code).collect()");
  lines.push("}");
  lines.push("");
  return lines.join("\n");
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const cards = JSON.parse(await readFile(opts.cards, "utf8"));
  const heroes = JSON.parse(await readFile(opts.heroes, "utf8"));
  const built = buildCards(cards);
  const gens = buildGenerals(heroes);
  const text = render({
    ...built,
    generals: gens.picked,
    generalRows: gens.rows,
  });

  if (opts.check) {
    let current = "";
    try {
      current = await readFile(opts.out, "utf8");
    } catch {
      console.error(`[check] 缺少生成文件：${opts.out}`);
      process.exit(1);
    }
    if (current !== text) {
      console.error(`[check] 生成文件过期：${opts.out}（请重新运行生成脚本）`);
      process.exit(1);
    }
    console.log(`[check] OK：${built.physical.length} 张牌 / ${gens.picked.length} 位武将，${opts.out} 已是最新`);
    return;
  }

  await mkdir(dirname(opts.out), { recursive: true });
  await writeFile(opts.out, text, "utf8");
  console.log(
    `[gen] ${built.defs.size} 个牌定义 / ${built.physical.length} 张物理牌 / ${gens.picked.length} 位标准将 → ${opts.out}`,
  );
}

main().catch((err) => {
  console.error("[gen] 失败:", err);
  process.exit(1);
});
