#!/usr/bin/env node
// V3 花色出口门禁（红颜 K1'）：花色改写只允许在**已登记的出口**发生。
//
// 口径（`align/B14.json` 红颜 + `v3-parity-baseline §7.20` K1'）：
// 不做全局属性改写；「使用/打出」时生成虚拟红桃牌，结算只看虚拟牌。
// 因此下面 3 个出口**必须**走单一助手，禁止再次出现「直接读物理花色」的分叉：
//   1) `card_play::push_virtual_use` —— 虚拟/转化牌花色（覆盖防具/属性/响应）
//   2) `judge::reveal` / `effective` —— 判定牌花色
//   3) `card_play::step_fire_attack` —— 他人展示手牌判色
//
// 用法：`node apps/scripts/check-sgs3-suit.mjs`（`npm run check:sgs3-suit`）。

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const KERNEL = path.join(root, "crates/engines/sanguosha3/src/kernel");

const tag = "[check:sgs3-suit]";
const problems = [];

function body(file, marker, endMarker) {
  const text = readFileSync(path.join(KERNEL, file), "utf8");
  const at = text.indexOf(marker);
  if (at < 0) return null;
  const end = endMarker ? text.indexOf(endMarker, at + marker.length) : -1;
  return end > at ? text.slice(at, end) : text.slice(at, at + 4000);
}

const checks = [
  {
    file: "card_play.rs",
    marker: "fn push_virtual_use(",
    end: "fn use_from_pile_as(",
    must: "skills::suit_for_use",
    why: "虚拟/转化牌花色必须走 `skills::suit_for_use`（红颜：黑桃→红桃）",
  },
  {
    file: "judge.rs",
    marker: "pub fn reveal(",
    end: "pub fn effective(",
    must: "skills::suit_for_judge",
    why: "判定牌花色必须走 `skills::suit_for_judge`",
  },
  {
    file: "judge.rs",
    marker: "pub fn effective(",
    end: null,
    must: "skills::suit_for_judge",
    why: "判定牌生效花色必须走 `skills::suit_for_judge`",
  },
  {
    file: "card_play.rs",
    marker: "fn step_fire_attack(",
    end: "fn step_collateral(",
    must: "skills::suit_of_card",
    why: "火攻展示牌判色必须走 `skills::suit_of_card`（他人读牌）",
  },
];

for (const c of checks) {
  const b = body(c.file, c.marker, c.end);
  if (b === null) {
    problems.push(`${c.file} 找不到 ${c.marker}（出口被改名/删除？）`);
    continue;
  }
  if (!b.includes(c.must)) problems.push(`${c.file}::${c.marker} 未使用 ${c.must}：${c.why}`);
}

if (problems.length) {
  console.error(`❌ ${tag} 失败（${problems.length} 条）：`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log(`✅ ${tag}：4 个花色出口均走单一助手（红颜 K1'）`);
