#!/usr/bin/env node
// 门禁：武将包预设 ↔ `data.rs` ↔ `data-model.md` 一致（P4.4.3 harness）。
//
// 目的：扩包 / 改同名去重 / 改预设时，防止「代码池变了但文档表没改」或「文档写了预设但代码没提供」。
//
// 口径：
//   1. `data.rs`：`GENERAL_PACK_KEYS`（包顺序）+ 全部 `GeneralDef`（pack/name）；
//   2. 预设 = `standard` + 其余包的全部子集（2^(N-1)），按包数升序（与 `lib.rs::general_pack_presets` 同序）；
//   3. 可用人数 = 包过滤 → 同名去重（`prefer_jiexian`：与「界化」武将同名的原版剔除）；
//   4. 文档侧：`data-model.md` §3.7「武将包口径」预设表（mode / 标签 / 人数）；
//   5. 附带断言：`lib.rs` 大厅循环真的用 `general_pack_presets()`、`Config` 默认仍为 `["standard"]`、
//      `legal-actions.md` 登记了 `lobby:set_general_packs:<mode>`。
//
// 用法：node apps/scripts/check-sgs-pack-presets.mjs [--verbose]

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const DATA_RS = resolve("crates", "engines", "sanguosha", "src", "data.rs");
const LIB_RS = resolve("crates", "engines", "sanguosha", "src", "lib.rs");
const STATE_RS = resolve("crates", "engines", "sanguosha", "src", "state.rs");
const DATA_MODEL_MD = resolve("docs", "games", "sanguosha", "data-model.md");
const LEGAL_ACTIONS_MD = resolve("docs", "games", "sanguosha", "legal-actions.md");

const verbose = process.argv.includes("--verbose") || process.argv.includes("-v");

const PACK_LABELS = {
  wind_fire_forest_mountain: "风林火山",
  yijiang: "一将成名",
  jiexian: "界限突破",
};

const problems = [];
function fail(msg) {
  problems.push(msg);
}
function info(msg) {
  if (verbose) console.log(`[pack-presets] ${msg}`);
}

/** 解析 `GENERAL_PACK_KEYS`（顺序即预设归一化顺序）。 */
function parsePackKeys(text) {
  const block = /pub static GENERAL_PACK_KEYS[^=]*=\s*&\[([\s\S]*?)\];/.exec(text)?.[1];
  if (!block) throw new Error("data.rs 未找到 GENERAL_PACK_KEYS");
  return [...block.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);
}

/** 解析全部 `GeneralDef`（code/gid/name/pack）。 */
function parseGenerals(text) {
  const re = /code: "(sgs_hero_\d+)", gid: \d+, name: "([^"]+)", pack: "([a-z_]+)"/g;
  return [...text.matchAll(re)].map((m) => ({ code: m[1], name: m[2], pack: m[3] }));
}

/** 预设枚举（与 `lib.rs::general_pack_presets` 同序：先掩码生成，再按包数稳定排序）。 */
function presetsOf(keys) {
  const others = keys.filter((k) => k !== "standard");
  const out = [];
  for (let mask = 0; mask < 1 << others.length; mask++) {
    const combo = ["standard"];
    others.forEach((p, i) => {
      if (mask & (1 << i)) combo.push(p);
    });
    out.push(combo.join("+"));
  }
  return out.sort((a, b) => a.split("+").length - b.split("+").length);
}

/** 选将池 = 包过滤 → 同名去重（界优先）。 */
function poolOf(generals, mode) {
  const packs = new Set(mode.split("+"));
  const codes = generals.filter((g) => packs.has(g.pack));
  const jieNames = new Set(
    codes.filter((g) => g.pack === "jiexian").map((g) => g.name.replace(/^界/, "")),
  );
  return codes.filter((g) => g.pack === "jiexian" || !jieNames.has(g.name));
}

function labelOf(mode) {
  return mode
    .split("+")
    .map((p) => (p === "standard" ? "标准" : (PACK_LABELS[p] ?? p)))
    .join(" + ");
}

/** 从 data-model.md §3.7 抽取预设表行。 */
function parseDocRows(text) {
  const start = text.indexOf("预设（`lobby:set_general_packs:<mode>`）");
  if (start < 0) throw new Error("data-model.md 未找到「预设（lobby:set_general_packs:<mode>）」表");
  const rest = text.slice(start);
  const end = rest.indexOf("\n- 同名去重");
  if (end < 0) throw new Error("data-model.md 预设表后缺少「- 同名去重」段落");
  const rows = [];
  for (const m of rest.slice(0, end).matchAll(/^\| `([^`]+)` \| ([^|]+?) \| (\d+) \|$/gm)) {
    // 文档允许给全量组合加注「（全量）」（仅标注，不参与标签比对）。
    rows.push({ mode: m[1], label: m[2].trim().replace(/（全量）$/, ""), count: Number(m[3]) });
  }
  return rows;
}

const [dataRs, libRs, stateRs, dataModel, legalActions] = await Promise.all([
  readFile(DATA_RS, "utf8"),
  readFile(LIB_RS, "utf8"),
  readFile(STATE_RS, "utf8"),
  readFile(DATA_MODEL_MD, "utf8"),
  readFile(LEGAL_ACTIONS_MD, "utf8"),
]);

const keys = parsePackKeys(dataRs);
const generals = parseGenerals(dataRs);
if (generals.length === 0) throw new Error("data.rs 未解析到 GeneralDef");

// —— 代码侧 ——
if (!libRs.includes("general_pack_presets()")) {
  fail("lib.rs 大厅未使用 general_pack_presets()（预设枚举缺失）");
}
if (!/pub fn default_general_packs\(\) -> Vec<String> \{\s*vec!\["standard"\.to_string\(\)\]/.test(stateRs)) {
  fail("state.rs：default_general_packs 应保持仅标准包（P4.4.3 口径）");
}
if (!legalActions.includes("`lobby:set_general_packs:<mode>`")) {
  fail("legal-actions.md §3 未登记 `lobby:set_general_packs:<mode>`");
}

const presets = presetsOf(keys);
const expected = presets.map((mode) => ({
  mode,
  label: labelOf(mode),
  count: poolOf(generals, mode).length,
}));

// —— 文档侧 ——
const docRows = parseDocRows(dataModel);
if (docRows.length !== expected.length) {
  fail(`data-model.md 预设表行数 ${docRows.length} ≠ 代码预设数 ${expected.length}`);
}
for (const want of expected) {
  const row = docRows.find((r) => r.mode === want.mode);
  if (!row) {
    fail(`data-model.md 预设表缺少 \`${want.mode}\``);
    continue;
  }
  if (row.label !== want.label) {
    fail(`预设 \`${want.mode}\` 标签不一致：文档「${row.label}」≠ 代码「${want.label}」`);
  }
  if (row.count !== want.count) {
    fail(`预设 \`${want.mode}\` 人数不一致：文档 ${row.count} ≠ 池 ${want.count}`);
  }
  info(`${want.mode} → ${want.label}（${want.count} 位）`);
}
const extra = docRows.filter((r) => !expected.some((e) => e.mode === r.mode));
for (const row of extra) fail(`data-model.md 预设表多出 \`${row.mode}\`（代码无此预设）`);

// —— 汇总 ——
console.log(
  `[check] 武将包：${keys.join(" → ")}；武将 ${generals.length} 位；预设 ${expected.length} 个`,
);
if (problems.length) {
  console.error(`[check] ❌ 武将包预设不一致（${problems.length} 项）：`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log(
  `[check] OK：预设 ${expected.map((e) => `${e.mode}=${e.count}`).join(" / ")} 与 data-model.md 一致`,
);
