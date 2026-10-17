#!/usr/bin/env node
// 开发门禁：主动消息（proactive / 主动推送）严格把控。
//
// 规则：落地方案/计划中若出现「主动消息」相关字样，必须经**人工明确确认**，并在计划文档写入标记
//   【主动消息已人工确认】
// 否则本脚本以非零码退出，阻止把含主动消息的方案当作已确认落地。
//
// 用法：
//   node apps/scripts/check-proactive-gate.mjs                 # 检查最新的 memory_palace/plan/*.md
//   node apps/scripts/check-proactive-gate.mjs <plan1.md> ...  # 检查指定文件
//
// 说明：主动消息默认禁用（PROACTIVE_MESSAGES_ENABLED=0）；运行时被动回复超限会丢弃而非转主动。
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const MARKER = "【主动消息已人工确认】";
const PATTERNS = [/主动消息/, /pushActive/, /\bproactive\b/i];

function mentionsProactive(text) {
  return PATTERNS.some((re) => re.test(text));
}

let files = process.argv.slice(2);
if (files.length === 0) {
  const dir = resolve("memory_palace", "plan");
  if (!existsSync(dir)) {
    console.log("✅ 主动消息开发门禁通过（无计划目录）");
    process.exit(0);
  }
  const all = readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .map((f) => join(dir, f));
  // 计划文件名是 sessionId（UUID），不可按名排序 → 按**修改时间**取最新。
  all.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (all.length === 0) {
    console.log("✅ 主动消息开发门禁通过（无计划文件）");
    process.exit(0);
  }
  files = [all[all.length - 1]];
}

let failed = 0;
for (const f of files) {
  const text = readFileSync(f, "utf8");
  if (!mentionsProactive(text)) continue;
  if (text.includes(MARKER)) {
    console.log(`✅ ${f}: 含主动消息且已人工确认`);
  } else {
    console.error(
      `❌ ${f}: 方案包含主动消息，但缺少人工确认标记「${MARKER}」\n` +
        "   处理：先在计划「待确定事项」列明用途/配额影响，取得人工明确确认后写入标记；或改用被动回复/合并消息。",
    );
    failed++;
  }
}

if (failed > 0) {
  console.error("\n主动消息开发门禁未通过。");
  process.exit(1);
}
console.log("✅ 主动消息开发门禁通过");
