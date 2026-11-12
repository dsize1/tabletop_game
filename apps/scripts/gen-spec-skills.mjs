#!/usr/bin/env node
// 生成技能契约（specs/sanguosha/skills/B*.md）——数据驱动，不手写。
//
// 数据源：
//   - Rust 注册表导出：`cargo run -p sanguosha2 --example sgs2-registry -- --json`
//     （code / name / kind / triggers / window / active / transform / lord_side / dying / flip…）
//   - 对齐单：`docs/games/sanguosha/align/B*.json`（generals[].skills[]：original / current / questions[].test / status）
//
// 用法：
//   node apps/scripts/gen-spec-skills.mjs            # 生成/刷新全部批次
//   node apps/scripts/gen-spec-skills.mjs --batch B0 # 只生成一批
//   node apps/scripts/gen-spec-skills.mjs --check    # 只校验（不写文件），与 --write 后一致
//
// 产物：specs/sanguosha/skills/B0.md … B20.md（每个技能一行表格 + 锚点清单）

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(".");
const ALIGN = path.join(ROOT, "docs", "games", "sanguosha", "align");
const OUT = path.join(ROOT, "specs", "sanguosha", "skills");
const CHECK = process.argv.includes("--check");
const batchArgIdx = process.argv.indexOf("--batch");
const onlyBatch = batchArgIdx >= 0 ? process.argv[batchArgIdx + 1] : null;

function registry() {
  const raw = execFileSync(
    "cargo",
    [
      "run",
      "-q",
      "--manifest-path",
      path.join(ROOT, "crates", "Cargo.toml"),
      "-p",
      "sanguosha2",
      "--example",
      "sgs2-registry",
      "--",
      "--json",
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  const line = raw
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.startsWith("{"))
    .pop();
  if (!line) throw new Error("sgs2-registry 未输出 JSON");
  return JSON.parse(line).skills;
}

/** align 的 `test` 字段形如 `b0::fn_name` → `tests/b0.rs::fn_name`。 */
function anchorOf(test) {
  if (typeof test !== "string" || !test.includes("::")) return null;
  const [file, fn] = test.split("::");
  if (!/^[a-z0-9_]+$/i.test(file)) return null;
  return `tests/${file.toLowerCase()}.rs::${fn}`;
}

function renderBatch(align, regByCode, index) {
  const batch = align.batch;
  const generals = align.generals ?? [];
  const lines = [];
  lines.push(`# ${batch} 技能契约`);
  lines.push("");
  lines.push(`> 由 \`npm run gen:spec-skills -- --batch ${batch}\` 生成（数据源：Rust 注册表 + \`align/${batch}.json\`）。`);
  lines.push("> 契约字段含义见 `specs/sanguosha/skills/README.md`；**人工走查状态见** `docs/games/sanguosha/general-audit.md`。");
  lines.push("");
  lines.push(`- 对齐单状态：\`${align.status ?? "?"}\``);
  lines.push(`- 确认：${align.confirmed_by ?? "-"} / ${align.confirmed_at ?? "-"}`);
  lines.push(`- 武将：${generals.length} 位 / 技能：${generals.reduce((n, g) => n + (g.skills?.length ?? 0), 0)} 个`);
  lines.push("");
  lines.push("| 武将 | 技能 | code | kind | 触发 | 窗口 | 主动 | 对齐单 | 用例锚点 | 状态 |");
  lines.push("|---|---|---|---|---|---|---|---|---|---|");
  for (const g of generals) {
    for (const s of g.skills ?? []) {
      const reg = regByCode.get(s.code) ?? {};
      const triggers = (reg.triggers ?? []).join(" / ") || "—";
      const window = reg.window ?? "—";
      const active = reg.active ? "✔" : "—";
      // 锚点 = 各裁定项的 test ∪ 技能级 test（无裁定项的技能用技能级锚点）
      const anchors = [
        ...new Set([
          ...(s.questions ?? []).map((q) => anchorOf(q.test)).filter(Boolean),
          ...(anchorOf(s.test) ? [anchorOf(s.test)] : []),
        ]),
      ];
      const anchorText = anchors.length === 0 ? "—" : anchors.map((a) => `\`${a}\``).join("<br>");
      // 状态口径：
      //   verified    = 对齐单已裁定 + 有用例锚点
      //   implemented = 对齐单已裁定但**锚点待回填**（M10 legacy 欠账，显式计数）
      //   draft       = 待走查
      const base = s.status ?? "draft";
      const anchorsN = anchors.length;
      const settled = base === "implemented" || base === "verified";
      const status = settled ? (anchorsN > 0 ? "verified" : "implemented") : base;
      const alignRef = `align/${batch}.json#${s.code}（${(s.questions ?? []).length} 裁定）`;
      lines.push(
        `| ${g.name} | ${s.name} | \`${s.code}\` | ${reg.kind ?? s.kind ?? "?"} | ${triggers} | ${window} | ${active} | ${alignRef} | ${anchorText} | ${status} |`,
      );
      index.push({
        code: s.code,
        batch,
        file: `specs/sanguosha/skills/${batch}.md`,
        kind: reg.kind ?? s.kind ?? "",
        status,
        anchors,
      });
    }
  }
  lines.push("");
  lines.push("## 交互面（契约）");
  lines.push("");
  lines.push("每个技能的「原文 / 现实现（逐条）/ 歧义裁定」以对齐单为唯一事实源；本目录只记录可机器校验的部分：");
  lines.push("");
  lines.push("- **触发 / 窗口 / 主动**：来自 `SkillDef`（注册表导出），改动即被 `check:spec-conformance` 发现；");
  lines.push("- **用例锚点**：来自 `align/<batch>.json` 的 `questions[].test`，锚点函数必须存在；");
  lines.push("- **状态**：`implemented` 表示对齐单已裁定并落地；`draft` 表示待走查。");
  lines.push("");
  lines.push("> 交互细节（出口语义 / pending.data / 文案）遵守 `specs/sanguosha/interaction/spec.md` 与 `constitution.md`，");
  lines.push("> 逐技能不重复声明；违反即由 I1 门禁（`contracts/interaction.json`）拦下。");
  lines.push("");
  return lines.join("\n");
}


/** 从测试文件里按 skill code 猜锚点（`#[test]` 函数体里出现 "code" 字符串）。 */
function detectAnchors(codes) {
  const out = new Map();
  const testsDir = path.join(ROOT, "crates", "engines", "sanguosha2", "tests");
  const files = readdirSync(testsDir).filter((f) => f.endsWith(".rs"));
  for (const f of files) {
    const text = readFileSync(path.join(testsDir, f), "utf8");
    // 按 #[test] 切块
    const blocks = text.split(/#\[test\]/).slice(1);
    for (const block of blocks) {
      const m = /^\s*fn\s+([A-Za-z0-9_]+)/.exec(block);
      if (!m) continue;
      for (const code of codes) {
        if (block.includes(`"${code}"`)) {
          const list = out.get(code) ?? [];
          const anchor = `tests/${f}::${m[1]}`;
          if (!list.includes(anchor) && list.length < 3) list.push(anchor);
          out.set(code, list);
        }
      }
    }
  }
  return out;
}

/** 未被 align 覆盖的注册技能（授予技等）单独成表。 */
function renderGranted(codes, regByCode, anchors) {
  const lines = [];
  lines.push("# 授予技 / 无对齐单条目的技能契约");
  lines.push("");
  lines.push("> 由 `npm run gen:spec-skills` 生成。这些技能已注册并实现，但不在 align 的武将技能表里");
  lines.push("> （授予技、变体技能等）；语义以 `docs/games/sanguosha/skills.md` 与授予者的对齐单为准。");
  lines.push("");
  lines.push("| 名称 | code | kind | 触发 | 窗口 | 主动 | 用例锚点 | 状态 |");
  lines.push("|---|---|---|---|---|---|---|---|");
  for (const code of codes.sort()) {
    const r = regByCode.get(code) ?? {};
    const a = anchors.get(code) ?? [];
    lines.push(
      `| ${r.name ?? code} | \`${code}\` | ${r.kind ?? "?"} | ${(r.triggers ?? []).join(" / ") || "—"} | ${r.window ?? "—"} | ${r.active ? "✔" : "—"} | ${
        a.length ? a.map((x) => `\`${x}\``).join("<br>") : "—"
      } | ${a.length ? "implemented" : "draft"} |`,
    );
  }
  lines.push("");
  return lines.join("\n");
}

// ---------------------------------------------------------------------------

const reg = registry();
const regByCode = new Map(reg.map((s) => [s.code, s]));

const alignFiles = readdirSync(ALIGN)
  .filter((f) => /^B[0-9a-z]+\.json$/.test(f))
  .sort((a, b) => {
    const na = parseInt(a.slice(1), 10);
    const nb = parseInt(b.slice(1), 10);
    return na - nb || a.localeCompare(b);
  });

let written = 0;
let stale = 0;
const covered = new Set();
const index = [];
for (const f of alignFiles) {
  const align = JSON.parse(readFileSync(path.join(ALIGN, f), "utf8"));
  const batch = align.batch ?? f.replace(/\.json$/, "");
  if (onlyBatch && batch !== onlyBatch) continue;
  for (const g of align.generals ?? []) {
    for (const s of g.skills ?? []) covered.add(s.code);
  }
  const body = renderBatch(align, regByCode, index);
  const outPath = path.join(OUT, `${batch}.md`);
  if (CHECK) {
    const cur = existsSync(outPath) ? readFileSync(outPath, "utf8") : null;
    if (cur !== body) {
      stale += 1;
      console.error(`[gen:spec-skills] ${batch} 契约文件不是最新（跑 npm run gen:spec-skills）`);
    }
  } else {
    writeFileSync(outPath, body, "utf8");
    written += 1;
  }
}

// 授予技 / 未覆盖技能
const uncovered = reg.filter((s) => !covered.has(s.code)).map((s) => s.code);
const anchors = detectAnchors(new Set([...uncovered, ...[...covered].slice(0, 400)]));
const grantedBody = renderGranted(uncovered, regByCode, anchors);
const grantedPath = path.join(OUT, "granted.md");
if (CHECK) {
  const cur = existsSync(grantedPath) ? readFileSync(grantedPath, "utf8") : null;
  if (cur !== grantedBody) {
    stale += 1;
    console.error("[gen:spec-skills] granted.md 不是最新（跑 npm run gen:spec-skills）");
  }
} else if (!onlyBatch) {
  writeFileSync(grantedPath, grantedBody, "utf8");
}

// granted（授予技）条目也进 index
for (const code of uncovered.sort()) {
  const a = anchors.get(code) ?? [];
  index.push({
    code,
    batch: "granted",
    file: "specs/sanguosha/skills/granted.md",
    kind: regByCode.get(code)?.kind ?? "",
    status: a.length ? "implemented" : "draft",
    anchors: a,
  });
}

const indexPath = path.join(OUT, "index.json");
const indexBody =
  JSON.stringify(
    {
      id: "skill-contracts",
      generated: true,
      note: "由 `npm run gen:spec-skills` 生成；不得手改。reg=技能注册表，覆盖与锚点由 check:spec-conformance 校验。",
      count: index.length,
      skills: index.sort((a, b) => String(a.code).localeCompare(String(b.code))),
    },
    null,
    1,
  ) + String.fromCharCode(10);
if (CHECK) {
  const cur = existsSync(indexPath) ? readFileSync(indexPath, "utf8") : null;
  if (cur !== indexBody) {
    stale += 1;
    console.error("[gen:spec-skills] index.json 不是最新（跑 npm run gen:spec-skills）");
  }
} else if (!onlyBatch) {
  writeFileSync(indexPath, indexBody, "utf8");
}

if (CHECK) {
  if (stale > 0) process.exit(1);
  console.log(`[gen:spec-skills] OK：${alignFiles.length} 批契约 + granted.md 均为最新`);
} else {
  const missing = [];
  console.log(
    `[gen:spec-skills] 已写入 ${written} 个批次文件；注册表 ${reg.length} 个技能中 ` +
      `${reg.length - missing.length} 个已在契约中${missing.length ? `，未覆盖 ${missing.length}：${missing
        .slice(0, 8)
        .map((s) => s.code)
        .join(", ")}${missing.length > 8 ? "…" : ""}` : ""}`,
  );
}
