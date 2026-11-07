#!/usr/bin/env node
// 旧 crate 退役提醒（O4 裁定）：到期未删除则持续告警。
//
// 口径（用户 2026-11-06 裁定）：
//   主口径 = B20 验收签字日 + 30 天；硬上限 2027-01-31（取先到者）。
//   到期未删除 → 每次 `npm run sgs:check` 与会话开工提示告警；删除前需用户二次确认。
//
// 用法：node apps/scripts/check-sgs-retire.mjs [--strict]
//   --strict：过期直接以 exit 1 失败（CI 用；默认只告警）。

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const REPORT = path.join(projectRoot, "docs/games/sanguosha/legacy-retire.json");
const OLD_CRATE = path.join(projectRoot, "crates/engines/sanguosha");
const B20_SPEC = path.join(projectRoot, "docs/games/sanguosha/align/B20.json");
const strict = process.argv.includes("--strict");

if (!existsSync(OLD_CRATE)) {
  console.log("[retire] OK：旧 crate 已删除，无需提醒。");
  process.exit(0);
}
if (!existsSync(REPORT)) {
  console.log("[retire] 跳过：缺少 legacy-retire.json（未登记退役计划）。");
  process.exit(0);
}

const cfg = JSON.parse(readFileSync(REPORT, "utf8"));
const DAY = 24 * 3600 * 1000;
const today = new Date(new Date().toISOString().slice(0, 10));

function parseDate(s) {
  const d = new Date(`${s}T00:00:00Z`);
  return Number.isNaN(d.getTime()) ? null : d;
}

let deadline = parseDate(cfg.hard_deadline);
let source = `硬上限 ${cfg.hard_deadline}`;

// B20 验收签字 → deadline = 签字日 + 30 天（若早于硬上限则采用）。
if (existsSync(B20_SPEC)) {
  try {
    const spec = JSON.parse(readFileSync(B20_SPEC, "utf8"));
    if (spec.status === "verified" && spec.confirmed_at) {
      const verified = parseDate(spec.confirmed_at.slice(0, 10));
      if (verified) {
        const plus30 = new Date(verified.getTime() + 30 * DAY);
        if (!deadline || plus30 < deadline) {
          deadline = plus30;
          source = `B20 验收 ${spec.confirmed_at} + 30 天`;
        }
      }
    }
  } catch {
    /* 忽略解析失败 */
  }
}

if (!deadline) {
  console.log("[retire] 跳过：deadline 未设置。");
  process.exit(0);
}

const days = Math.ceil((deadline.getTime() - today.getTime()) / DAY);
const iso = deadline.toISOString().slice(0, 10);

if (days > 0) {
  console.log(`[retire] 旧 crate 退役截止 ${iso}（${source}），剩余 ${days} 天。`);
  process.exit(0);
}

const msg = [
  `[retire] ⚠ 旧 crate 退役已过期 ${-days} 天（截止 ${iso}，${source}）。`,
  "  请确认是否删除 `crates/engines/sanguosha`（删除前需用户二次确认；对拍语料保留在 data/replay/）。",
];
for (const line of msg) console.error(line);
process.exit(strict ? 1 : 0);
