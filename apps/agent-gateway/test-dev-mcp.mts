// dev-task-mcp 集成验证（需先 cargo build -p dev-task-mcp 产出二进制）。
// 运行：npx tsx test-dev-mcp.mts
// 可用 DEV_TASK_MCP 指定二进制路径（默认 crates/target/debug）。
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";
import assert from "node:assert";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const exe =
  process.env.DEV_TASK_MCP ??
  path.resolve(
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

// list_capabilities：应含全部工具（含新增 prepare_task_link / append_instruction）
const caps = JSON.parse(await call("list_capabilities"));
assert.ok(Array.isArray(caps) && caps.length >= 11, "能力清单应含全部工具");
for (const name of ["prepare_task_link", "append_instruction"]) {
  assert.ok(caps.some((c: { name: string }) => c.name === name), `能力清单应含 ${name}`);
}

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

// prepare_task_link：归属+非终态 → 资源（token 由网关签发）
const link = JSON.parse(await call("prepare_task_link", { session_id: "sid-1", qq_id: "testdev" }));
assert.equal(link.resource, "dev_task:sid-1");
assert.ok((await call("prepare_task_link", { session_id: "sid-1", qq_id: "other" })).includes("错误"));

// append_instruction：授权追加自由文本到子会话
const append = JSON.parse(
  await call("append_instruction", { session_id: "sid-1", qq_id: "testdev" }),
);
assert.equal(append.session_id, "sid-1");
assert.ok((await call("append_instruction", { session_id: "sid-1", qq_id: "other" })).includes("错误"));

// 终态后 prepare_task_link/append_instruction 被拒
await call("transition_task", { session_id: "sid-1", status: "verifying" });
await call("confirm_task", { session_id: "sid-1", qq_id: "testdev" }); // verifying -> done
assert.ok(
  (await call("prepare_task_link", { session_id: "sid-1", qq_id: "testdev" })).includes("错误"),
  "终态应拒 prepare_task_link",
);
assert.ok(
  (await call("append_instruction", { session_id: "sid-1", qq_id: "testdev" })).includes("错误"),
  "终态应拒 append_instruction",
);

// list_tasks
const tasks = JSON.parse(await call("list_tasks"));
assert.ok(tasks.some((t: { session_id: string }) => t.session_id === "sid-1"));

console.log("test-dev-mcp 集成验证通过 ✅");
await transport.close();
