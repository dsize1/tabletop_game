#!/usr/bin/env node
// v1 耦合防漏改门禁（S0，切换期临时门禁；S6 删除 v1 后退场）。
//
// 治的病（plan §19.2 M5/M6 高频踩点）：把 v2 注册进 MCP / server / 网关时，
// 「注册表换了，但 server 的 AI 驱动 / 预览沙盒 / 中台同步 / 网关座位读取还调 v1」，
// 表现为线上半新半旧（页面卡死、机器人不动、`#加机器人` 回执 0 人）。
//
// 本脚本把「v1 引用」变成清单：
//   - Rust：`crates/server/src`、`crates/game-engine-mcp/src` 出现 `sanguosha::`（v1 crate 路径）
//   - Rust（v2 自身）：`crates/engines/sanguosha2/{src,tests,examples}` 不得出现 v1 引用
//   - TS：`apps/agent-gateway/src` 出现 v1 原始 state 座位读取（`data?.seats` / `d.seats` 等）
//
// 白名单：docs/games/sanguosha/engine-replacement/coupling-whitelist.json
//   - 命中白名单 → 允许（reason + until 必填）
//   - 白名单条目不再命中 → **失败**（强制随手删除，防白名单腐烂）
//
// 用法：node apps/scripts/check-sgs2-coupling.mjs [--verbose]
// 退出码：0 通过；1 有未登记耦合 / 白名单过期 / 白名单缺字段。

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const verbose = process.argv.includes("--verbose");
const WHITELIST = path.join(
  projectRoot,
  "docs/games/sanguosha/engine-replacement/coupling-whitelist.json",
);

const DEFAULT_SCAN = {
  rust: {
    roots: ["crates/server/src", "crates/game-engine-mcp/src"],
    patterns: ["sanguosha::"],
  },
  v2_self: {
    roots: ["crates/engines/sanguosha2/src", "crates/engines/sanguosha2/tests", "crates/engines/sanguosha2/examples"],
    patterns: ["sanguosha::", "use sanguosha;"],
  },
  ts: {
    roots: ["apps/agent-gateway/src"],
    patterns: ["data?.seats", "d.seats", "st.data.seats", "public_board?.seats"],
  },
};

const wl = existsSync(WHITELIST)
  ? JSON.parse(readFileSync(WHITELIST, "utf8"))
  : { version: 0, rules: [], scan: {} };
const scan = { ...DEFAULT_SCAN, ...(wl.scan ?? {}) };

const problems = [];
const fileHits = new Map(); // "file|pattern" -> count

function walk(dirAbs) {
  if (!existsSync(dirAbs)) return [];
  const out = [];
  for (const name of readdirSync(dirAbs)) {
    const abs = path.join(dirAbs, name);
    const st = statSync(abs);
    if (st.isDirectory()) out.push(...walk(abs));
    else out.push(abs);
  }
  return out;
}

const rel = (abs) => path.relative(projectRoot, abs).split(path.sep).join("/");

function scanGroup(groupName, cfg) {
  for (const root of cfg.roots) {
    for (const abs of walk(path.join(projectRoot, root))) {
      if (!/\.(rs|ts|mjs)$/.test(abs)) continue;
      const lines = readFileSync(abs, "utf8").split("\n");
      for (const pattern of cfg.patterns) {
        let count = 0;
        const samples = [];
        for (const [i, line] of lines.entries()) {
          if (line.includes(pattern)) {
            count += 1;
            if (samples.length < 3) samples.push(`${i + 1}: ${line.trim().slice(0, 110)}`);
          }
        }
        if (count > 0) {
          const key = `${rel(abs)}|${pattern}`;
          fileHits.set(key, { group: groupName, file: rel(abs), pattern, count, samples });
        }
      }
    }
  }
}

for (const [name, cfg] of Object.entries(scan)) scanGroup(name, cfg);

// ---------------------------------------------------------------------------
// 白名单匹配
// ---------------------------------------------------------------------------

const rules = wl.rules ?? [];
const matchedRuleIds = new Set();

for (const [key, hit] of fileHits) {
  const rule = rules.find(
    (r) => r.file === hit.file && (r.patterns ?? []).some((p) => p === hit.pattern),
  );
  if (rule) {
    if (!rule.reason || !rule.until) {
      problems.push(`白名单规则 ${rule.id ?? "?"} 缺 reason/until（不允许无理由豁免）`);
    }
    matchedRuleIds.add(rule.id ?? `${rule.file}|${hit.pattern}`);
    if (verbose) {
      console.log(`  · 已登记：${hit.file} × ${hit.pattern}（${hit.count} 处，${rule.until}）`);
    }
    continue;
  }
  const sample = hit.samples.map((s) => `      ${s}`).join("\n");
  problems.push(
    `未登记 v1 耦合：${hit.file} × ${hit.pattern}（${hit.count} 处）\n${sample}\n    → 迁移到 v2，或在 coupling-whitelist.json 登记（reason + until）`,
  );
}

// 白名单腐烂检查：登记了但不再命中 → 必须删掉。
for (const rule of rules) {
  const hitCount = [...fileHits.values()].filter(
    (h) => h.file === rule.file && (rule.patterns ?? []).includes(h.pattern),
  ).length;
  if (hitCount === 0) {
    problems.push(
      `白名单过期：${rule.id ?? rule.file} 已无命中（${rule.until ?? "?"} 完成？）→ 请删除该规则`,
    );
  }
}

// ---------------------------------------------------------------------------
// 汇总
// ---------------------------------------------------------------------------

const totalHits = [...fileHits.values()].reduce((n, h) => n + h.count, 0);
const files = new Set([...fileHits.values()].map((h) => h.file));
console.log(
  `[coupling] 扫描 v1 引用 ${files.size} 个文件 / ${totalHits} 处；白名单 ${rules.length} 条`,
);
if (verbose) {
  for (const hit of fileHits.values()) {
    if (!hit.samples.length) continue;
    console.log(`  · ${hit.file}（${hit.pattern} × ${hit.count}）`);
    for (const s of hit.samples) console.log(`      ${s}`);
  }
}
if (problems.length) {
  console.error(`[coupling] 未通过（${problems.length}）：`);
  for (const p of problems) console.error(`  ✗ ${p}`);
  process.exit(1);
}
console.log("[coupling] OK：所有 v1 引用均已登记（迁移期白名单）");
