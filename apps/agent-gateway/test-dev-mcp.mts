// dev-task-mcp 集成验证（需先 cargo build -p dev-task-mcp 产出二进制）。
// 运行：npx tsx test-dev-mcp.mts
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";
import assert from "node:assert";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const exe = path.resolve(
  __dirname,
  "../../crates/target/debug",
  process.platform === "win32" ? "dev-task-mcp.exe" : "dev-task-mcp",
);
const projectRoot = path.resolve(__dirname, "../..");

// 独立测试库 + 白名单（经 env 显式传给子进程）
const sqlitePath = path.join(os.tmpdir(), `dev-task-smoke-${Date.now()}.db`);

const transport = new StdioClientTransport({
  command: exe,
  args: [],
  cwd: projectRoot,
  env: {
    DEV_WHITELIST: "testdev",
    SQLITE_PATH: sqlitePath,
  },
});
const client = new Client({ name: "smoke", version: "0.1.0" });
await client.connect(transport);

async function call(name: string, args: Record<string, unknown> = {}): Promise<string> {
  const result = await client.callTool({ name, arguments: args });
  const content = result.content as Array<{ type: string; text?: string }>;
  return content.find((c) => c.type === "text")?.text ?? "";
}

// list_capabilities：应有 9 个能力
const caps = JSON.parse(await call("list_capabilities"));
assert.ok(Array.isArray(caps) && caps.length >= 9, "能力清单应含全部工具");

// create_task（白名单）
const created = JSON.parse(
  await call("create_task", { session_id: "sid-1", group_id: "g1", qq_id: "testdev" }),
);
assert.equal(created.status, "queued");

// 非白名单被拒
const denied = await call("create_task", { session_id: "sid-2", group_id: "g1", qq_id: "outsider" });
assert.ok(denied.includes("错误"), "非白名单应被拒");

// 确定性状态迁移
assert.equal(
  JSON.parse(await call("transition_task", { session_id: "sid-1", status: "planning" })).status,
  "planning",
);
assert.equal(
  JSON.parse(await call("transition_task", { session_id: "sid-1", status: "awaiting_confirm" })).status,
  "awaiting_confirm",
);

// 非法迁移被拒（awaiting_confirm -> done 跳过 running）
const illegal = await call("transition_task", { session_id: "sid-1", status: "done" });
assert.ok(illegal.includes("错误"), "非法迁移应被拒");

// confirm（归属校验）→ running
assert.equal(
  JSON.parse(await call("confirm_task", { session_id: "sid-1", qq_id: "testdev" })).status,
  "running",
);
// 非归属者 confirm 被拒
const notOwner = await call("confirm_task", { session_id: "sid-1", qq_id: "other" });
assert.ok(notOwner.includes("错误"), "非归属者应被拒");

// list_tasks
const tasks = JSON.parse(await call("list_tasks"));
assert.ok(tasks.some((t: { session_id: string }) => t.session_id === "sid-1"));

console.log("test-dev-mcp 集成验证通过 ✅");
await transport.close();
