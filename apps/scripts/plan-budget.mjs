#!/usr/bin/env node
// 上下文预算守卫：统计一批入口文件的行数，超过阈值时非零退出。
//
// 用法：
//   node apps/scripts/plan-budget.mjs <file...> [--max 2000]
//   node apps/scripts/plan-budget.mjs --from-plan memory_palace/plan/<id>.md --batch B1
//
// --from-plan：从计划「**批次 B1**：…｜入口：`a`、`b`」行解析入口文件；--batch 指定批次前缀。
// 目的：把「每批只读 ≤6 文件 / ≤2000 行」变成可执行检查，避免跨会话靠记忆。
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const maxIdx = args.indexOf("--max");
const max = maxIdx >= 0 ? Number(args[maxIdx + 1] ?? 2000) : 2000;
const fromIdx = args.indexOf("--from-plan");
const batchIdx = args.indexOf("--batch");
const batch = batchIdx >= 0 ? args[batchIdx + 1] : "";

let files = args.filter((a, i) => {
  if (a.startsWith("--")) return false;
  if (maxIdx >= 0 && i === maxIdx + 1) return false;
  if (fromIdx >= 0 && i === fromIdx + 1) return false;
  if (batchIdx >= 0 && i === batchIdx + 1) return false;
  return true;
});

if (fromIdx >= 0) {
  const planPath = args[fromIdx + 1];
  if (!planPath || !existsSync(planPath)) {
    console.error(`[budget] 计划文件不存在: ${planPath}`);
    process.exit(2);
  }
  const text = readFileSync(planPath, "utf8");
  const line = text.split("\n").find((l) => l.includes(`**批次 ${batch}`) || (batch && l.includes(batch) && l.includes("入口：")));
  if (!line) {
    console.error(`[budget] 计划中未找到批次 ${batch} 的入口行`);
    process.exit(2);
  }
  const entry = line.split("入口：")[1] ?? "";
  const parsed = [...entry.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
  files = files.concat(parsed.filter((p) => !p.includes("/{") && !p.endsWith("/**")));
}

if (files.length === 0) {
  console.error("用法: node apps/scripts/plan-budget.mjs <file...> [--max 2000] | --from-plan <plan.md> --batch B1");
  process.exit(2);
}

let total = 0;
let missing = 0;
for (const f of files) {
  const p = resolve(f);
  if (!existsSync(p)) {
    console.log(`  ?  ${f} (missing)`);
    missing++;
    continue;
  }
  const n = readFileSync(p, "utf8").split("\n").length;
  total += n;
  console.log(`  ${String(n).padStart(6)}  ${f}`);
}
console.log(`\n文件 ${files.length - missing}${missing ? `(+${missing} missing)` : ""}，合计 ${total} 行（上限 ${max}）`);
if (missing) {
  console.error("[budget] 有文件不存在，无法完整评估");
  process.exit(2);
}
if (total > max) {
  console.error(`[budget] 超预算 ${total - max} 行 → 拆批或裁剪入口清单`);
  process.exit(1);
}
console.log("[budget] OK");
