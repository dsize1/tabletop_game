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
//
// 2026-10-20 修复两处缺陷（ADR-0015 §8 / 计划 harness）：
//   ① 原实现按 mtime 降序后取 all[all.length-1]，实际取到**最旧**计划（注释与行为相反）。
//   ② 原实现用 text.includes(MARKER)，导致「说明如何添加标记」的文档（引用/代码块/正文提及）即被误判为已确认。
//      现改为：剥离代码块、行内代码、引用行后，标记必须**单独成行**（trim 后以标记开头）才算确认。
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const MARKER = "【主动消息已人工确认】";
export const PATTERNS = [/主动消息/, /pushActive/, /\bproactive\b/i];

/** 去掉代码块、行内代码、引用行（`>`），避免「解释标记」被当成「标记」。 */
export function stripNoise(text) {
  return text
    .replace(/```[\s\S]*?```/g, "")
    .replace(/`[^`\n]*`/g, "")
    .split("\n")
    .filter((line) => !line.trimStart().startsWith(">"))
    .join("\n");
}

/** 是否提到主动消息（用原文判定，保守：代码块里提到也算）。 */
export function mentionsProactive(text) {
  return PATTERNS.some((re) => re.test(text));
}

/** 是否存在「已人工确认」标记（必须单独成行；正文/引用/代码里的提及不算）。 */
export function hasConfirmationMarker(text) {
  return stripNoise(text)
    .split("\n")
    .some((line) => line.trim().replace(/^[-*]\s+/, "").startsWith(MARKER));
}

/** 取目录下最新修改的计划文件（按 mtime 降序，取第一个）。 */
export function pickLatestPlanFile(dir) {
  if (!existsSync(dir)) return null;
  const all = readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .map((f) => join(dir, f));
  if (all.length === 0) return null;
  all.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return all[0];
}

/** 纯函数：检查一组文件，返回结果清单（便于单测）。 */
export function checkFiles(files) {
  const results = [];
  for (const f of files) {
    const text = readFileSync(f, "utf8");
    if (!mentionsProactive(text)) {
      results.push({ file: f, ok: true, skipped: true });
      continue;
    }
    results.push({ file: f, ok: hasConfirmationMarker(text), skipped: false });
  }
  return results;
}

function main() {
  let files = process.argv.slice(2);
  if (files.length === 0) {
    const latest = pickLatestPlanFile(resolve("memory_palace", "plan"));
    if (!latest) {
      console.log("✅ 主动消息开发门禁通过（无计划目录或计划文件）");
      return 0;
    }
    files = [latest];
  }

  const results = checkFiles(files);
  let failed = 0;
  for (const r of results) {
    if (r.ok) {
      console.log(r.skipped ? `— ${r.file}: 未涉及主动消息` : `✅ ${r.file}: 含主动消息且已人工确认`);
      continue;
    }
    console.error(
      `❌ ${r.file}: 方案包含主动消息，但缺少人工确认标记「${MARKER}」\n` +
        "   处理：先在计划「待确定事项」列明用途/配额影响，取得人工明确确认后，在计划中**单独一行**写入该标记；或改用被动回复/合并消息。",
    );
    failed++;
  }
  if (failed > 0) {
    console.error("\n主动消息开发门禁未通过。");
    return 1;
  }
  console.log("✅ 主动消息开发门禁通过");
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(main());
}
