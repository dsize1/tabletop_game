#!/usr/bin/env node
// 三国杀批次开工脚手架（P4.4.2 harness）。
//
// 从 `data.rs` + `development-plan.md` 生成本批 8 将的开工清单与可粘贴片段：
// - 武将元数据（gid/名称/势力/体力/技能名）
// - `skills.rs` 常量 + match 分支模板（code 用占位符，需手工填拼音）
// - `skills.md` 表格行模板
// - 用例骨架（tests/pNN.rs）与 p67 计数提示
//
// 用法：
//   node apps/scripts/sgs-batch-scaffold.mjs                 # 自动取「下一批」
//   node apps/scripts/sgs-batch-scaffold.mjs --batch Y4
//   node apps/scripts/sgs-batch-scaffold.mjs --gids 187,188,321
//   node apps/scripts/sgs-batch-scaffold.mjs --module resolve_yijiang5 --tests p91

import { batchReport, loadGenerals } from "./sgs-data-lib.mjs";

const args = process.argv.slice(2);
function opt(name, fallback = null) {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback;
}

const generals = loadGenerals();
const { report, next } = batchReport();

let batch = opt("batch");
let gids = opt("gids");
let entries;
if (gids) {
  entries = gids
    .split(",")
    .map((g) => Number(g.trim()))
    .filter((n) => Number.isInteger(n))
    .map((gid) => ({ gid, name: generals.get(gid)?.name ?? "?" }));
} else {
  const row = batch ? report.find((b) => b.batch === batch) : next;
  if (!row) {
    console.error(
      batch
        ? `[scaffold] 批次表里找不到 ${batch}`
        : "[scaffold] 一将成名批次已全部完成，用 --batch/--gids 指定其他包",
    );
    process.exit(1);
  }
  batch = row.batch;
  entries = row.entries;
}

const moduleName = opt("module", "resolve_yijiangN");
const testsFile = opt("tests", "pNN");

const totalSkills = entries.reduce(
  (n, e) => n + (generals.get(e.gid)?.skills.length ?? 0),
  0,
);
console.log(
  `# 批次 ${batch ?? ""} 开工脚手架（${entries.length} 将 / ${totalSkills} 技能）`,
);
console.log("");
console.log("## 1. 武将数据（data.rs）");
for (const e of entries) {
  const g = generals.get(e.gid);
  if (!g) {
    console.log(`- ${e.gid} ${e.name}：data.rs 未找到（检查生成器）`);
    continue;
  }
  const skills = g.skills.length ? g.skills.join(" / ") : "（无技能，检查数据）";
  console.log(
    `- ${g.gid} ${g.name}（${g.code}，${g.kingdom}，${g.max_hp} 体力，${g.gender}，pack=${g.pack}）：${skills}`,
  );
}
console.log("");
console.log("## 2. skills.rs 片段（code 按拼音手工填写）");
console.log("");
console.log("```rust");
console.log(
  `// ---- P4.4.2 一将成名（批次 ${batch ?? ""}：${entries.length} 将 / ${totalSkills} 技能）----`,
);
for (const e of entries) {
  const g = generals.get(e.gid);
  if (!g) continue;
  const constNameFor = `G${g.gid}`;
  if (g.skills.length === 1) {
    console.log(`const ${constNameFor}: &[SkillDef] = &[SkillDef {`);
    console.log(`    code: "TODO_${g.gid}",  // ${g.skills[0]}`);
    console.log(`    name: "${g.skills[0]}",`);
    console.log(`    kind: SkillType::Normal,`);
    console.log(`    implemented: true,`);
    console.log(`}];`);
  } else {
    console.log(`const ${constNameFor}: &[SkillDef] = &[`);
    for (const [i, s] of g.skills.entries()) {
      console.log(`    SkillDef {`);
      console.log(`        code: "TODO_${g.gid}_${i + 1}",  // ${s}`);
      console.log(`        name: "${s}",`);
      console.log(`        kind: SkillType::Normal,`);
      console.log(`        implemented: true,`);
      console.log(`    },`);
    }
    console.log(`];`);
  }
  console.log("");
}
console.log("// match general {");
for (const e of entries) {
  console.log(`    "sgs_hero_${e.gid}" => G${e.gid},`);
}
console.log("// }");
console.log("```");
console.log("");
console.log("## 3. skills.md 表格行模板");
console.log("");
console.log("```md");
for (const e of entries) {
  const g = generals.get(e.gid);
  if (!g) continue;
  for (const [i, skill] of g.skills.entries()) {
    const code = g.skills.length > 1 ? `TODO_${g.gid}_${i + 1}` : `TODO_${g.gid}`;
    console.log(
      `| ${g.name} \`${g.code}\` | ${skill} | \`${code}\` | 普通 | 实现要点（自动/可选口径写清楚） |`,
    );
  }
}
console.log("```");
console.log("");
console.log("## 4. 用例 / 文档 / 门禁");
console.log("");
console.log(`- 用例文件：\`crates/engines/sanguosha/src/tests/${testsFile}.rs\`（每将 ≥3 条，复用 \`tests/support.rs\`）`);
console.log(`- 实现模块：\`crates/engines/sanguosha/src/${moduleName}.rs\` + \`resolve.rs\` 三处注册（Frame / step / act）`);
console.log(`- p67：\`tests/p67.rs\` 的「一将成名已实现 N 位」+${entries.length}（\`npm run sgs:batch-status --check\` 校验）`);
console.log("- 文档：`skills.md` 新 §2.7.x + `rules-rulings.md` 新一节 + `development-plan.md` 批次记录 + `milestones.md` 新 M 条目");
console.log("- 收尾：`npm run sgs:check`（必要时 `--full`）；流程见 `docs/games/sanguosha/batch-playbook.md`");
