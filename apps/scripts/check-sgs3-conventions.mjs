#!/usr/bin/env node
// V3 机制优先门禁（2026-11-16 用户裁定「行殇硬编码属于严重偏离」后新增）。
//
// 目的：防止 `kernel/*.rs`（非 skills/）再出现「技能名分支 / 直写体力」，
// 以及 skills 层绕过 `flow::*` 单出口直接改 hp/max_hp。
//
// 规则：
// C1 kernel 非 skills 文件不得出现 `skills::active/enabled/limited_used(..., "code")`
//    或 `take_answer(g, "<skill>")`；流程键（flow:/dying/pindian/relay/wuxie/judge）与显式白名单除外。
// C2 kernel 任何文件不得直接写 `Seat.hp` / `max_hp`（`flow.rs` 单出口、`turn.rs` 开局初始化除外）。
// C3 kernel 非 skills 文件不得出现已注册技能 code 的字面量（`guhuo.rs` = hidden-declaration 机制专用流程，白名单）。
//
// 用法：`node apps/scripts/check-sgs3-conventions.mjs`

import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const KERNEL = path.join(root, "crates/engines/sanguosha3/src/kernel");
const SKILLS_DIR = path.join(KERNEL, "skills");

const problems = [];
const tag = "[check:sgs3-conventions]";

const files = readdirSync(KERNEL).filter((f) => f.endsWith(".rs"));
const skillFiles = readdirSync(SKILLS_DIR)
  .filter((f) => f.endsWith(".rs"))
  .map((f) => path.join(SKILLS_DIR, f));

// 已注册技能 code（注册表事实源）
const registered = new Set();
for (const f of skillFiles) {
  const text = readFileSync(f, "utf8");
  for (const m of text.matchAll(/(?:code:\s*|skill!\(\s*)"([a-z0-9_]+)"/g)) {
    registered.add(m[1]);
  }
}

// 流程键：不是技能名（pending skill 通道 / 判定 / 拼点 / 接力）。
const FLOW_KEYS = new Set([
  "flow:card",
  "flow:play",
  "flow:discard",
  "dying",
  "pindian",
  "relay",
  "wuxie",
  "judge",
  "zhangba",
  "zhangba_pick",
]);

// C1 显式白名单（逐条理由；新增必须写理由）。
const C1_ALLOW = new Map([
  ["guhuo.rs", "hidden-declaration 机制专用流程（蛊惑）；暂以机制名作为流程键，登记在 v3-parity §7.26b"],
]);

// C2 白名单：体力单出口 / 开局初始化。
const C2_ALLOW = new Map([
  ["flow.rs", "flow::damage/lose_hp/heal/restore_hp/gain_max_hp 是体力唯一出口"],
  ["turn.rs", "begin_game 初始化体力（未开局写入，非对局中修改）"],
  ["mod.rs", "Game::default/测试装配字段初始化"],
  ["generals.rs", "选将落地：apply_to_seat 写入 max_hp/hp（开局前，非对局中修改）"],
]);

// C4 白名单：牌区移动底层 / 判定区移出（只发 JudgeZoneLeft）。
const C4_ALLOW = new Map([
  ["flow.rs", "flow::discard 底层移动"],
  ["card_play.rs", "to_discard_with / finish_card / 判定区 LIFO 底层"],
  ["judge.rs", "判定牌落盘底层"],
  ["gear.rs", "装备替换底层"],
  ["pindian.rs", "拼点牌落盘"],
  ["guhuo.rs", "蛊惑扣置牌落盘"],
  ["turn.rs", "死亡清理底层"],
  ["b14.rs", "涅槃：判定区移出只发 JudgeZoneLeft"],
]);

// C3 白名单：机制子系统（不是技能分支）。
const C3_ALLOW = new Map([
  ["guhuo.rs", "hidden-declaration（蛊惑）机制专用流程；K16 通用化前保留"],
]);

function rel(p) {
  return path.relative(KERNEL, p).replaceAll("\\", "/");
}

for (const f of files) {
  const name = rel(path.join(KERNEL, f));
  const text = readFileSync(path.join(KERNEL, f), "utf8");

  // C1
  if (name !== "skills.rs" && !C1_ALLOW.has(name)) {
    for (const m of text.matchAll(/skills::(?:active|enabled|limited_used|mark_limited_used|revoke)\([^\n]*?"([a-z0-9_]+)"/g)) {
      const code = m[1];
      if (FLOW_KEYS.has(code)) continue;
      if (/^flow:/.test(code)) continue;
      problems.push(`C1 ${name}: 内核出现技能名调用（${code}）——请改钩子/注册表查询`);
    }
    for (const m of text.matchAll(/take_answer\(g,\s*"([^"]+)"/g)) {
      const key = m[1];
      if (FLOW_KEYS.has(key) || key.startsWith("flow:")) continue;
      problems.push(`C1 ${name}: take_answer 使用了技能名键（${key}）——流程键不得是技能名`);
    }
  }

  // C2（只扫对局状态写入，排除比较）
  if (!C2_ALLOW.has(name)) {
    for (const m of text.matchAll(/\.\s*(hp|max_hp)\s*=(?!=)/g)) {
      problems.push(`C2 ${name}: 直接写 .${m[1]}——必须走 flow::damage/lose_hp/heal/restore_hp/gain_max_hp`);
    }
  }

  // C3
  if (name !== "skills.rs" && !C3_ALLOW.has(name)) {
    for (const code of registered) {
      if (new RegExp(`"${code}"`).test(text)) {
        problems.push(`C3 ${name}: 内核非技能文件出现技能 code 字面量（${code}）`);
      }
    }
  }
}

// C4：skills 层不得绕过单出口直接 insert Zone::Discard
for (const p of skillFiles) {
  const name = `skills/${path.basename(p)}`;
  if (C4_ALLOW.has(path.basename(p))) continue;
  const text = readFileSync(p, "utf8");
  if (/cards\.insert\([^)]*Zone::Discard/.test(text)) {
    problems.push(`C4 ${name}: 直接 insert Zone::Discard——请走 flow::discard / card_play::to_discard_with`);
  }
}

// skills 层：C2（不得直写 hp/max_hp）
for (const p of skillFiles) {
  const text = readFileSync(p, "utf8");
  for (const m of text.matchAll(/\.\s*(hp|max_hp)\s*=(?!=)/g)) {
    problems.push(`C2 skills/${path.basename(p)}: 直接写 .${m[1]}——必须走 flow::* 单出口`);
  }
}


// ---------------------------------------------------------------------------
// C5–C9（2026-11-16 审计后新增：把「机制优先」的欠账变成门禁）
// ---------------------------------------------------------------------------

// C3b：内核非 skills 文件不得出现**技能中文名**（C3 只扫 code，配置为文案时漏检：
// 父魂曾在 `turn.rs` 里以选项文案出现，绕过 C3）。
const names = new Set();
for (const f of skillFiles) {
  const text = readFileSync(f, "utf8");
  for (const m of text.matchAll(/name:\s*"([^"]+)"/g)) names.add(m[1]);
}
const C3B_ALLOW = new Map([
  ["card_play.rs", "濒死救援的历史播报文案（cause 文本，不是分支条件）"],
  ["flow.rs", "缔盟换手牌的历史播报文案（cause 文本）"],
  ["quick_judge.rs", "文档注释里的示例文案"],
]);
for (const f of files) {
  const name = rel(path.join(KERNEL, f));
  if (name === "skills.rs" || C3_ALLOW.has(name) || C3B_ALLOW.has(name)) continue;
  const text = readFileSync(path.join(KERNEL, f), "utf8");
  for (const n of names) {
    if (new RegExp(`"${n}"`).test(text)) {
      problems.push(`C3b ${name}: 内核非技能文件出现技能中文名（${n}）——文案也属技能分支`);
    }
  }
}

// C5：skills 层不得直接写任何牌区（单出口：flow::gain/exile/to_processing/store_on_general/
//     reveal_top/take_from_deck/to_deck_top/... / gear::place_from_hand/move_equip/swap_zones）。
//     `Zone::Discard` 由 C4 管（判定区移出例外在 C4 白名单）。
const C5_ALLOW = new Map([]);
for (const p of skillFiles) {
  const base = path.basename(p);
  if (C5_ALLOW.has(base)) continue;
  const text = readFileSync(p, "utf8");
  for (const m of text.matchAll(/cards\s*\.\s*insert\([^;]*?Zone::(Equip|Mark|Hand|Void|Processing|Draw)/gs)) {
    problems.push(`C5 skills/${base}: 直接写 Zone::${m[1]}——请走 flow::*/gear::* 单出口`);
  }
  for (const m of text.matchAll(/deck\.(retain|insert|push|remove|pop)\(/g)) {
    problems.push(`C5 skills/${base}: 直接改 \`deck.${m[1]}\`——请走 flow::take_from_deck/reveal_top/to_deck_top/gain_from_deck`);
  }
}

// C6：skills 层「视为使用/转化」不得吞掉错误（legal≡execute：失败必须冒泡）
for (const p of skillFiles) {
  const base = path.basename(p);
  const text = readFileSync(p, "utf8");
  for (const m of text.matchAll(/let _ =[^;]*(use_virtual|use_as|recast_any|place_from_hand)\(/g)) {
    problems.push(`C6 skills/${base}: \`let _ = ${m[1]}(…)\` 吞错——改为 \`?\` 冒泡`);
  }
}

// C7：`turn.`/`phase.`/`round.` 前缀的文本标记必须有清理出口（marks 泄漏）
const turnSrc = readFileSync(path.join(KERNEL, "turn.rs"), "utf8");
for (const prefix of ["turn.", "phase.", "round."]) {
  if (!turnSrc.includes(`clear_marks("${prefix}")`)) {
    problems.push(`C7 turn.rs: 缺少 clear_marks("${prefix}")——${prefix} 标记会跨回合/阶段泄漏`);
  }
}

// ---------------------------------------------------------------------------
// C8/C9（2026-11-16「虚拟牌无属性」裁定后新增）
// ---------------------------------------------------------------------------

// C8：颜色判定必须用 `Suit::is_black`（或 `cards::is_black`），禁止 `!…is_red()`
//     —— 无属性牌（虚拟）`is_red()=false`，取反会把它当成黑色（仁王盾误拦）。
for (const p of skillFiles) {
  const base = path.basename(p);
  const text = readFileSync(p, "utf8");
  for (const m of text.matchAll(/!\s*[\w.()\[\]]*\.is_red\(\)/g)) {
    problems.push(`C8 skills/${base}: 使用了 \`!…is_red()\`——请改 \`.is_black()\`/\`cards::is_black\`（无属性牌不是黑色）`);
  }
}
for (const f of files) {
  const name = rel(path.join(KERNEL, f));
  if (name === "judge.rs" || name === "quick_judge.rs") continue;
  const text = readFileSync(path.join(KERNEL, f), "utf8");
  for (const m of text.matchAll(/!\s*[\w.()\[\]]*\.is_red\(\)/g)) {
    problems.push(`C8 ${name}: 使用了 \`!…is_red()\`——请改 \`.is_black()\`（无属性牌不是黑色）`);
  }
}

// C9：花色等值比较必须走 `cards::suit_is`（无属性 → false），禁止 `x.suit == Suit::…`。
for (const p of skillFiles) {
  const base = path.basename(p);
  const text = readFileSync(p, "utf8");
  for (const m of text.matchAll(/\.suit\s*==\s*(?:super::super::)?Suit::/g)) {
    problems.push(`C9 skills/${base}: 直接比较 \`.suit == Suit::…\`——请用 \`cards::suit_is\`（无属性判否）`);
  }
}

if (problems.length) {
  console.error(`${tag} ❌ 发现 ${problems.length} 处违反：`);
  for (const p of problems) console.error(`- ${p}`);
  process.exit(1);
}
console.log(`${tag} ✅ 通过：内核零技能名分支（code+中文名）/ 零直写体力 / 牌区与视为使用单出口 / marks 作用域；${registered.size} 个已注册 code 已扫描`);
