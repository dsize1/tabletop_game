#!/usr/bin/env node
// 由 docs/games/sanguosha/data/{cards,heroes}.json 生成引擎内置数据表
// （v1 crate 已于 2026-11-10 删除；现只生成 v2 `cards_generated.rs` / `generals_generated.rs` / `engine-catalog.json`）。
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

import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fixedGeneralName } from "./lib/sgs-name-fixes.mjs";

const DEFAULT_CARDS = resolve("docs", "games", "sanguosha", "data", "cards.json");
const DEFAULT_HEROES = resolve("docs", "games", "sanguosha", "data", "heroes.json");
const DEFAULT_OUT = resolve("crates", "engines", "sanguosha", "src", "data.rs");
/// 内核 v2（`sanguosha2`）的牌表产物：只含牌定义 + 160 张物理牌（不含武将）。
const DEFAULT_OUT_V2 = resolve("crates", "engines", "sanguosha2", "src", "cards_generated.rs");
const DEFAULT_OUT_V2_GENERALS = resolve(
  "crates",
  "engines",
  "sanguosha2",
  "src",
  "generals_generated.rs",
);
/// 门禁/脚本的机器可读名录（v1 `data.rs` 退役后，这是武将/牌数据的唯一来源）。
const DEFAULT_CATALOG = resolve("docs", "games", "sanguosha", "data", "engine-catalog.json");

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
  {
    key: "yijiang",
    label: "一将成名",
    staticName: "YIJIANG_GENERALS",
    countConst: "YIJIANG_GENERAL_COUNT",
    // 官网 `一将成名` 标签下的 72 位；按批次实现，未实现武将不进选将池。
    gids: [
      151, 152, 153, 155, 156, 157, 158, 160, 161, 162, 164, 166, 167, 168, 169,
      170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184,
      185, 186, 187, 188, 321, 322, 323, 324, 325, 326, 327, 328, 329, 330, 331,
      332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 380, 381, 382, 383,
      384, 385, 386, 387, 388, 389, 390, 391, 392, 393, 394, 395,
    ],
    // 官网数据无性别字段：按官方设定维护（P4.4.2 批次 1 起）。
    femaleGids: new Set([157, 161, 168, 183, 187, 327, 331, 336, 380, 388, 389, 390, 391]),
  },
  {
    key: "jiexian",
    label: "界限突破",
    staticName: "JIEXIAN_GENERALS",
    countConst: "JIEXIAN_GENERAL_COUNT",
    // 官网 `界限突破` 标签下的 87 位（与标准/风林火山同名武将互为「界化」版本，见 ADR-0024）。
    gids: [
      26, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312,
      313, 314, 315, 316, 317, 318, 319, 320, 440, 441, 442, 443, 444, 445, 446,
      447, 448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458, 462, 463, 470,
      471, 479, 480, 484, 485, 486, 492, 496, 505, 515, 526, 550, 563, 587, 588,
      597, 599, 600, 603, 604, 610, 620, 621, 625, 626, 627, 628, 629, 640, 641,
      642, 658, 671, 676, 690, 696, 726, 749, 752, 753, 756, 775,
    ],
    femaleGids: new Set([309, 443, 444, 445, 457, 479, 505, 621, 625, 627, 640, 671, 726, 752, 775]),
  },
];

// 生成进 data.rs 的包（全量登记；未实现武将靠 `skills::general_ready` 不进选将池）。
// 注意：`Config.general_packs` 的**运行时默认**仍是 `["standard"]`（见 engine `state.rs`）。
const DEFAULT_GENERAL_PACKS = [
  "standard",
  "wind_fire_forest_mountain",
  "yijiang",
  "jiexian",
];

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

// ---------------------------------------------------------------------------
// 内核 v2（`sanguosha2/src/cards_generated.rs`）映射。
// 口径：v2 **只读生成物**；本文件是唯一「牌名 → 属性/效果」的映射点，v2 内核不得按牌名判定。
// ---------------------------------------------------------------------------

/// 牌效果（`cards::Effect`）。新增牌必须在此登记（否则生成失败）。
const V2_EFFECT = {
  杀: "Sha",
  火杀: "Sha",
  雷杀: "Sha",
  闪: "Shan",
  桃: "Tao",
  酒: "Jiu",
  无懈可击: "Wuxie",
  决斗: "Duel",
  过河拆桥: "Dismantle",
  顺手牵羊: "Snatch",
  无中生有: "ExNihilo",
  南蛮入侵: "SavageAssault",
  万箭齐发: "ArcheryAttack",
  桃园结义: "PeachGarden",
  五谷丰登: "Harvest",
  火攻: "FireAttack",
  借刀杀人: "Collateral",
  铁索连环: "IronChain",
  乐不思蜀: "Indulgence",
  兵粮寸断: "SupplyShortage",
  闪电: "Lightning",
};

/// 防具（`cards::ArmorKind`）。未登记的防具 = `None`。
const V2_ARMOR = {
  八卦阵: "Bagua",
  仁王盾: "Renwang",
  藤甲: "Tengjia",
  白银狮子: "Baiyin",
};

/// 武器（`cards::WeaponKind`）。未登记的武器 = `None`（只有攻击范围）。
const V2_WEAPON = {
  青釭剑: "Qinggang",
  雌雄双股剑: "Cixiong",
  寒冰剑: "Hanbing",
  青龙偃月刀: "Qinglong",
  贯石斧: "Guanshi",
  方天画戟: "Fangtian",
  麒麟弓: "Qilin",
  诸葛连弩: "Zhugeliannu",
  古锭刀: "Gudingdao",
  丈八蛇矛: "Zhangba",
  朱雀羽扇: "Zhuque",
};

/// 目标模式（`cards::TargetMode`）：JSON 的 `target_mode` 只有 single/aoe，
/// 「只对自己 / 必须选人」按 v2 口径显式覆盖（禁止用牌名启发式）。
const V2_TARGET = {
  杀: "Chosen",
  火杀: "Chosen",
  雷杀: "Chosen",
  闪: "SelfOnly",
  桃: "SelfOnly",
  酒: "SelfOnly",
  无懈可击: "SelfOnly",
  无中生有: "SelfOnly",
  铁索连环: "Chosen",
  乐不思蜀: "Chosen",
  兵粮寸断: "Chosen",
  闪电: "SelfOnly",
};

/// 响应牌（`cards::ResponseKind`）：该牌要求的响应。
const V2_RESPOND = {
  杀: "Shan",
  火杀: "Shan",
  雷杀: "Shan",
  南蛮入侵: "Sha",
  万箭齐发: "Shan",
  决斗: "Sha",
};

/// 不在 160 张牌堆内、但被测试夹具/沙盒使用的牌（宝物等）。
const V2_EXTRA_DEFS = [
  {
    name: "木牛流马",
    category: "treasure",
    equip_slot: "treasure",
    range_value: null,
  },
];

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
    outV2: DEFAULT_OUT_V2,
    outV2Generals: DEFAULT_OUT_V2_GENERALS,
    catalog: DEFAULT_CATALOG,
    generalPacks: DEFAULT_GENERAL_PACKS,
    check: false,
  };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--cards") opts.cards = resolve(argv[++i]);
    else if (argv[i] === "--heroes") opts.heroes = resolve(argv[++i]);
    else if (argv[i] === "--out") opts.out = resolve(argv[++i]);
    else if (argv[i] === "--out-v2") opts.outV2 = resolve(argv[++i]);
    else if (argv[i] === "--out-v2-generals") opts.outV2Generals = resolve(argv[++i]);
    else if (argv[i] === "--catalog") opts.catalog = resolve(argv[++i]);
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
        "用法：node apps/scripts/gen-sanguosha-engine-data.mjs [--cards cards.json] [--heroes heroes.json] [--out data.rs] [--out-v2 cards_generated.rs] [--catalog engine-catalog.json] [--general-packs standard|all] [--check]",
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

/// v2 产物：牌定义 + 160 张物理牌（不生成武将：v2 尚未使用武将表）。
function buildV2(built, payload) {
  const damageType = new Map();
  for (const c of payload.cards ?? []) {
    if (c.damage_type) damageType.set(c.name, c.damage_type);
  }
  const slotOf = (equipSlot) => (equipSlot ? SLOT[equipSlot] : null);
  const rows = [];
  const addDef = (d) => {
    const effect = V2_EFFECT[d.name];
    const cat = String(d.category).toLowerCase();
    const isEquip = ["weapon", "armor", "horse", "treasure"].includes(cat);
    if (!effect && !isEquip) throw new Error(`v2 未登记牌效果：${d.name}`);
    const kind = isEquip ? "Equip" : cat === "basic" ? "Basic" : "Trick";
    const targetMode =
      V2_TARGET[d.name] ??
      (cat === "trick" || cat === "delayed_trick"
        ? d.target_mode === "aoe"
          ? "AllAlive"
          : "Chosen"
        : "SelfOnly");
    const dt = damageType.get(d.name);
    const damageKind = dt === "fire" ? "Fire" : dt === "thunder" ? "Thunder" : "Normal";
    rows.push({
      code: d.code,
      name: d.name,
      kind,
      slot: slotOf(d.equip_slot),
      weapon_range: cat === "weapon" ? d.range_value : null,
      horse: cat === "horse" ? (d.range_value ?? 1) : null,
      damage_kind: damageKind,
      damage_trick: ["决斗", "万箭齐发", "南蛮入侵", "火攻"].includes(d.name),
      effect: effect ?? "None",
      armor: V2_ARMOR[d.name] ?? "None",
      weapon: V2_WEAPON[d.name] ?? "None",
      target_mode: targetMode,
      respond: V2_RESPOND[d.name] ?? null,
      sha: ["杀", "火杀", "雷杀"].includes(d.name),
      shan: d.name === "闪",
      tao: d.name === "桃",
      jiu: d.name === "酒",
      wuxie: d.name === "无懈可击",
      duel: d.name === "决斗",
      delayed: Boolean(d.delayed),
      lianhuan: d.name === "铁索连环",
    });
  };
  for (const d of built.defs.values()) addDef(d);
  for (const d of V2_EXTRA_DEFS) {
    if (built.defs.has(d.name)) continue;
    addDef({
      name: d.name,
      category: d.category,
      equip_slot: d.equip_slot,
      range_value: d.range_value,
      target_mode: null,
      delayed: false,
    });
  }

  const deckRows = [];
  for (const deck of built.decks) {
    for (const c of deck.physical) {
      const def = built.defs.get(c.def);
      if (!def) throw new Error(`物理牌缺少定义：${c.def}`);
      deckRows.push({ name: def.name, pack: c.pack, suit: c.suit, rank: c.rank });
    }
  }
  return { rows, deckRows };
}

/** 四个「技能已全部实现」的包 key（其余包的武将标记为待审批 review）。 */
const COMPLETE_PACKS = new Set(["standard", "wind_fire_forest_mountain", "yijiang", "jiexian"]);

/**
 * 武将状态（与中台 sgs_generals.status 同口径）：
 *  - review：技能未完成（不在四包内）
 *  - offline：被「界/将」同名去重排除的原版
 *  - live：可选将池（147）
 */
function assignGeneralStatus(generals) {
  const norm = (name) => name.replace(/^(界|将)/, "");
  const jieNames = new Set(
    generals.filter((g) => g.pack === "jiexian").map((g) => norm(g.name)),
  );
  return generals.map((g) => {
    let status = "live";
    if (!COMPLETE_PACKS.has(g.pack)) status = "review";
    else if (g.pack !== "jiexian" && jieNames.has(norm(g.name))) status = "offline";
    return { ...g, status };
  });
}

function renderV2Generals(generals) {
  const L = [];
  L.push("//! 生成的武将名录（化身池 / 中台状态）。**勿手改**：由 gen:sgs-engine-data 生成。");
  L.push("");
  L.push("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
  L.push("pub enum Status {");
  L.push("    /// 上线（进选将池 / 化身池）。");
  L.push("    Live,");
  L.push("    /// 下线（被「界/将」同名去重排除的原版）。");
  L.push("    Offline,");
  L.push("    /// 待审批（技能未完成）。");
  L.push("    Review,");
  L.push("}");
  L.push("");
  L.push("#[derive(Debug, Clone, Copy)]");
  L.push("pub struct GeneralDef {");
  L.push("    pub code: &'static str,");
  L.push("    /// 官网 gid（素材映射/中台主键）。");
  L.push("    pub gid: u32,");
  L.push("    pub name: &'static str,");
  L.push("    /// 武将包 key（`standard` / `wind_fire_forest_mountain` / `yijiang` / `jiexian`）。");
  L.push("    pub pack: &'static str,");
  L.push("    pub kingdom: &'static str,");
  L.push("    pub male: bool,");
  L.push("    pub max_hp: i32,");
  L.push("    pub lord: bool,");
  L.push("    pub skills: &'static [&'static str],");
  L.push("    pub status: Status,");
  L.push("}");
  L.push("");
  L.push("/// 武将包 key（顺序=选将/大厅菜单顺序；`standard` 恒为基数）。");
  {
    const keys = [...new Set(generals.map((g) => g.pack))];
    L.push(`pub const PACK_KEYS: &[&str] = &[${keys.map(rustStr).join(", ")}];`);
  }
  L.push("");
  L.push("pub const GENERALS: &[GeneralDef] = &[");
  for (const g of generals) {
    const status = g.status === "live" ? "Live" : g.status === "offline" ? "Offline" : "Review";
    const skills = (g.skills ?? []).map((x) => JSON.stringify(x)).join(", ");
    L.push("    GeneralDef {");
    L.push("        code: " + JSON.stringify(g.code) + ",");
    L.push("        gid: " + Number(g.gid) + ",");
    L.push("        name: " + JSON.stringify(g.name) + ",");
    L.push("        pack: " + JSON.stringify(g.pack) + ",");
    L.push("        kingdom: " + JSON.stringify(g.kingdom) + ",");
    L.push("        male: " + (g.gender === "male" ? "true" : "false") + ",");
    L.push("        max_hp: " + g.max_hp + ",");
    L.push("        lord: " + (g.is_lord ? "true" : "false") + ",");
    L.push("        skills: &[" + skills + "],");
    L.push("        status: Status::" + status + ",");
    L.push("    },");
  }
  L.push("];");
  L.push("");
  L.push("/// 按 code 查武将。");
  L.push("pub fn by_code(code: &str) -> Option<&'static GeneralDef> {");
  L.push("    GENERALS.iter().find(|g| g.code == code)");
  L.push("}");
  L.push("");
  L.push("/// 上线池（化身牌池 / 选将池）。");
  L.push("pub fn live() -> impl Iterator<Item = &'static GeneralDef> {");
  L.push("    GENERALS.iter().filter(|g| g.status == Status::Live)");
  L.push("}");
  L.push("");
  return L.join("\n");
}

function renderV2({ rows, deckRows }) {
  const opt = (v, fn) => (v === null || v === undefined ? "None" : `Some(${fn(v)})`);
  const lines = [];
  lines.push("//! 三国杀内核 v2 卡牌数据（**自动生成，勿手改**）。");
  lines.push("//!");
  lines.push(
    "//! 生成：`node apps/scripts/gen-sanguosha-engine-data.mjs`（同时生成 v1 `data.rs`）",
  );
  lines.push("//! 校验：`node apps/scripts/gen-sanguosha-engine-data.mjs --check`");
  lines.push(
    "//! 来源：`docs/games/sanguosha/data/cards.json`（standard 108 + junzheng 52 = 160）。",
  );
  lines.push("//!");
  lines.push(
    "//! 约束（`docs/conventions/skill-spec-rules.md` §3）：v2 内核与本表**只允许**按属性字段判定，",
  );
  lines.push("//! 禁止按牌名 `match`/字符串比较；新增牌必须先在生成脚本登记属性。");
  lines.push("");
  lines.push("use crate::cards::{ArmorKind, CardKind, DamageKind, Effect, TargetMode, WeaponKind};");
  lines.push("use crate::kernel::{Slot, Suit};");
  lines.push("");
  lines.push("/// 生成的全部物理牌数（standard + junzheng）。");
  lines.push(`pub const DECK_SIZE: usize = ${deckRows.length};`);
  lines.push("");
  lines.push("/// 牌定义（每张名称一条；物理牌实例见 `DECK`）。");
  lines.push("pub struct Row {");
  lines.push("    /// 定义码（与 v1 `CardDef.code` 一致；场景/模板 token 兼容用）。");
  lines.push("    pub code: &'static str,");
  lines.push("    pub name: &'static str,");
  lines.push("    pub kind: CardKind,");
  lines.push("    pub slot: Option<Slot>,");
  lines.push("    /// 武器攻击距离 / 坐骑距离。");
  lines.push("    pub weapon_range: Option<usize>,");
  lines.push("    pub horse: Option<usize>,");
  lines.push("    pub damage_kind: DamageKind,");
  lines.push("    /// 伤害锦囊（决斗/万箭/南蛮/火攻）。");
  lines.push("    pub damage_trick: bool,");
  lines.push("    pub effect: Effect,");
  lines.push("    pub armor: ArmorKind,");
  lines.push("    pub weapon: WeaponKind,");
  lines.push("    pub target_mode: TargetMode,");
  lines.push("    /// 该牌要求的响应牌（杀→闪 / 南蛮→杀 / 万箭→闪 / 决斗→杀）。");
  lines.push("    pub respond: Option<crate::cards::ResponseKind>,");
  lines.push("    pub sha: bool,");
  lines.push("    pub shan: bool,");
  lines.push("    pub tao: bool,");
  lines.push("    pub jiu: bool,");
  lines.push("    pub wuxie: bool,");
  lines.push("    pub duel: bool,");
  lines.push("    pub delayed: bool,");
  lines.push("    pub lianhuan: bool,");
  lines.push("}");
  lines.push("");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static DEFS: &[Row] = &[");
  for (const r of rows) {
    lines.push(
      `    Row { code: ${rustStr(r.code)}, name: ${rustStr(r.name)}, kind: CardKind::${r.kind}, slot: ${opt(
        r.slot,
        (v) => `Slot::${v}`,
      )}, weapon_range: ${opt(r.weapon_range, (v) => v)}, horse: ${opt(
        r.horse,
        (v) => v,
      )}, damage_kind: DamageKind::${r.damage_kind}, damage_trick: ${r.damage_trick}, effect: Effect::${r.effect}, armor: ArmorKind::${r.armor}, weapon: WeaponKind::${r.weapon}, target_mode: TargetMode::${r.target_mode}, respond: ${opt(
        r.respond,
        (v) => `crate::cards::ResponseKind::${v}`,
      )}, sha: ${r.sha}, shan: ${r.shan}, tao: ${r.tao}, jiu: ${r.jiu}, wuxie: ${r.wuxie}, duel: ${r.duel}, delayed: ${r.delayed}, lianhuan: ${r.lianhuan} },`,
    );
  }
  lines.push("];");
  lines.push("");
  lines.push("/// 物理牌（按建堆顺序：standard → junzheng）。");
  lines.push("pub struct Phy {");
  lines.push("    pub name: &'static str,");
  lines.push("    /// 所属牌堆包（`standard` / `junzheng`；大厅按包组堆）。");
  lines.push("    pub pack: &'static str,");
  lines.push("    pub suit: Suit,");
  lines.push("    pub rank: u8,");
  lines.push("}");
  lines.push("");
  lines.push("#[rustfmt::skip]");
  lines.push("pub static DECK: &[Phy] = &[");
  for (const c of deckRows) {
    lines.push(
      `    Phy { name: ${rustStr(c.name)}, pack: ${rustStr(c.pack)}, suit: Suit::${c.suit}, rank: ${c.rank} },`,
    );
  }
  lines.push("];");
  lines.push("");
  return lines.join("\n");
}

/// 机器可读名录（JSON）：武将 + 牌定义 + 物理牌 + 包清单。
///
/// 口径：与 v1 `data.rs` / v2 `cards_generated.rs` 同源同序（同一份生成逻辑），
/// 供门禁脚本读取；字段命名与 Rust 结构一致（Suit 用枚举名）。
function buildCatalog(built, gens, cardsPayload) {
  const defs = [...built.defs.values()].map((d) => ({
    code: d.code,
    name: d.name,
    category: d.category,
    equip_slot: d.equip_slot ?? null,
    range_value: d.range_value ?? null,
    range_type: d.range_type ?? null,
    target_mode: d.target_mode ?? null,
    delayed: Boolean(d.delayed),
    damage_type: (() => {
      const hit = (cardsPayload.cards ?? []).find((c) => c.name === d.name && c.damage_type);
      return hit ? hit.damage_type : null;
    })(),
  }));
  const decks = built.decks.map((deck) => ({
    key: deck.key,
    label: deck.label,
    size: deck.physical.length,
    cards: deck.physical.map((c) => ({
      pack: c.pack,
      def: c.def,
      name: defs.find((d) => d.code === c.def)?.name ?? c.def,
      suit: c.suit,
      rank: c.rank,
    })),
  }));
  const generals = gens.built.flatMap((pack) =>
    pack.picked.map((g) => ({
      code: g.code,
      gid: g.gid,
      name: g.name,
      pack: g.pack,
      kingdom: g.kingdom,
      max_hp: g.max_hp,
      gender: g.gender,
      is_lord: g.is_lord,
      skills: g.skills,
    })),
  );
  return {
    generated_by: "apps/scripts/gen-sanguosha-engine-data.mjs",
    note: "自动生成，勿手改。v1 data.rs 退役后，本文件是脚本/门禁的武将+牌名录唯一机器源。",
    source: {
      cards: "docs/games/sanguosha/data/cards.json",
      heroes: "docs/games/sanguosha/data/heroes.json",
    },
    card_counts: {
      standard: built.decks.find((d) => d.key === "standard")?.physical.length ?? 0,
      junzheng: built.decks.find((d) => d.key === "junzheng")?.physical.length ?? 0,
      total: built.total,
      defs: defs.length,
    },
    card_packs: built.decks.map((d) => ({ key: d.key, label: d.label, size: d.physical.length })),
    card_defs: defs,
    decks,
    general_counts: {
      total: gens.total,
      packs: gens.built.map((p) => ({ key: p.key, label: p.label, count: p.picked.length })),
    },
    general_pack_keys: gens.built.map((p) => p.key),
    generals,
  };
}

function renderCatalog(catalog) {
  return JSON.stringify(catalog, null, 2) + "\n";
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
        name: fixedGeneralName(h),
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
  const v2 = buildV2(built, cards);
  const textV2 = renderV2(v2);
  const catalog = buildCatalog(built, gens, cards);
  catalog.generals = assignGeneralStatus(catalog.generals);
  const textCatalog = renderCatalog(catalog);
  const textV2Generals = renderV2Generals(catalog.generals);

  const deckSummary = built.decks
    .map((d) => `${d.label} ${d.physical.length}`)
    .join(" + ");
  const generalSummary = gens.built
    .map((p) => `${p.label} ${p.picked.length}`)
    .join(" + ");

  // v1 crate 删除后 `--out` 目录不存在：v1 产物变为可选（S6 起只校验 v2 + catalog）。
  const v1Present = existsSync(dirname(opts.out));
  if (opts.check) {
    const targets = [
      ...(v1Present ? [{ out: opts.out, text, label: "v1 data.rs" }] : []),
      { out: opts.outV2, text: textV2, label: "v2 cards_generated.rs" },
      { out: opts.catalog, text: textCatalog, label: "engine-catalog.json" },
      { out: opts.outV2Generals, text: textV2Generals, label: "v2 generals_generated.rs" },
    ];
    for (const t of targets) {
      let current = "";
      try {
        current = await readFile(t.out, "utf8");
      } catch {
        console.error(`[check] 缺少生成文件（${t.label}）：${t.out}`);
        process.exit(1);
      }
      if (current !== t.text) {
        console.error(`[check] 生成文件过期（${t.label}）：${t.out}（请重新运行生成脚本）`);
        process.exit(1);
      }
    }
    console.log(
      `[check] OK：${built.total} 张牌（${deckSummary}）/ ${gens.total} 位武将（${generalSummary}）/ v2 ${v2.deckRows.length} 张物理牌 + ${v2.rows.length} 条定义 / catalog ${catalog.generals.length} 将 + ${catalog.card_defs.length} 定义，产品均为最新（v1 ${v1Present ? "已校验" : "已删除，跳过"}）`,
    );
    return;
  }

  if (v1Present) {
    await mkdir(dirname(opts.out), { recursive: true });
    await writeFile(opts.out, text, "utf8");
  }
  await mkdir(dirname(opts.outV2), { recursive: true });
  await writeFile(opts.outV2, textV2, "utf8");
  await mkdir(dirname(opts.catalog), { recursive: true });
  await writeFile(opts.catalog, textCatalog, "utf8");
  await mkdir(dirname(opts.outV2Generals), { recursive: true });
  await writeFile(opts.outV2Generals, textV2Generals, "utf8");
  if (v1Present) {
    console.log(
      `[gen] ${built.defs.size} 个牌定义 / ${built.total} 张物理牌（${deckSummary}）/ ${gens.total} 位武将（${generalSummary}）→ ${opts.out}`,
    );
  }
  console.log(
    `[gen] v2：${v2.rows.length} 条牌定义 / ${v2.deckRows.length} 张物理牌 → ${opts.outV2}`,
  );
  console.log(
    `[gen] v2 generals：${catalog.generals.filter((g) => g.status === "live").length} 上线 / ${catalog.generals.filter((g) => g.status === "offline").length} 下线 / ${catalog.generals.filter((g) => g.status === "review").length} 待审批 → ${opts.outV2Generals}`,
  );
  console.log(
    `[gen] catalog：${catalog.generals.length} 将 / ${catalog.card_defs.length} 牌定义 / ${catalog.decks.length} 牌堆 → ${opts.catalog}`,
  );
}

main().catch((err) => {
  console.error("[gen] 失败:", err);
  process.exit(1);
});
