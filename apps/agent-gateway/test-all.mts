// 统一测试入口：显式清单（default-deny）+ 逐文件计时 + 汇总，失败/超时以非零码退出。
// 未归类的新 `test-*.mts` 会让本 runner 直接报错，强制「新增测试时显式决定是否纳入离线套件」。
// 运行：npx tsx test-all.mts（= npm test）
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

/** 离线套件（无外部依赖，必须能自行退出）。新增离线测试在此登记。 */
const INCLUDED = [
  "test-decision.mts",
  "test-intent-confirm.mts",
  "test-systemone.mts",
  "test-dev-command.mts",
  "test-game-command.mts",
  "test-docs.mts",
  "test-link-fetch.mts",
  "test-time-context.mts",
  "test-topic-switch.mts",
  "test-time-guards.mts",
  "test-chat-log.mts",
  "test-profile-scheduler.mts",
  "test-profile-distill.mts",
  "test-profile-inject.mts",
  "test-profile-local-llm.mts",
  "test-context.mts",
];

/** 显式排除（附原因）。 */
const EXCLUDED: Record<string, string> = {
  "test-all.mts": "本 runner 自身",
  "test-helpers.mts": "公共桩，无断言",
  "test-decision-live.mts": "需本地决策服务在线（用 `npm run test:decision:live`）",
  "test-dev-plan.mts": "依赖外部环境且进程不自行退出",
  "test-dev-session.mts": "依赖外部环境且进程不自行退出",
  "test-dev-mcp.mts": "需已构建的 dev-task-mcp 二进制",
  "test-docs-mcp.mts": "需已构建的 docs-mcp 二进制",
  "test-short-link.mts": "需 server 运行",
};

/** 单文件超时（毫秒）：超时即判失败，避免污染整体测试时长。 */
const PER_FILE_TIMEOUT_MS = 120_000;

// 归类完整性：新增/改名 test-*.mts 必须显式归类
const present = readdirSync(here).filter((f) => f.startsWith("test-") && f.endsWith(".mts"));
const unclassified = present.filter((f) => !INCLUDED.includes(f) && !(f in EXCLUDED));
if (unclassified.length > 0) {
  console.error(
    `❌ 未归类的测试文件：${unclassified.join(", ")}\n` +
      "请加入 test-all.mts 的 INCLUDED（离线可跑）或 EXCLUDED（附原因）。",
  );
  process.exit(1);
}

interface Result {
  file: string;
  status: "pass" | "fail" | "timeout";
  ms: number;
  output: string;
}

const results: Result[] = [];
for (const file of INCLUDED) {
  const started = Date.now();
  const r = spawnSync(process.execPath, ["--import", "tsx", join(here, file)], {
    cwd: here,
    encoding: "utf8",
    timeout: PER_FILE_TIMEOUT_MS,
  });
  const ms = Date.now() - started;
  const output = `${r.stdout ?? ""}${r.stderr ?? ""}`.trim();
  const timedOut = r.error?.name === "Error" && /ETIMEDOUT/i.test(r.error.message);
  const status: Result["status"] = timedOut ? "timeout" : r.status === 0 ? "pass" : "fail";
  results.push({ file, status, ms, output });
  const icon = status === "pass" ? "✅" : status === "timeout" ? "⏱️" : "❌";
  console.log(`${icon} ${file} (${ms}ms)${status === "pass" ? "" : `\n${output}`}`);
}

const failed = results.filter((r) => r.status !== "pass");
const totalMs = results.reduce((a, r) => a + r.ms, 0);
console.log(
  `\n${failed.length === 0 ? "✅ 全部通过" : "❌ 有失败"}：${results.length - failed.length}/${results.length} 个测试文件，耗时 ${(totalMs / 1000).toFixed(1)}s`,
);
if (failed.length > 0) {
  console.log(
    "失败清单：\n" + failed.map((r) => `- ${r.file}（${r.status}）`).join("\n"),
  );
  process.exit(1);
}
