#!/usr/bin/env node
// 外部约束事实登记表「纪律」检查：
//   硬检查：docs/conventions/external-constraints.md 存在且含「来源」「验证状态」列（缺 → 退出码 1）
//   提示：列出 docs/.pi/memory_palace 中数值型约束行「未带来源/验证状态标记」的项，供人工复核
// 用法：node apps/scripts/check-constraints.mjs
import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { join } from "node:path";

const REGISTRY = "docs/conventions/external-constraints.md";
const ROOTS = ["docs", ".pi/prompts", "memory_palace/decisions"];
const NUMERIC = /\d+(?:\.\d+)?\s*(?:分钟|秒|次|条|字|qpm|qps|GB|天|ms)/i;
const MARK = /(来源|官方|实测|bot\.q\.qq\.com|已验证|待验证|external-constraints|消息收发概述)/i;

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) out.push(...walk(p));
    else if (name.endsWith(".md")) out.push(p);
  }
  return out;
}

// 1) 硬检查：登记表
if (!existsSync(REGISTRY)) {
  console.error(`❌ 缺少外部约束事实登记表：${REGISTRY}`);
  process.exit(1);
}
const reg = readFileSync(REGISTRY, "utf-8");
if (!/来源/.test(reg) || !/验证状态/.test(reg)) {
  console.error(`❌ 登记表 ${REGISTRY} 缺少「来源」或「验证状态」列`);
  process.exit(1);
}
console.log(`✅ 登记表存在且含「来源」「验证状态」列：${REGISTRY}`);

// 2) 提示：未标注来源的数值约束行
let total = 0;
let unmarked = 0;
for (const root of ROOTS) {
  for (const file of walk(root)) {
    if (file.replaceAll("\\", "/") === REGISTRY) continue;
    const lines = readFileSync(file, "utf-8").split("\n");
    const defer = /external-constraints|官方|消息收发概述/.test(lines.join("\n")); // 文件已指向登记表/官方口径
    lines.forEach((line, i) => {
      if (!NUMERIC.test(line)) return;
      total++;
      const ctx = [lines[i - 1] ?? "", line, lines[i + 1] ?? ""].join("\n");
      if (defer || MARK.test(ctx)) return;
      unmarked++;
      console.log(`  ⚠️ ${file}:${i + 1}  ${line.trim().slice(0, 100)}`);
    });
  }
}
console.log(
  `\n数值约束行 ${total} 处，未带来源/验证状态标记 ${unmarked} 处（供人工复核，非硬失败）。`,
);
