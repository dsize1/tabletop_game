#!/usr/bin/env node
// V2 开发约定门禁（机制/迁移欠账防复发）。
//
// 用法：
//   node apps/scripts/check-sgs2-conventions.mjs            # 校验
//   node apps/scripts/check-sgs2-conventions.mjs --report   # 只报告（不非零退出）
//
// 覆盖（对应 docs/games/sanguosha/engine-self-audit.md）：
//   C1 技能层不得直写体力（必须走 flow::heal/lose_hp/revive_to/clamp_hp）
//   C2 内核不得出现技能名（白名单显式登记，新增即失败）
//   C3 座位计数标记必须进视图投影或在内部白名单
//   C4 全局计数器必须有作用域前缀（turn./phase./round./total.）或白名单
//   C5 「跳过/不弃置」类出口必须用 escape_mandatory 或显式白名单
//   C6 kind 一致性：locked 不得有主动入口；limited/awaken 必须有 used 守卫
//   C7 询问完整性（I1）：内核「不询问」分支必须走 must_ask（只有合法集为空才允许不问）
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(".");
const SK = path.join(ROOT, "crates/engines/sanguosha2/src/skills");
const KERNEL = path.join(ROOT, "crates/engines/sanguosha2/src/kernel");
const VIEW = path.join(ROOT, "crates/engines/sanguosha2/src/view.rs");
const ALLOW = path.join(ROOT, "docs/games/sanguosha/conventions-allowlist.json");
const REPORT = process.argv.includes("--report");
const WRITE = process.argv.includes("--write-allowlist");

const allow = fs.existsSync(ALLOW) ? JSON.parse(fs.readFileSync(ALLOW, "utf8")) : {};
const has = (list, pred) => (allow[list] ?? []).some(pred);

const files = fs.readdirSync(SK).filter((f) => f.endsWith(".rs"));
const src = new Map(files.map((f) => [f, fs.readFileSync(path.join(SK, f), "utf8")]));
const view = fs.readFileSync(VIEW, "utf8");
const visible = new Set(
  [...(/pub const VISIBLE_COUNTER_MARKS[^=]*=\s*&\[([\s\S]*?)\];/.exec(view)?.[1] ?? "").matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]),
);

const violations = [];
const add = (check, file, key, detail) => violations.push({ check, file, key, detail });

// ---- C1 体力直写 ----
for (const [f, text] of src) {
  text.split("\n").forEach((ln, i) => {
    if (/\.hp\s*(=|\+=|-=)/.test(ln) && !/==|!=|<=|>=/.test(ln) && !/^\s*\/\//.test(ln)) {
      add("C1-hp", f, ln.trim(), `第 ${i + 1} 行`);
    }
  });
}

// ---- C2 内核技能名 ----
const codes = new Set();
for (const [, text] of src) {
  for (const m of text.matchAll(/code:\s*"([a-z0-9_]+)"/g)) codes.add(m[1]);
}
for (const f of fs.readdirSync(KERNEL).filter((x) => x.endsWith(".rs"))) {
  fs.readFileSync(path.join(KERNEL, f), "utf8")
    .split("\n")
    .forEach((ln, i) => {
      for (const c of codes) {
        if (ln.includes(`"${c}"`)) add("C2-kernel-skill", f, c, `第 ${i + 1} 行`);
      }
    });
}

// ---- C3 可见标记 / C4 计数器 ----
for (const [f, text] of src) {
  text.split("\n").forEach((ln, i) => {
    const mark = /(?:marks|counters)\.insert\(\s*"([a-z0-9_]+)"/.exec(ln);
    if (mark && !visible.has(mark[1])) add("C3-mark", f, mark[1], `第 ${i + 1} 行`);
    const counter = /g\.(?:counters\.insert|bump|counter)\(\s*"([a-z0-9_]+)"/.exec(ln);
    if (counter && !/^(turn|phase|round|total)\./.test(counter[1])) {
      add("C4-counter", f, counter[1], `第 ${i + 1} 行`);
    }
  });
}

// ---- C5 可跳过出口 ----
for (const [f, text] of src) {
  text.split("\n").forEach((ln, i) => {
    const m = /\.(escape|escape_with)\(\s*"([^"]*(?:跳过|不弃置)[^"]*)"/.exec(ln);
    if (m) add("C5-escape", f, m[2], `第 ${i + 1} 行`);
  });
}

// ---- C7 询问完整性（I1：只有合法动作集为空才允许不询问） ----
// v1 口径（用户 2026-10-28 裁定）：无手牌 → 跳过；有手牌 → 即使没有对应牌也显式询问。
// 内核里任何「没有可选项就不问」的分支必须由 `g.must_ask(seat, has)` 统一判定。
const NL = String.fromCharCode(10);
for (const f of ["card_play.rs", "flow.rs"]) {
  const p = path.join(KERNEL, f);
  if (!fs.existsSync(p)) continue;
  const lines = fs.readFileSync(p, "utf8").split(NL);
  lines.forEach((ln, i) => {
    const skip = /^\s*if\s+(?:!has|options\.is_empty\(\))\s*\{/.test(ln);
    if (!skip) return;
    const window = lines.slice(Math.max(0, i - 6), i + 1).join(NL);
    if (!window.includes("must_ask")) add("C7-ask", f, ln.trim(), `第 ${i + 1} 行`);
  });
}

// ---- C6 kind 一致性 ----
for (const [f, text] of src) {
  for (const m of text.matchAll(/SkillDef \{([\s\S]*?)\};/g)) {
    const b = m[1];
    const code = /code:\s*"([a-z0-9_]+)"/.exec(b)?.[1];
    const kind = /kind:\s*SkillKind::(\w+)/.exec(b)?.[1];
    if (!code) continue;
    if (kind === "Locked" && /can_active:\s*Some/.test(b)) add("C6-kind", f, code, "锁定技不得有主动入口");
    if (kind === "Limited" || kind === "Awaken") {
      if (!text.includes(`"${code}_used"`) && !text.includes("_used")) {
        add("C6-kind", f, code, `${kind} 缺 used 守卫`);
      }
    }
  }
}

if (WRITE) {
  const reasons = {
    "C2-kernel-skill": "V2.2（S6）待抽通用钩子：内核按技能名读取数值/窗口/接力（见 engine-self-audit B4）",
    "C3-mark": "内部标记（不面向 UI）：思蜀指定 / 烈弓内部 / 醇醪计数（UI 只播报数量）",
    "C4-counter": "跨回合/整局累计计数器：思蜀指定、醇醪轮次、诫训累计（需显式作用域 registry）",
    "C5-escape": "候选为空/技能明文可选时的放弃出口（评审保留；空选项自动跳过待 M9 落地）",
    "C7-ask": "历史「没有可选项就不询问」分支（已由 must_ask 收口；新增即失败）",
    "C6-kind": "kind 一致性豁免",
  };
  const out = { generatedAt: new Date().toISOString(), note: "显式白名单：新增违反必须补机制或登记理由（engine-self-audit B4/M8/M9）" };
  const buckets = { kernelLiterals: [], internalMarks: [], internalCounters: [], escapes: [], kindWaivers: [] };
  for (const v of violations) {
    const base = { reason: reasons[v.check] };
    if (v.check === "C2-kernel-skill") buckets.kernelLiterals.push({ ...base, file: v.file, code: v.key });
    else if (v.check === "C3-mark") buckets.internalMarks.push({ ...base, key: v.key });
    else if (v.check === "C4-counter") buckets.internalCounters.push({ ...base, key: v.key });
    else if (v.check === "C5-escape") buckets.escapes.push({ ...base, file: v.file, label: v.key });
    else buckets.kindWaivers.push({ ...base, file: v.file, code: v.key });
  }
  const uniq = (arr, k) => [...new Map(arr.map((x) => [k(x), x])).values()];
  out.kernelLiterals = uniq(buckets.kernelLiterals, (x) => x.file + x.code);
  out.internalMarks = uniq(buckets.internalMarks, (x) => x.key);
  out.internalCounters = uniq(buckets.internalCounters, (x) => x.key);
  out.escapes = uniq(buckets.escapes, (x) => x.file + x.label);
  out.kindWaivers = uniq(buckets.kindWaivers, (x) => x.file + x.code);
  fs.writeFileSync(ALLOW, `${JSON.stringify(out, null, 2)}
`);
  console.log(`[sgs2-conventions] 已写入白名单 ${path.relative(ROOT, ALLOW)}（${violations.length} 条）`);
  process.exit(0);
}

const allowed = (v) =>
  has(v.check === "C2-kernel-skill" ? "kernelLiterals" : "", (e) => e.file === v.file && e.code === v.key) ||
  has(v.check === "C3-mark" ? "internalMarks" : "", (e) => e.key === v.key) ||
  has(v.check === "C4-counter" ? "internalCounters" : "", (e) => e.key === v.key) ||
  has(v.check === "C5-escape" ? "escapes" : "", (e) => e.file === v.file && e.label === v.key) ||
  has(v.check === "C6-kind" ? "kindWaivers" : "", (e) => e.file === v.file && e.code === v.key);

const bad = violations.filter((v) => !allowed(v));
const byCheck = {};
for (const v of violations) byCheck[v.check] = (byCheck[v.check] ?? 0) + 1;
console.log("[sgs2-conventions] 命中：", byCheck, `· 白名单内 ${violations.length - bad.length}`);
for (const v of bad.slice(0, 40)) console.log(`  ✗ ${v.check} ${v.file} ${v.key} ${v.detail}`);
if (bad.length > 0) {
  console.log(`[sgs2-conventions] FAIL：${bad.length} 处未登记（新增约定违反请补机制或显式登记白名单+理由）`);
  process.exit(REPORT ? 0 : 1);
}
console.log("[sgs2-conventions] OK");
