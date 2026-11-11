#!/usr/bin/env node
// v2 视图字段对齐门禁（S0）：v2 投影必须覆盖 Web 端消费的字段。
//
// 背景（OQ2=A）：v2 接管 `game_id=sanguosha` 时，Web 三分区页不改 schema，由 v2 `view.rs`
// 输出 v1 兼容字段。本脚本把「前端字段不静默消失」变成机检：
//   - 契约清单：docs/games/sanguosha/engine-replacement/view-required.json（来源 types.ts + 实际读取点）
//   - 样本来源：cargo run -p sanguosha2 --example sgs2-view-dump -- --json
//
// 退出码：
//   默认（S0–S3）：缺失只报告，exit 0（迁移期允许）。
//   --strict（S4 起由 sgs:check:fast 启用）：任何缺失 → exit 1。
//
// 用法：
//   node apps/scripts/check-sgs2-view-parity.mjs [--strict] [--players 5] [--verbose]

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const args = process.argv.slice(2);
const strict = args.includes("--strict");
const verbose = args.includes("--verbose");
const players = (() => {
  const i = args.indexOf("--players");
  return i >= 0 ? args[i + 1] : "5";
})();

const MANIFEST = path.join(
  projectRoot,
  "docs/games/sanguosha/engine-replacement/view-required.json",
);

const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));

// ---------------------------------------------------------------------------
// 取样：跑 v2 example（确定性 seed）
// ---------------------------------------------------------------------------

const run = spawnSync(
  "cargo",
  [
    "run",
    "-q",
    "--manifest-path",
    "crates/Cargo.toml",
    "-p",
    "sanguosha2",
    "--example",
    "sgs2-view-dump",
    "--",
    "--json",
    "--players",
    players,
  ],
  { cwd: projectRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
if (run.status !== 0) {
  console.error("[view-parity] 取样失败（cargo run）：");
  console.error((run.stderr ?? "").split("\n").slice(-20).join("\n"));
  process.exit(1);
}
// stdio 可能被 cargo 的 warning 污染：取最后一行 JSON。
const line = (run.stdout ?? "")
  .split("\n")
  .reverse()
  .find((l) => l.trim().startsWith("{"));
if (!line) {
  console.error("[view-parity] 取样失败：stdout 没有 JSON 行");
  process.exit(1);
}
const sample = JSON.parse(line);

// ---------------------------------------------------------------------------
// 路径解析：支持 `a.b` / `a[].b`
// ---------------------------------------------------------------------------

/** 返回 { found: boolean, kind: "value" | "array-empty" | "missing" }。 */
function probe(root, pathExpr) {
  const segments = pathExpr.split(".");
  let current = root;
  for (let i = 0; i < segments.length; i += 1) {
    const seg = segments[i];
    const isArray = seg.endsWith("[]");
    const key = isArray ? seg.slice(0, -2) : seg;

    if (current === null || typeof current !== "object") return { found: false, why: `父级非对象（${key}）` };
    if (!(key in current)) return { found: false, why: `缺少 key ${key}` };
    current = current[key];

    if (isArray) {
      if (!Array.isArray(current)) return { found: false, why: `${key} 不是数组` };
      const rest = segments.slice(i + 1).join(".");
      if (!rest) return { found: true, empty: current.length === 0 };
      if (current.length === 0) return { found: false, why: `${key} 为空数组，无法验证 ${rest}` };
      // 只验证首元素（同一投影函数生成，元素结构一致）。
      const inner = probe(current[0], rest);
      return inner;
    }
  }
  return { found: true };
}

const missing = [];
const emptyArrays = [];
for (const item of manifest.required ?? []) {
  const hit = probe(sample.public, item.path);
  if (!hit.found) {
    missing.push({ path: item.path, note: item.note, why: hit.why });
  } else if (hit.empty) {
    emptyArrays.push({ path: item.path, note: item.note });
  }
}

// `player_area` 字段用个人视图取样（公共视图里恒为 null / 空）：
// 先在「已开局」样本里找，找不到再到「选将阶段」样本里找（setup 字段只在该阶段存在）。
const stillMissing = [];
for (const m of missing) {
  if (!m.path.startsWith("player_area")) {
    stillMissing.push(m);
    continue;
  }
  const rel = m.path.slice("player_area.".length);
  const hit = probe(sample.player.player_area, rel);
  if (hit.found) continue;
  const setupSample = sample.player_setup ?? { player_area: null };
  const hit2 = probe(setupSample.player_area, rel);
  if (!hit2.found) stillMissing.push({ ...m, why: hit2.why });
}

// ---------------------------------------------------------------------------
// 汇总
// ---------------------------------------------------------------------------

const total = (manifest.required ?? []).length;
console.log(
  `[view-parity] 契约字段 ${total} / 缺失 ${stillMissing.length}（strict=${strict}，players=${players}）`,
);
if (emptyArrays.length && verbose) {
  for (const e of emptyArrays) console.log(`  · 空数组（新建局正常）：${e.path}`);
}
if (stillMissing.length) {
  for (const m of stillMissing) {
    console.log(`  ✗ ${m.path} — ${m.note ?? ""}（${m.why ?? "缺失"}）`);
  }
  if (strict) {
    console.error(`[view-parity] 未通过：缺 ${stillMissing.length} 个前端字段（S4 必须补齐）`);
    process.exit(1);
  }
  console.log("[view-parity] 迁移期：缺失已登记（S4 补齐后由 --strict 强制）");
} else {
  console.log("[view-parity] OK：v2 视图覆盖全部前端契约字段");
}
