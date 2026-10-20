//! dev-bridge：MCP stdio 服务端（薄桥）。
//!
//! 启动：`npx tsx apps/dev-bridge/src/main.ts`（cwd 任意；状态全部走 SERVER_URL）。
//! 环境：SERVER_URL（默认 http://127.0.0.1:3000）。
//!
//! 只做「原语 ↔ HTTP」翻译：不直连 SQLite、不判状态合法性（server 判）。

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

import { DevApi } from "./client.js";
import { createPrimitives, type PrimitiveArgs, type PrimitiveTable } from "./primitives.js";
import { repoRoot } from "./plan.js";

/** 工具清单（inputSchema 与旧 MCP 对齐；v2 原语标 internal）。 */
export const TOOLS = [
  { name: "list_tasks", description: "[读] 列出所有开发任务（可按状态过滤），返回 session_id/status/title/group_id/qq_id", inputSchema: { type: "object", properties: { status: { type: "string" } }, additionalProperties: true } },
  { name: "get_task", description: "[读] 查询单个开发任务详情", inputSchema: { type: "object", properties: { session_id: { type: "string" } }, required: ["session_id"], additionalProperties: true } },
  { name: "get_progress", description: "[读] 查询任务进度（阶段 + 待办/验证完成度）", inputSchema: { type: "object", properties: { session_id: { type: "string" } }, required: ["session_id"], additionalProperties: true } },
  { name: "prepare_task_link", description: "[读] 校验任务归属+非终态，返回计划短链资源（网关据此签发 token）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "append_instruction", description: "[写] 校验归属+非终态，授权把自由文本作为补充指令追加到已有子会话", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "list_capabilities", description: "[读] 列出本服务全部 MCP 能力（能力名/简述/使用方式/读写）", inputSchema: { type: "object", properties: {}, additionalProperties: true } },
  { name: "create_task", description: "[写] 创建开发任务（白名单校验），初始状态 queued", inputSchema: { type: "object", properties: { session_id: { type: "string" }, group_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "group_id", "qq_id"], additionalProperties: true } },
  { name: "confirm_task", description: "[写] 人工确认计划并开始执行（白名单+任务归属校验）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "cancel_task", description: "[写] 人工取消任务（白名单+任务归属校验）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "transition_task", description: "[写] 任务状态迁移（内容里程碑，确定性迁移表校验）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, status: { type: "string" } }, required: ["session_id", "status"], additionalProperties: true } },
  { name: "update_task_meta", description: "[写] 回填任务标题/计划路径（子会话计划生成后由网关调用）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, title: { type: "string" }, plan_path: { type: "string" }, plan_format: { type: "string" } }, required: ["session_id"], additionalProperties: true } },
  { name: "approve_plan", description: "[写] 内部：批准计划（awaiting_confirm→running）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "approve_verify", description: "[写] 内部：验收通过（verifying→done）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, qq_id: { type: "string" } }, required: ["session_id", "qq_id"], additionalProperties: true } },
  { name: "start_run", description: "[写] 内部：启动 run（映射 dev_runs；同任务单活跃）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, stage: { type: "string" } }, required: ["session_id", "stage"], additionalProperties: true } },
  { name: "finish_run", description: "[写] 内部：结束 run（succeeded/failed/cancelled）", inputSchema: { type: "object", properties: { run_id: { type: "string" }, status: { type: "string" }, executor_ref: { type: "string" }, last_error: { type: "string" } }, required: ["run_id"], additionalProperties: true } },
  { name: "cancel_run", description: "[写] 内部：取消当前活跃 run", inputSchema: { type: "object", properties: { session_id: { type: "string" } }, required: ["session_id"], additionalProperties: true } },
  { name: "list_events", description: "[读] 内部：任务事件增量（审计/通知源）", inputSchema: { type: "object", properties: { session_id: { type: "string" }, since_id: { type: "number" }, limit: { type: "number" } }, required: ["session_id"], additionalProperties: true } },
  { name: "check_permission", description: "[读] 内部：权限判定（白名单+归属）", inputSchema: { type: "object", properties: { op: { type: "string" }, qq_id: { type: "string" }, session_id: { type: "string" } }, required: ["op", "qq_id"], additionalProperties: true } },
];

export async function dispatch(
  prims: PrimitiveTable,
  name: string,
  args: PrimitiveArgs,
): Promise<string> {
  try {
    switch (name) {
      case "list_tasks": return await prims.list_tasks(args);
      case "get_task": return await prims.get_task(args);
      case "get_progress": return await prims.get_progress(args);
      case "prepare_task_link": return await prims.prepare_task_link(args);
      case "append_instruction": return await prims.append_instruction(args);
      case "list_capabilities": return await prims.list_capabilities();
      case "create_task": return await prims.create_task(args);
      case "confirm_task": return await prims.confirm_task(args);
      case "cancel_task": return await prims.cancel_task(args);
      case "transition_task": return await prims.transition_task(args);
      case "update_task_meta": return await prims.update_task_meta(args);
      case "approve_plan": return await prims.approve_plan(args);
      case "approve_verify": return await prims.approve_verify(args);
      case "start_run": return await prims.start_run(args);
      case "finish_run": return await prims.finish_run(args);
      case "cancel_run": return await prims.cancel_run(args);
      case "list_events": return await prims.list_events(args);
      case "check_permission": return await prims.check_permission(args);
      default: return `错误: 未知原语 ${name}`;
    }
  } catch (e) {
    return `错误: ${(e as Error).message}`;
  }
}

export async function startServer(): Promise<void> {
  const api = new DevApi();
  const prims = createPrimitives(api);
  const server = new Server(
    { name: "dev-bridge", version: "0.1.0" },
    { capabilities: { tools: {} } },
  );
  server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));
  server.setRequestHandler(CallToolRequestSchema, async (req) => {
    const text = await dispatch(prims, req.params.name, (req.params.arguments ?? {}) as PrimitiveArgs);
    return { content: [{ type: "text", text }] };
  });
  await server.connect(new StdioServerTransport());
  // stdio 被协议占用：日志只能写 stderr
  console.error(`[dev-bridge] ready root=${repoRoot()}`);
}

// 直接运行（非被 import）时启动（Windows 下 argv[1] 为绝对路径）
const entry = process.argv[1] ?? "";
if (entry.endsWith("main.ts") || entry.endsWith("main.js")) {
  startServer().catch((e) => {
    console.error("[dev-bridge] 启动失败:", e);
    process.exit(1);
  });
}
