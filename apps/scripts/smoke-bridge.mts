// 临时冒烟：用 MCP SDK client 通过 stdio 拉起 dev-bridge，调用只读原语。
// 用完即删（不入库）。
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const appsRoot = resolve(repoRoot, "apps");
const transport = new StdioClientTransport({
  command: process.execPath,
  args: ["--import", "tsx", resolve(appsRoot, "dev-bridge/src/main.ts")],
  cwd: appsRoot,
  env: { SERVER_URL: "http://127.0.0.1:3000" },
  stderr: "inherit",
});
const client = new Client({ name: "smoke", version: "0" });
await client.connect(transport);
const tools = await client.listTools();
console.log("tools=", tools.tools.length);
const caps = await client.callTool({ name: "list_capabilities", arguments: {} });
const arr = JSON.parse((caps.content as any)[0].text);
console.log("capabilities=", arr.length, "first=", arr[0].name);
const perm = await client.callTool({
  name: "check_permission",
  arguments: { op: "create", qq_id: "nobody" },
});
console.log("permission=", (perm.content as any)[0].text);
const missing = await client.callTool({ name: "get_task", arguments: { session_id: "nope" } });
console.log("missing=", (missing.content as any)[0].text);
const tasks = await client.callTool({ name: "list_tasks", arguments: {} });
console.log("tasks head=", String((tasks.content as any)[0].text).slice(0, 120));
const sid = process.env.SMOKE_SESSION_ID;
if (sid) {
  const prog = await client.callTool({ name: "get_progress", arguments: { session_id: sid } });
  const p = JSON.parse(String((prog.content as any)[0].text));
  console.log("progress=", p.status, "todo=", p.todo.done + "/" + p.todo.total, "verify=", p.verify.done + "/" + p.verify.total, "plan=", p.plan_path);
}
await client.close();

// SMOKE_FULL=1：跑一次完整人工生命周期（用真实白名单 openid；会写一条 dev_tasks 记录）
if (process.env.SMOKE_FULL === "1") {
  const client2 = new Client({ name: "smoke-full", version: "0" });
  await client2.connect(new StdioClientTransport({
    command: process.execPath,
    args: ["--import", "tsx", resolve(appsRoot, "dev-bridge/src/main.ts")],
    cwd: appsRoot,
    env: { SERVER_URL: "http://127.0.0.1:3000" },
    stderr: "inherit",
  }));
  const qq = process.env.DEV_WHITELIST_OPENID!;
  const sid = `smoke-b4-${Date.now()}`;
  const call = async (name: string, args: any) => {
    const r = await client2.callTool({ name, arguments: args });
    return String((r.content as any)[0].text);
  };
  console.log("create=", (await call("create_task", { session_id: sid, group_id: "smoke-group", qq_id: qq })).slice(0, 80));
  console.log("planning=", (await call("transition_task", { session_id: sid, status: "planning" })).slice(0, 80));
  console.log("awaiting=", (await call("transition_task", { session_id: sid, status: "awaiting_confirm" })).slice(0, 80));
  console.log("approve_plan=", (await call("approve_plan", { session_id: sid, qq_id: qq })).slice(0, 80));
  console.log("verify=", (await call("transition_task", { session_id: sid, status: "verifying" })).slice(0, 80));
  console.log("approve_verify=", (await call("approve_verify", { session_id: sid, qq_id: qq })).slice(0, 80));
  const ev = JSON.parse(await call("list_events", { session_id: sid }));
  console.log("events=", ev.map((e: any) => e.kind).join(","));
  await client2.close();
}

process.exit(0);
