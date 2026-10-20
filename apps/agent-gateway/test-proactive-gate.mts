// 主动消息门禁脚本单测（离线）。
// 覆盖 2026-10-20 修复的两个缺陷：
//   ① 默认取「最新」计划文件（不是最旧）；
//   ② 标记必须在计划中单独成行，正文/引用/代码块里的提及不算确认（负样本防「自证」）。
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// @ts-ignore 纯 JS 脚本（无类型声明），此处只做行为断言
const gate: any = await import("../scripts/check-proactive-gate.mjs");

const { MARKER, mentionsProactive, hasConfirmationMarker, pickLatestPlanFile, checkFiles } = gate;

// ---- mentionsProactive：识别是否涉及主动消息 ----
assert.equal(mentionsProactive("这里要发主动消息"), true);
assert.equal(mentionsProactive("调用 pushActive 推送"), true);
assert.equal(mentionsProactive("proactive gating"), true);
assert.equal(mentionsProactive("普通聊天与游戏链路"), false);

// ---- hasConfirmationMarker：只有单独成行才算确认 ----
const markerLine = MARKER;
assert.equal(hasConfirmationMarker(`${markerLine}`), true, "单独成行应通过");
assert.equal(hasConfirmationMarker(`- ${markerLine}`), true, "列表项应通过");
assert.equal(hasConfirmationMarker(`* ${markerLine} 备注`), true, "列表项带后缀应通过");

assert.equal(
  hasConfirmationMarker(`计划中需要写入 ${markerLine} 这个标记`),
  false,
  "正文提及不算确认（防自证）",
);
assert.equal(hasConfirmationMarker(`> ${markerLine}`), false, "引用行不算确认");
assert.equal(hasConfirmationMarker(`说明：\`${markerLine}\``), false, "行内代码不算确认");
assert.equal(
  hasConfirmationMarker(["```md", markerLine, "```"].join("\n")),
  false,
  "代码块不算确认",
);
assert.equal(hasConfirmationMarker("普通文本"), false);

// ---- pickLatestPlanFile：取最新（按 mtime），不是最旧 ----
const dir = mkdtempSync(join(tmpdir(), "proactive-gate-"));
try {
  const older = join(dir, "a-older.md");
  const newer = join(dir, "b-newer.md");
  writeFileSync(older, "旧计划");
  writeFileSync(newer, "新计划");
  const oldTime = new Date(Date.now() - 60_000);
  utimesSync(older, oldTime, oldTime);
  assert.equal(pickLatestPlanFile(dir), newer, "应取 mtime 最新的计划");

  // ---- checkFiles：三态（未涉及 / 已确认 / 未确认） ----
  const plain = join(dir, "c-plain.md");
  const confirmed = join(dir, "d-confirmed.md");
  const missing = join(dir, "e-missing.md");
  writeFileSync(plain, "只聊聊天");
  writeFileSync(confirmed, `方案会发主动消息\n\n${markerLine}\n`);
  writeFileSync(missing, "方案会发主动消息，但没写标记");
  const results = checkFiles([plain, confirmed, missing]);
  assert.deepEqual(
    results.map((r: any) => ({ skipped: r.skipped, ok: r.ok })),
    [
      { skipped: true, ok: true },
      { skipped: false, ok: true },
      { skipped: false, ok: false },
    ],
    "三态检查结果不符",
  );
} finally {
  rmSync(dir, { recursive: true, force: true });
}

console.log("✅ test-proactive-gate 通过");
