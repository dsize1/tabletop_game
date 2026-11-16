#!/usr/bin/env node
// V3 技能注册表门禁：`align/B*.json`（权威语义）↔ `kernel/skills/bN.rs`（注册）
// ↔ `ENABLED_SKILLS`（已收口可挂载）↔ `tests/*.rs`（用例锚点）。
//
// 背景（2026-11-16 审计）：`check:sgs3`（cargo test + 数据生成 + 内核机制锚点）**不校验
// 「本批每个技能都注册/收口/有用例」**。B10 的 `gongxin` 因此注册了却没进 `ENABLED_SKILLS`
// 而长期无人发现（形状绿 ≠ 批次完整）。本门禁把该口径固化。
//
// 规则：
// 1. 已开工批次（`V3_STARTED_BATCHES`，必须从 B0 连续）里，`align/B*.json` 的**每个**技能 code 必须：
//    ① 在 `kernel/skills/bN.rs` 注册；② 在 `ENABLED_SKILLS`；③ 在 `tests/` 出现（用例锚点）。
// 2. 注册表 code 不得重复；注册表内的 code 必须属于某个 align 批次，或声明为 `grant_only`。
// 3. 开批即受约束：新批次开工时把批次 id 加进 `V3_STARTED_BATCHES`（**开工即全绿或不开工**）。
//
// 用法：`node apps/scripts/check-sgs3-registry.mjs`（`npm run check:sgs3-registry`）。

import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const SGS3 = path.join(root, "crates/engines/sanguosha3");
const SKILLS_DIR = path.join(SGS3, "src/kernel/skills");
const TESTS_DIR = path.join(SGS3, "tests");
const ALIGN_DIR = path.join(root, "docs/games/sanguosha/align");

/** 已开工批次（连续，从 B0 起）。新批次开工时追加；**不得**跳过。 */
const V3_STARTED_BATCHES = [
  "B0", "B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9", "B10", "B11", "B12", "B13", "B14", "B15", "B16", "B17", "B18",
];

/**
 * **进行中批次**（只允许紧接在已开工批次之后的若干批）：允许「已注册一部分技能」，
 * 缺口以 `⏳` 计数报告（不静默）：已注册的技能仍必须「已收口 + 有用例」。
 * 全部注册完成后，把该批从本表移到 `V3_STARTED_BATCHES`（此后按完整批次校验）。
 */
const V3_IN_PROGRESS_BATCHES = ["B19"];

const tag = "[check:sgs3-registry]";
const problems = [];

// ---------------------------------------------------------------------------
// 1) 读 V3 注册表（按批次文件）
// ---------------------------------------------------------------------------

const batchFiles = readdirSync(SKILLS_DIR)
  .filter((f) => /^b\d+\.rs$/.test(f))
  .sort((a, b) => Number(a.match(/\d+/)[0]) - Number(b.match(/\d+/)[0]));

/** code → { batch, grantOnly } */
const registry = new Map();
/** batch 文件号 → codes */
const byBatchFile = new Map();

for (const file of batchFiles) {
  const text = readFileSync(path.join(SKILLS_DIR, file), "utf8");
  const batchNo = Number(file.match(/\d+/)[0]);
  const codes = [];
  for (const m of text.matchAll(/(?:code:\s*|skill!\(\s*)"([a-z0-9_]+)"/g)) {
    const code = m[1];
    if (codes.includes(code)) continue;
    codes.push(code);
    if (registry.has(code)) {
      problems.push(`技能 code 重复注册：${code}（${registry.get(code).batch} 与 ${file}）`);
      continue;
    }
    // 粗判 grant_only：同一 code 声明块后 ~300 字符内出现 `grant_only: true`。
    const idx = m.index ?? 0;
    const window = text.slice(Math.max(0, idx - 200), idx + 300);
    registry.set(code, { batch: `b${batchNo}`, grantOnly: /grant_only:\s*true/.test(window) });
  }
  byBatchFile.set(`b${batchNo}`, codes);
}

// ---------------------------------------------------------------------------
// 2) 读 ENABLED_SKILLS
// ---------------------------------------------------------------------------

const skillsRs = readFileSync(path.join(SGS3, "src/kernel/skills.rs"), "utf8");
const enabledBlock = skillsRs.split("pub const ENABLED_SKILLS")[1]?.split("];")[0] ?? "";
const enabled = new Set(
  [...enabledBlock.matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]),
);

// ---------------------------------------------------------------------------
// 3) 读用例文本（锚点：用例体里显式出现 `"<code>"`）
// ---------------------------------------------------------------------------

let testText = "";
/** 文件 → 文本（批次用例文件单独校验：`tests/bN.rs`）。 */
const testFiles = new Map();
for (const f of readdirSync(TESTS_DIR)) {
  const p = path.join(TESTS_DIR, f);
  if (!statSync(p).isFile()) continue;
  if (!f.endsWith(".rs")) continue;
  const text = readFileSync(p, "utf8");
  testFiles.set(f, text);
  testText += text;
}

// ---------------------------------------------------------------------------
// 4) 读 align 批次 → 技能 code
// ---------------------------------------------------------------------------

/** batchId → { generals, codes } */
const alignBatches = new Map();
for (const f of readdirSync(ALIGN_DIR)) {
  if (!/^B\d+\.json$/.test(f)) continue;
  const data = JSON.parse(readFileSync(path.join(ALIGN_DIR, f), "utf8"));
  const codes = new Set();
  const generals = [];
  for (const g of data.generals ?? []) {
    generals.push(g.name);
    for (const s of g.skills ?? []) codes.add(s.code);
  }
  alignBatches.set(data.batch ?? f.replace(".json", ""), {
    generals,
    codes: [...codes],
    status: data.status,
  });
}

// ---------------------------------------------------------------------------
// 5) 断言
// ---------------------------------------------------------------------------

// 5.0 进行中批次必须紧接在已开工批次之后
for (let i = 0; i < V3_IN_PROGRESS_BATCHES.length; i += 1) {
  const want = `B${V3_STARTED_BATCHES.length + i}`;
  if (V3_IN_PROGRESS_BATCHES[i] !== want) {
    problems.push(`V3_IN_PROGRESS_BATCHES 必须紧接已开工批次：第 ${i} 项应为 ${want}，实际 ${V3_IN_PROGRESS_BATCHES[i]}`);
  }
}

// 5.1 已开工批次必须连续
for (let i = 0; i < V3_STARTED_BATCHES.length; i += 1) {
  const want = `B${i}`;
  if (V3_STARTED_BATCHES[i] !== want) {
    problems.push(`V3_STARTED_BATCHES 必须从 B0 连续：第 ${i} 项应为 ${want}，实际 ${V3_STARTED_BATCHES[i]}`);
  }
}

// 5.2 每批：align ⊂ 注册表 ∩ 已收口 ∩ 有用例
const coveredCodes = new Set();
for (const batch of V3_STARTED_BATCHES) {
  const align = alignBatches.get(batch);
  if (!align) {
    problems.push(`已开工批次 ${batch} 缺对齐单 docs/games/sanguosha/align/${batch}.json`);
    continue;
  }
  const file = `b${Number(batch.slice(1))}`;
  const registeredInFile = new Set(byBatchFile.get(file) ?? []);
  for (const code of align.codes) {
    coveredCodes.add(code);
    if (!registry.has(code)) {
      problems.push(`${batch} 技能 ${code} 未注册（${file}.rs）`);
      continue;
    }
    if (!registeredInFile.has(code) && !registry.get(code).grantOnly) {
      // 允许跨批复用（界版同名 code 在更早批次），但必须已注册。
      const owner = registry.get(code).batch;
      if (!byBatchFile.has(owner)) problems.push(`${batch} 技能 ${code} 注册归属异常（${owner}）`);
    }
    if (!enabled.has(code)) problems.push(`${batch} 技能 ${code} 未进 ENABLED_SKILLS（未收口/不可挂载）`);
    // 本批用例文件优先（`tests/bN.rs`）；跨批共享 code（界版同名复用）允许任意用例文件。
    const ownTestFile = `b${Number(batch.slice(1))}.rs`;
    const ownText = testFiles.get(ownTestFile) ?? "";
    const shared = registry.has(code) && registry.get(code).batch !== file;
    if (!ownText.includes(`"${code}"`) && !(shared && testText.includes(`"${code}"`))) {
      problems.push(`${batch} 技能 ${code} 无用例锚点（${ownTestFile} 中未出现）`);
    }
  }
}

// 5.2b 进行中批次：已注册的技能必须已收口 + 有用例；未注册的计为缺口（⏳ 报告）
const inProgress = [];
for (const batch of V3_IN_PROGRESS_BATCHES) {
  const align = alignBatches.get(batch);
  if (!align) {
    problems.push(`进行中批次 ${batch} 缺对齐单 docs/games/sanguosha/align/${batch}.json`);
    continue;
  }
  const file = `b${Number(batch.slice(1))}`;
  const done = [];
  const pending = [];
  for (const code of align.codes) {
    coveredCodes.add(code);
    if (!registry.has(code)) {
      pending.push(code);
      continue;
    }
    done.push(code);
    if (!enabled.has(code)) problems.push(`${batch}（进行中）技能 ${code} 已注册但未进 ENABLED_SKILLS（收口后才能注册）`);
    const ownText = testFiles.get(`b${Number(batch.slice(1))}.rs`) ?? "";
    const shared = registry.get(code).batch !== file;
    if (!ownText.includes(`"${code}"`) && !(shared && testText.includes(`"${code}"`))) {
      problems.push(`${batch}（进行中）技能 ${code} 无用例锚点`);
    }
  }
  inProgress.push({ batch, done: done.length, total: align.codes.length, pending });
}

// 5.3 注册表 ↔ align：未覆盖的 code 只能是 grant_only
for (const [code, meta] of registry) {
  if (coveredCodes.has(code)) continue;
  if (!meta.grantOnly) {
    problems.push(`注册表 code ${code} 不在任何已开工批次的 align 单，且未声明 grant_only`);
  }
  if (!enabled.has(code)) {
    problems.push(`授予技 ${code} 已注册但未进 ENABLED_SKILLS（授予后也不会被触发）`);
  }
  if (!testText.includes(`"${code}"`)) problems.push(`授予技 ${code} 无用例锚点`);
}

// 5.4 ENABLED_SKILLS 不得包含未注册 code
for (const code of enabled) {
  if (!registry.has(code)) problems.push(`ENABLED_SKILLS 含未注册技能：${code}`);
}

// ---------------------------------------------------------------------------
// 6) 输出
// ---------------------------------------------------------------------------

const nextBatch = `B${V3_STARTED_BATCHES.length}`;
const nextAlign = alignBatches.get(nextBatch);
const nextInfo = nextAlign
  ? `下一批 ${nextBatch}：${nextAlign.generals.length} 将 / ${nextAlign.codes.length} 技能（未开工）`
  : `无下一批（B0–${V3_STARTED_BATCHES.at(-1)} 已全部开工）`;

if (problems.length) {
  console.error(`❌ ${tag} 失败（${problems.length} 条）：`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}

const total = registry.size;
const enabledCount = enabled.size;
console.log(
  `✅ ${tag}：已开工 ${V3_STARTED_BATCHES[0]}–${V3_STARTED_BATCHES.at(-1)}，` +
    `注册 ${total} / 收口 ${enabledCount}；${nextInfo}`,
);
for (const p of inProgress) {
  console.log(
    `⏳ ${p.batch} 进行中：已注册 ${p.done}/${p.total}` +
      (p.pending.length ? `；待注册：${p.pending.join(", ")}` : "；已全量注册，可移入 V3_STARTED_BATCHES"),
  );
}
