#!/usr/bin/env node
// 由 data/cards.json 生成 docs/games/sanguosha/cards.md。
//
// 用法：
//   node apps/scripts/gen-sanguosha-cards-md.mjs
//   node apps/scripts/gen-sanguosha-cards-md.mjs --in docs/games/sanguosha/data/cards.json --out docs/games/sanguosha/cards.md

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const DEFAULT_IN = resolve("docs", "games", "sanguosha", "data", "cards.json");
const DEFAULT_OUT = resolve("docs", "games", "sanguosha", "cards.md");

const DECK_LABEL = { standard: "标准包", junzheng: "军争" };
const CATEGORY_ORDER = ["basic", "trick", "delayed_trick", "weapon", "armor", "horse"];
const CATEGORY_LABEL = {
  basic: "基本牌",
  trick: "锦囊牌",
  delayed_trick: "延时锦囊",
  weapon: "武器",
  armor: "防具",
  horse: "坐骑",
};
const SLOT_LABEL = {
  weapon: "武器",
  armor: "防具",
  horse_offense: "进攻马",
  horse_defense: "防御马",
};

function parseArgs(argv) {
  const opts = { in: DEFAULT_IN, out: DEFAULT_OUT };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--in") opts.in = resolve(argv[++i]);
    else if (argv[i] === "--out") opts.out = resolve(argv[++i]);
    else if (argv[i] === "--help" || argv[i] === "-h") {
      console.log("用法：node apps/scripts/gen-sanguosha-cards-md.mjs [--in cards.json] [--out cards.md]");
      process.exit(0);
    } else {
      console.error(`未知参数：${argv[i]}`);
      process.exit(1);
    }
  }
  return opts;
}

function cell(s) {
  return String(s ?? "").replace(/\r?\n/g, "<br>").replace(/\|/g, "\\|").trim();
}

function groupByName(cards) {
  const map = new Map();
  for (const c of cards) {
    if (!map.has(c.name)) map.set(c.name, []);
    map.get(c.name).push(c);
  }
  return [...map.entries()]
    .map(([name, list]) => ({ name, list, count: list.length }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name, "zh-Hans-CN"));
}

function suits(list) {
  return list.map((c) => `${c.suit}${c.rank}`).join("、");
}

function note(item) {
  const c = item.list[0];
  if (c.category === "basic") {
    if (c.name === "火杀") return "火焰伤害";
    if (c.name === "雷杀") return "雷电伤害";
    if (c.name === "酒") return "出牌阶段加伤 / 濒死回复";
    if (c.name === "闪") return "响应【杀】";
    if (c.name === "桃") return "回复体力 / 濒死救援";
    return "—";
  }
  if (c.category === "weapon") return `攻击范围 ${c.range_value}`;
  if (c.category === "armor") return "防具";
  if (c.category === "horse") return c.equip_slot === "horse_offense" ? "进攻马（攻击距离 +1）" : "防御马（防御距离 +1）";
  const bits = [];
  if (c.delayed) bits.push("延时锦囊");
  if (c.trick_subtype) bits.push(c.trick_subtype);
  if (c.target_mode === "aoe") bits.push("AOE");
  else if (c.target_mode === "single") bits.push("单目标");
  if (typeof c.range_value === "number") bits.push(`距离 ${c.range_value}`);
  return bits.length ? bits.join(" / ") : "—";
}

function sectionLines(deck, cards) {
  const lines = [];
  for (const cat of CATEGORY_ORDER) {
    const items = groupByName(cards.filter((c) => c.category === cat));
    if (items.length === 0) continue;
    const total = items.reduce((a, x) => a + x.count, 0);
    lines.push(`### ${CATEGORY_LABEL[cat]}（${total}）`);
    lines.push("");
    lines.push("| 牌名 | 数量 | 花色点数 | 说明 |");
    lines.push("|---|---:|---|---|");
    for (const item of items) {
      lines.push(`| ${cell(item.name)} | ${item.count} | ${cell(suits(item.list))} | ${cell(note(item))} |`);
    }
    lines.push("");
  }
  return lines;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const data = JSON.parse(await readFile(opts.in, "utf8"));
  const cards = data.cards ?? [];

  const lines = [];
  lines.push("# 三国杀牌表");
  lines.push("");
  lines.push("> 本文件由 `docs/games/sanguosha/data/cards.json` 自动生成，**请勿手工编辑**；数据更新后重新运行生成脚本。");
  lines.push("> 来源：社区开源牌堆数据（`npm sanguosha-agent-plugin@1.17.0` 内置 `STANDARD_DECK` / `JUNZHENG_DECK`）。");
  lines.push(`> 生成时间：${data.fetched_at ?? "—"}；标准包 ${data.counts?.standard ?? 0} 张，军争 ${data.counts?.junzheng ?? 0} 张，合计 ${data.counts?.total ?? cards.length} 张。`);
  lines.push("");
  lines.push("说明：");
  lines.push("");
  lines.push("- 花色：`♠` 黑桃 / `♥` 红桃 / `♣` 梅花 / `♦` 方块；点数 `A`–`K`（`A=1`、`J=11`、`Q=12`、`K=13`）。");
  lines.push("- `杀` 在军争中按 `damageType` 拆为 **火杀** / **雷杀**；标准包的 `杀` 为普通伤害。");
  lines.push("- 坐骑距离按项目口径：**进攻马 +1 攻击距离 / 防御马 +1 防御距离**（见 `rules.md` §7）。");
  lines.push("- 延时锦囊判定顺序为 **LIFO（后进先判）**；详见 `rules-rulings.md` §4.1。");
  lines.push("- 本表为**待人工复核**的初始化数据：花色/点数/数量已与标准包 108 张核对；军争 52 张按社区牌表整理。");
  lines.push("- 图片素材不使用官方图，后续自行制作。");
  lines.push("");
  lines.push("## 目录");
  lines.push("");
  for (const deck of ["standard", "junzheng"]) {
    const list = cards.filter((c) => c.deck === deck);
    if (list.length) lines.push(`- [${DECK_LABEL[deck]}](#${DECK_LABEL[deck]})（${list.length}）`);
  }
  lines.push("");

  for (const deck of ["standard", "junzheng"]) {
    const list = cards.filter((c) => c.deck === deck);
    if (!list.length) continue;
    lines.push(`## ${DECK_LABEL[deck]}`);
    lines.push("");
    lines.push(`> 共 ${list.length} 张。`);
    lines.push("");
    lines.push(...sectionLines(deck, list));
  }

  await mkdir(dirname(opts.out), { recursive: true });
  await writeFile(opts.out, lines.join("\n"), "utf8");
  console.log(`[gen] ${cards.length} 张牌 → ${opts.out}`);
}

main().catch((err) => {
  console.error("[gen] 失败:", err);
  process.exit(1);
});
