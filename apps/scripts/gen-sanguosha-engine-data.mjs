#!/usr/bin/env node
// 由 docs/games/sanguosha/data/{cards,heroes}.json 生成引擎内置数据表
// （crates/engines/sanguosha/src/data.rs）。
//
// 用法：
//   node apps/scripts/gen-sanguosha-engine-data.mjs
//   node apps/scripts/gen-sanguosha-engine-data.mjs --check             # 只校验仓库文件是否最新
//   node apps/scripts/gen-sanguosha-engine-data.mjs --general-packs standard,wind
//
// 口径（P4.0）：
// - 牌：cards.json 的 `deck === "standard"`（108 张）+ `deck === "junzheng"`（52 张），
//   定义码用下方 DEF_CODE 映射；物理牌带 `pack` 字段，引擎按 `Config.packs` 组堆；
// - 武将：heroes.json 按 `pack` 分组生成；默认生成标准包 25 将 + 风林火山 48 将（gid 表见 GENERAL_PACKS），
//   `GeneralDef.pack` 为武将包 code；新增包需同时在 GENERAL_PACKS 登记（含性别表）；
// - 「主公将」由技能 `type_guess === "lord"` 推出（官网无该字段）。
// - 输出确定性：同样的输入必然产出逐字节相同的文件。

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const DEFAULT_CARDS = resolve("docs", "games", "sanguosha", "data", "cards.json");
const DEFAULT_HEROES = resolve("docs", "games", "sanguosha", "data", "heroes.json");
const DEFAULT_OUT = resolve("crates", "engines", "sanguosha", "src", "data.rs");

/// 牌堆包（顺序即建堆顺序：同种子下 standard-only 对局的牌实例 id 保持 c1..c108）。
const DECKS = [
  { key: "standard", label: "标准", size: 108, staticName: "STANDARD_DECK", sizeConst: "STANDARD_DECK_SIZE" },
  { key: "junzheng", label: "军争", size: 52, staticName: "JUNZHENG_DECK", sizeConst: "JUNZHENG_DECK_SIZE" },
];

/// 武将包登记表。`gids` 为 null 时取该包全部武将；`femaleGids` 为 null 表示缺性别表（拒绝生成）。
/// P4.4 扩包时在此追加（官网数据无性别字段，需按官方设定维护 femaleGids）。
const GENERAL_PACKS = [
  {
    key: "standard",
    label: "标准",
    staticName: "STANDARD_GENERALS",
    countConst: "STANDARD_GENERAL_COUNT",
    gids: Array.from({ length: 25 }, (_, i) => i + 1),
    femaleGids: new Set([7, 13, 21, 24, 25]),
  },
  {
    key: "wind_fire_forest_mountain",
    label: "风林火山",
    staticName: "WIND_FIRE_FOREST_MOUNTAIN_GENERALS",
    countConst: "WIND_FIRE_FOREST_MOUNTAIN_GENERAL_COUNT",
    // 官网 `风林火山` 标签下的全部 48 位（核心风火林山 32 + 后续/SP 16）；
    // 技能按批次实现，未实现武将不进选将池（`skills::general_ready`）。
    gids: [
      27, 28, 29, 30, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
      49, 50, 51, 52, 53, 54, 55, 56, 57, 61, 100, 210, 211, 212, 362, 400, 401,
      402, 403, 404, 406, 407, 408, 409, 410, 411, 413, 414, 415,
    ],
    femaleGids: new Set([30, 46, 57, 411]),
  },
];

const DEFAULT_GENERAL_PACKS = ["standard", "wind_fire_forest_mountain"];

const DEF_CODE = {
  // 标准包
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
  // 军争包（P4.0 入定义；结算见 P4.3）
  酒: "jiu",
  火杀: "huo_sha",
  雷杀: "lei_sha",
  火攻: "huo_gong",
  兵粮寸断: "bingliang_cunduan",
  铁索连环: "tiesuo_lianhuan",
  白银狮子: "baiyin_shizi",
  藤甲: "tengjia",
  古锭刀: "guding_dao",
  朱雀羽扇: "zhuque_yushan",
  骅骝: "hualiu",
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
  const opts = {
    cards: DEFAULT_CARDS,
    heroes: DEFAULT_HEROES,
    out: DEFAULT_OUT,
    generalPacks: DEFAULT_GENERAL_PACKS,
    check: false,
  };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--cards") opts.cards = resolve(argv[++i]);
    else if (argv[i] === "--heroes") opts.heroes = resolve(argv[++i]);
    else if (argv[i] === "--out") opts.out = resolve(argv[++i]);
    else if (argv[i] === "--general-packs") {
      opts.generalPacks = String(argv[++i] ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      if (!opts.generalPacks.length) {
        console.error("--general-packs 不能为空");
        process.exit(1);
      }
      if (opts.generalPacks.length === 1 && opts.generalPacks[0] === "all") {
        opts.generalPacks = GENERAL_PACKS.map((p) => p.key);
      }
    } else if (argv[i] === "--check") opts.check = true;
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log(
        "用法：node apps/scripts/gen-sanguosha-engine-data.mjs [--cards cards.json] [--heroes heroes.json] [--out data.rs] [--general-packs standard|all] [--check]",
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

  const defs = new Map();
  const decks = [];
  for (const deck of DECKS) {
    const picked = all.filter((c) => c.deck === deck.key);
    if (picked.length !== deck.size) {
      throw new Error(`${deck.label}牌堆应为 ${deck.size} 张，实际 ${picked.length}`);
    }
    const physical = [];
    for (const c of picked) {
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
        // 同名定义可能同时出现在多个牌堆（桃/闪/无懈可击）：字段必须完全一致。
        const same = [
          "name",
          "category",
          "equip_slot",
          "range_value",
          "range_type",
          "target_mode",
          "delayed",
        ].every((k) => JSON.stringify(prev[k]) === JSON.stringify(def[k]));
        if (!same) throw new Error(`同名定义字段不一致：${c.name}`);
      } else {
        defs.set(code, def);
      }
      physical.push({ pack: deck.key, def: code, suit, rank: rankNum(c.rank) });
    }
    decks.push({ ...deck, physical });
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
  const deckRows = decks.map((deck) => ({
    ...deck,
    rows: deck.physical.map(
      (c) =>
        `    PhysicalCard { pack: ${rustStr(c.pack)}, def: ${rustStr(c.def)}, suit: Suit::${c.suit}, rank: ${c.rank} },`,
    ),
  }));
  return {
    defs,
    decks: deckRows,
    defRows,
    total: deckRows.reduce((n, d) => n + d.physical.length, 0),
  };
}

function resolveGeneralPacks(keys) {
  const unknown = keys.filter((k) => !GENERAL_PACKS.some((p) => p.key === k));
  if (unknown.length) {
    throw new Error(
      `未知武将包：${unknown.join(", ")}（可选：${GENERAL_PACKS.map((p) => p.key).join(", ")}）`,
    );
  }
  return GENERAL_PACKS.filter((p) => keys.includes(p.key));
}

function buildGenerals(payload, packKeys) {
  const all = Array.isArray(payload.heroes) ? payload.heroes : [];
  const built = [];
  for (const pack of resolveGeneralPacks(packKeys)) {
    if (!pack.femaleGids) throw new Error(`武将包 ${pack.key} 缺少性别表（femaleGids）`);
    const heroes = all
      .filter((h) => h.pack === pack.label && (pack.gids === null || pack.gids.includes(h.gid)))
      .sort((a, b) => a.gid - b.gid);
    if (pack.gids) {
      for (const gid of pack.gids) {
        if (!heroes.some((h) => h.gid === gid)) {
          throw new Error(`heroes.json 缺少 ${pack.label}包 gid=${gid}`);
        }
      }
    }
    if (!heroes.length) throw new Error(`heroes.json 缺少 ${pack.label}包武将`);
    const picked = heroes.map((h) => {
      const skills = Array.isArray(h.skills) ? h.skills : [];
      return {
        code: `sgs_hero_${h.gid}`,
        gid: h.gid,
        name: h.name,
        pack: pack.key,
        kingdom: h.kingdom_code,
        max_hp: h.hp,
        gender: pack.femaleGids.has(h.gid) ? "female" : "male",
        is_lord: skills.some((s) => s.type_guess === "lord"),
        skills: skills.map((s) => s.name),
      };
    });
    const rows = picked.map((g) =>
      [
        "    GeneralDef {",
        ` code: ${rustStr(g.code)}, gid: ${g.gid}, name: ${rustStr(g.name)}, pack: ${rustStr(g.pack)},`,
        ` kingdom: ${rustStr(g.kingdom)}, max_hp: ${g.max_hp}, gender: ${rustStr(g.gender)}, is_lord: ${g.is_lord},`,
        ` skills: &[${g.skills.map(rustStr).join(", ")}],`,
        "},",
      ].join(""),
    );
    built.push({ ...pack, picked, rows });
  }
  return { built, total: built.reduce((n, p) => n + p.picked.length, 0) };
}

function render({ defs, decks, defRows, total, generals, generalTotal }) {
  const lines = [];
  lines.push("//! 三国杀引擎内置数据（**自动生成，勿手改**）。");
  lines.push("//!");
  lines.push("//! 生成：`node apps/scripts/gen-sanguosha-engine-data.mjs`");
  lines.push("//! 校验：`node apps/scripts/gen-sanguosha-engine-data.mjs --check`");
  lines.push(
    "//! 来源：`docs/games/sanguosha/data/cards.json`（standard + junzheng）+ `heroes.json`（按包生成）。",
  );
  lines.push("");
  lines.push("use crate::state::{Category, EquipSlot, RangeType, Suit, TargetMode};");
  lines.push("");
  for (const deck of decks) {
    lines.push(`pub const ${deck.sizeConst}: usize = ${deck.physical.length};`);
  }
  lines.push(`/// 全部牌堆包合计（standard + junzheng = 160）。`);
  lines.push(`pub const FULL_DECK_SIZE: usize = ${total};`);
  for (const pack of generals) {
    if (pack.key !== "standard") lines.push("#[allow(dead_code)]");
    lines.push(`pub const ${pack.countConst}: usize = ${pack.picked.length};`);
  }
  lines.push(`/// 已生成的全部武将数（所有包合计）。`);
  lines.push(`pub const GENERAL_COUNT: usize = ${generalTotal};`);
  lines.push("");
  lines.push("/// 支持的牌堆包（顺序即建堆顺序）。");
  lines.push(
    `pub static DECK_PACKS: &[&str] = &[${decks.map((d) => rustStr(d.key)).join(", ")}];`,
  );
  lines.push("");
  lines.push("/// 牌的定义（同一 `code` 只有一条；物理牌见各 `*_DECK`）。");
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
  lines.push("pub static CARD_DEFS: &[CardDef] = &[");
  lines.push(...defRows);
  lines.push("];");
  lines.push("");
  lines.push("/// 牌堆中的每一张物理牌（所属包 + 花色 + 点数）。");
  lines.push("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
  lines.push("pub struct PhysicalCard {");
  lines.push("    /// 所属牌堆包（`standard` / `junzheng`）。");
  lines.push("    pub pack: &'static str,");
  lines.push("    pub def: &'static str,");
  lines.push("    pub suit: Suit,");
  lines.push("    pub rank: u8,");
  lines.push("}");
  lines.push("");
  for (const deck of decks) {
    lines.push(`/// ${deck.label}包牌堆（${deck.physical.length} 张）。`);
    lines.push("#[rustfmt::skip]");
    lines.push(`pub static ${deck.staticName}: &[PhysicalCard] = &[`);
    lines.push(...deck.rows);
    lines.push("];");
    lines.push("");
  }
  lines.push("/// 全部牌堆包（顺序即建堆顺序）。");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static ALL_DECKS: &[(&str, &[PhysicalCard])] = &[");
  for (const deck of decks) {
    lines.push(`    (${rustStr(deck.key)}, ${deck.staticName}),`);
  }
  lines.push("];");
  lines.push("");
  lines.push("/// 武将（`code` 与中台 `sgs_generals.code` 一致；`pack` 为武将包 code）。");
  lines.push("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
  lines.push("pub struct GeneralDef {");
  lines.push("    pub code: &'static str,");
  lines.push("    pub gid: u32,");
  lines.push("    pub name: &'static str,");
  lines.push("    /// 武将包 code（`standard` / `wind_fire_forest_mountain`；选将池按 `Config.general_packs` 过滤）。");
  lines.push("    pub pack: &'static str,");
  lines.push("    pub kingdom: &'static str,");
  lines.push("    pub max_hp: i32,");
  lines.push("    /// 性别（male / female；用于离间、雌雄双股剑等）。");
  lines.push("    pub gender: &'static str,");
  lines.push("    /// 是否主公将（技能含主公技）。");
  lines.push("    pub is_lord: bool,");
  lines.push("    /// 技能名（展示与技能注册表对照用）。");
  lines.push("    pub skills: &'static [&'static str],");
  lines.push("}");
  lines.push("");
  for (const pack of generals) {
    lines.push(`/// ${pack.label}包武将（${pack.picked.length} 位）。`);
    lines.push("#[rustfmt::skip]");
    lines.push(`pub static ${pack.staticName}: &[GeneralDef] = &[`);
    lines.push(...pack.rows);
    lines.push("];");
    lines.push("");
  }
  lines.push("/// 已生成的武将包（按包顺序拼接即选将池）。");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static GENERAL_PACKS: &[&[GeneralDef]] = &[");
  for (const pack of generals) {
    lines.push(`    ${pack.staticName},`);
  }
  lines.push("];");
  lines.push("");
  lines.push("/// 支持的武将包 key（顺序即选将池包顺序，用于 `Config.general_packs` 归一化）。");
  lines.push(
    `pub static GENERAL_PACK_KEYS: &[&str] = &[${generals.map((p) => rustStr(p.key)).join(", ")}];`,
  );
  lines.push("");
  lines.push("/// 按 `Config.packs` 组合牌堆（顺序固定：standard → junzheng）。");
  lines.push("///");
  lines.push("/// 未知包或空组合返回错误（`lobby:set_packs` 与建堆共用同一套校验）。");
  lines.push("pub fn deck_cards(packs: &[String]) -> Result<Vec<PhysicalCard>, String> {");
  lines.push("    if packs.is_empty() {");
  lines.push('        return Err("牌堆包组合不能为空".into());');
  lines.push("    }");
  lines.push("    for pack in packs {");
  lines.push("        if !DECK_PACKS.contains(&pack.as_str()) {");
  lines.push('            return Err(format!("未知牌堆包：{pack}"));');
  lines.push("        }");
  lines.push("    }");
  lines.push("    let mut out = Vec::new();");
  lines.push("    for (name, deck) in ALL_DECKS {");
  lines.push("        if packs.iter().any(|p| p == name) {");
  lines.push("            // 生成脚本契约：同一牌堆里的物理牌 `pack` 必须与牌堆名一致。");
  lines.push("            if let Some(bad) = deck.iter().find(|c| c.pack != *name) {");
  lines.push("                return Err(format!(\"牌堆 {name} 含跨包牌：{}\", bad.def));");
  lines.push("            }");
  lines.push("            out.extend_from_slice(deck);");
  lines.push("        }");
  lines.push("    }");
  lines.push("    Ok(out)");
  lines.push("}");
  lines.push("");
  lines.push("pub fn card_def(code: &str) -> Option<&'static CardDef> {");
  lines.push("    CARD_DEFS.iter().find(|d| d.code == code)");
  lines.push("}");
  lines.push("");
  lines.push("pub fn general(code: &str) -> Option<&'static GeneralDef> {");
  lines.push("    GENERAL_PACKS");
  lines.push("        .iter()");
  lines.push("        .flat_map(|p| p.iter())");
  lines.push("        .find(|g| g.code == code)");
  lines.push("}");
  lines.push("");
  lines.push("/// 全部武将码（按包顺序 + gid 升序；调试/断言用）。");
  lines.push("#[allow(dead_code)]");
  lines.push("pub fn all_general_codes() -> Vec<&'static str> {");
  lines.push("    GENERAL_PACKS");
  lines.push("        .iter()");
  lines.push("        .flat_map(|p| p.iter())");
  lines.push("        .map(|g| g.code)");
  lines.push("        .collect()");
  lines.push("}");
  lines.push("");
  lines.push("/// 指定武将包组合内的武将码（按包顺序 + gid 升序）；未启用的包不参与。");
  lines.push("pub fn general_codes_in_packs(packs: &[String]) -> Vec<&'static str> {");
  lines.push("    GENERAL_PACKS");
  lines.push("        .iter()");
  lines.push("        .flat_map(|p| p.iter())");
  lines.push("        .filter(|g| packs.iter().any(|p| p == g.pack))");
  lines.push("        .map(|g| g.code)");
  lines.push("        .collect()");
  lines.push("}");
  lines.push("");
  return lines.join("\n");
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const cards = JSON.parse(await readFile(opts.cards, "utf8"));
  const heroes = JSON.parse(await readFile(opts.heroes, "utf8"));
  const built = buildCards(cards);
  const gens = buildGenerals(heroes, opts.generalPacks);
  const text = render({
    ...built,
    generals: gens.built,
    generalTotal: gens.total,
  });

  const deckSummary = built.decks
    .map((d) => `${d.label} ${d.physical.length}`)
    .join(" + ");
  const generalSummary = gens.built
    .map((p) => `${p.label} ${p.picked.length}`)
    .join(" + ");

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
    console.log(
      `[check] OK：${built.total} 张牌（${deckSummary}）/ ${gens.total} 位武将（${generalSummary}），${opts.out} 已是最新`,
    );
    return;
  }

  await mkdir(dirname(opts.out), { recursive: true });
  await writeFile(opts.out, text, "utf8");
  console.log(
    `[gen] ${built.defs.size} 个牌定义 / ${built.total} 张物理牌（${deckSummary}）/ ${gens.total} 位武将（${generalSummary}）→ ${opts.out}`,
  );
}

main().catch((err) => {
  console.error("[gen] 失败:", err);
  process.exit(1);
});
