#!/usr/bin/env node
// 门禁：技能对齐单全局一致性（148 将覆盖 / 字段必填 / 正反例 / 机制引用 / 跨批术语冲突 / 冲突阻断）。
//
// 口径（用户 2026-11-06 裁定）：
//   1. 148 将必须全部落在某个批次的对齐单里（缺批次先按 warning，`--require-all` 转 error）；
//   2. 已 confirmed 的技能：第 7 段（对齐后描述）非空、疑点全部有答案、机制引用合法；
//   3. 已 implemented/verified 的技能：实现锚点 + 用例 + **正例 1 条 + 反例 1 条**；
//   4. 机制词表版本 > 技能确认版本 → 冲突，该批必须置 blocked（阻断开发）；
//   5. 同一术语在不同技能里定义不一致 → 冲突。
//
// 用法：node apps/scripts/check-sgs-align.mjs [--require-all] [--verbose]

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { loadCatalog, projectRoot } from "./lib/sgs-catalog.mjs";
import { BATCHES } from "./lib/sgs-batches.mjs";
import {
  detectTermConflicts,
  listSpecBatches,
  loadMechanics,
  loadSpec,
  validateSpec,
} from "./lib/sgs-align.mjs";

const requireAll = process.argv.includes("--require-all");
const verbose = process.argv.includes("--verbose") || process.argv.includes("-v");

const catalog = loadCatalog();
const mechanics = loadMechanics();
const specBatches = listSpecBatches();
const specs = specBatches.map((b) => loadSpec(b));

const errors = [];
const warnings = [];
const conflicts = [];

// 1. 覆盖
const covered = new Map();
for (const spec of specs) {
  for (const g of spec.generals) {
    if (covered.has(g.name)) {
      errors.push(`武将「${g.name}」同时出现在 ${covered.get(g.name)} 与 ${spec.batch}`);
    } else {
      covered.set(g.name, spec.batch);
    }
  }
}
const missingBatches = BATCHES.filter((b) => !specBatches.includes(b.id));
for (const b of missingBatches) {
  const msg = `批次 ${b.id}（${b.names.length} 将）尚无对齐单`;
  if (requireAll) errors.push(msg);
  else warnings.push(msg);
}
const missingGenerals = catalog.audited.filter((g) => !covered.has(g.name));
if (missingGenerals.length && requireAll) {
  errors.push(`对齐单未覆盖 ${missingGenerals.length} 将：${missingGenerals.map((g) => g.name).join("、")}`);
}

// 2/3/4. 逐批校验
for (const spec of specs) {
  const r = validateSpec(spec, catalog, mechanics);
  errors.push(...r.errors.map((e) => `[${spec.batch}] ${e}`));
  warnings.push(...r.warnings.map((w) => `[${spec.batch}] ${w}`));
  conflicts.push(...r.conflicts);
  if (spec.status === "blocked") {
    errors.push(`[${spec.batch}] 批次处于 blocked（冲突待人工复核），不得继续开发`);
  }
}

// 4.5 裁定 ↔ 用例锚点（M10）：已答疑点要么带 `test`（必须真实存在），要么显式 `legacy: true`。
// 历史 775 条裁定已批量标 legacy（见 align/README）；新裁定必须写 test，防「裁定漂移无人守」。
// 口径：`q.test` 锚点仍校验 v2 用例库（历史裁定以 v2 为语料）；技能级 `impl`/`tests`
// 则按批次 `engine` 分派（v3 批次校验 `sanguosha3` 源码/用例，见 4.6）。
const testsDir = join(projectRoot, "crates/engines/sanguosha2/tests");
const testsText = readdirSync(testsDir)
  .filter((f) => f.endsWith(".rs"))
  .map((f) => readFileSync(join(testsDir, f), "utf8"))
  .join(String.fromCharCode(10));
let anchored = 0;
let legacy = 0;
for (const spec of specs) {
  for (const g of spec.generals ?? []) {
    for (const sk of g.skills ?? []) {
      for (const q of sk.questions ?? []) {
        if (!q.answer) continue;
        if (q.test) {
          const fnName = String(q.test).split("::").pop();
          if (!testsText.includes(`fn ${fnName}(`)) {
            errors.push(`[${spec.batch}] ${sk.code} ${q.id}: 用例 ${q.test} 不存在`);
          } else anchored += 1;
        } else if (q.legacy === true) {
          legacy += 1;
        } else {
          errors.push(
            `[${spec.batch}] ${sk.code} ${q.id}: 已裁定但缺 test 锚点（历史项须显式 legacy: true）`,
          );
        }
      }
    }
  }
}

// 5. 跨批术语
conflicts.push(...detectTermConflicts(specs));

// 5.0 引擎锚点（v3 批次）：`impl`/`tests` 必须真实落在 `sanguosha3` 源码/用例。
// 背景：V3 重写后 align 仍写 v2 锚点（假绿），步骤 B/C 要求「回写 + 门禁」。
const v3SkillsDir = join(projectRoot, "crates/engines/sanguosha3/src/kernel/skills");
const v3TestsDir = join(projectRoot, "crates/engines/sanguosha3/tests");
const v3Cache = new Map();
const v3Source = (file) => {
  if (!v3Cache.has(file)) {
    v3Cache.set(file, existsSync(file) ? readFileSync(file, "utf8") : null);
  }
  return v3Cache.get(file);
};
for (const spec of specs) {
  if ((spec.engine ?? "v2") !== "v3") continue;
  for (const g of spec.generals ?? []) {
    for (const s of g.skills ?? []) {
      if (!s.impl || !String(s.impl).includes("sanguosha3::")) {
        errors.push(`[${spec.batch}] ${s.code}: engine=v3 但 impl 未指向 sanguosha3（${s.impl ?? "-"}）`);
      }
      // impl 里的 `bN::fn` 必须能在 `src/kernel/skills/bN.rs` 找到 `fn fn(`。
      for (const m of String(s.impl ?? "").matchAll(/\b(b\d+)::([a-z_][a-z0-9_]*)/g)) {
        const file = join(v3SkillsDir, `${m[1]}.rs`);
        const src = v3Source(file);
        if (src === null) {
          errors.push(`[${spec.batch}] ${s.code}: impl 模块不存在 src/kernel/skills/${m[1]}.rs（${m[0]}）`);
        } else if (!src.includes(`fn ${m[2]}(`)) {
          errors.push(`[${spec.batch}] ${s.code}: impl 锚点 \`${m[0]}\` 在 ${m[1]}.rs 找不到 \`fn ${m[2]}(\``);
        }
      }
      // tests 的 `bN::fn` 必须在 `sanguosha3/tests/bN.rs` 存在。
      for (const t of s.tests ?? []) {
        const [mod, fn, extra] = String(t).split("::");
        if (!mod || !fn || extra !== undefined || !/^b\d+$/.test(mod)) {
          errors.push(`[${spec.batch}] ${s.code}: 用例锚点格式异常 \`${t}\`（应为 bN::fn）`);
          continue;
        }
        const src = v3Source(join(v3TestsDir, `${mod}.rs`));
        if (src === null) {
          errors.push(`[${spec.batch}] ${s.code}: 用例文件不存在 tests/${mod}.rs（${t}）`);
        } else if (!src.split("\n").some((line) => line.trimStart().startsWith(`fn ${fn}(`))) {
          errors.push(`[${spec.batch}] ${s.code}: 用例锚点 \`${t}\` 在 tests/${mod}.rs 中找不到 \`fn ${fn}(\``);
        }
      }
    }
  }
}

// 5.1 技能 code ↔ 名称撞车（B17 教训：宗室/纵适 曾共用 `zongshi`，导致界简雍纵适不触发）
// 允许：同一 code 在不同武将上对应**同一技能名**（界/标同名复用，ADR-0024）；
// 不允许：同一 code 对应 ≥2 个不同技能名（必须另起 code）。
const codeNames = new Map();
for (const g of [...catalog.audited, ...catalog.excluded]) {
  for (const s of g.skills ?? []) {
    if (!codeNames.has(s.code)) codeNames.set(s.code, new Map());
    const byName = codeNames.get(s.code);
    if (!byName.has(s.name)) byName.set(s.name, []);
    byName.get(s.name).push(g.name);
  }
}
for (const [code, byName] of codeNames) {
  if (byName.size <= 1) continue;
  const detail = [...byName.entries()]
    .map(([name, generals]) => `${name}(${generals.join("/")})`)
    .join(" vs ");
  errors.push(`技能 code 撞车：\`${code}\` → ${detail}（不同技能必须另起 code，见 ADR-0024）`);
}

if (verbose) {
  const skills = specs.flatMap((s) => s.generals.flatMap((g) => g.skills));
  console.log(
    `[align] 批次 ${specs.length}/${BATCHES.length}，技能 ${skills.length}，` +
      `状态 ${["draft", "discussed", "confirmed", "implemented", "verified", "blocked"]
        .map((st) => `${st}=${skills.filter((s) => s.status === st).length}`)
        .join("/")}`,
  );
}

for (const w of warnings) console.error(`  ! ${w}`);
for (const c of conflicts) console.error(`  ⚡ ${c}`);
for (const e of errors) console.error(`  ✗ ${e}`);

if (conflicts.length) {
  console.error(`[align] 失败：${conflicts.length} 个冲突（需人工复核，相关批次必须 blocked）`);
  process.exit(1);
}
if (errors.length) {
  console.error(`[align] 失败：${errors.length} 个错误`);
  process.exit(1);
}
console.log(
  `[align] OK：批次 ${specs.length}/${BATCHES.length}，覆盖 ${covered.size}/${catalog.audited.length} 将，` +
    `警告 ${warnings.length}；裁定用例锚点 ${anchored} 条（legacy ${legacy} 条待回填）`,
);
