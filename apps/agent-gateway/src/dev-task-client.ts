//! dev 原语 MCP client（stdio）：agent-gateway → `apps/dev-bridge`。
//! 原语契约不变；白名单/状态/审计在 server（`/internal/dev/*`），桥不直连 SQLite。

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export class DevTaskClient {
  private client: Client | null = null;

  async connect(): Promise<void> {
    const projectRoot = path.resolve(__dirname, "../../..");
    const appsRoot = path.resolve(__dirname, "../..");
    // B8：薄桥是唯一实现（Rust dev-task-mcp 已删除）；可用 DEV_BRIDGE_ENTRY 覆盖入口
    const bridgeEntry = process.env.DEV_BRIDGE_ENTRY ?? "dev-bridge/src/main.ts";
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: ["--import", "tsx", path.resolve(appsRoot, bridgeEntry)],
      cwd: appsRoot, // apps/node_modules 下可解析 tsx
      env: {
        SERVER_URL: process.env.SERVER_URL ?? "http://127.0.0.1:3000",
      },
    });
    this.client = new Client({ name: "agent-gateway", version: "0.1.0" });
    await this.client.connect(transport);
    console.log(`[agent-gateway] 已连接 dev-bridge: ${bridgeEntry}（projectRoot=${projectRoot}）`);
  }

  private async call(name: string, args: Record<string, unknown>): Promise<string> {
    if (!this.client) throw new Error("dev-task MCP client 未连接");
    const result = await this.client.callTool({ name, arguments: args });
    const content = (result.content ?? []) as Array<{ type: string; text?: string }>;
    const block = content.find((c) => c.type === "text");
    return block?.text ? String(block.text) : "";
  }

  // ---- 只读 ----

  async listTasks(status?: string): Promise<string> {
    return this.call("list_tasks", status ? { status } : {});
  }

  async getTask(sessionId: string): Promise<string> {
    return this.call("get_task", { session_id: sessionId });
  }

  async getProgress(sessionId: string): Promise<string> {
    return this.call("get_progress", { session_id: sessionId });
  }

  /** 校验归属+非终态，返回计划短链资源（token 由网关签发）。 */
  async prepareTaskLink(sessionId: string, qqId: string): Promise<string> {
    return this.call("prepare_task_link", { session_id: sessionId, qq_id: qqId });
  }

  /** 校验归属+非终态，授权把自由文本作为补充指令追加到已有子会话。 */
  async appendInstruction(sessionId: string, qqId: string): Promise<string> {
    return this.call("append_instruction", { session_id: sessionId, qq_id: qqId });
  }

  async listCapabilities(): Promise<string> {
    return this.call("list_capabilities", {});
  }

  // ---- 写入 ----

  async createTask(sessionId: string, groupId: string, qqId: string): Promise<string> {
    return this.call("create_task", { session_id: sessionId, group_id: groupId, qq_id: qqId });
  }

  async confirmTask(sessionId: string, qqId: string): Promise<string> {
    return this.call("confirm_task", { session_id: sessionId, qq_id: qqId });
  }

  /** v2：批准计划（仅薄桥提供；Rust MCP 无此原语）。 */
  async approvePlan(sessionId: string, qqId: string): Promise<string> {
    return this.call("approve_plan", { session_id: sessionId, qq_id: qqId });
  }

  /** v2：任务事件增量（审计/通知源）。 */
  async listEvents(sessionId: string, sinceId = 0, limit = 50): Promise<string> {
    return this.call("list_events", { session_id: sessionId, since_id: sinceId, limit });
  }

  /** v2：验收通过（仅薄桥提供）。 */
  async approveVerify(sessionId: string, qqId: string): Promise<string> {
    return this.call("approve_verify", { session_id: sessionId, qq_id: qqId });
  }

  async cancelTask(sessionId: string, qqId: string): Promise<string> {
    return this.call("cancel_task", { session_id: sessionId, qq_id: qqId });
  }

  async transitionTask(sessionId: string, status: string): Promise<string> {
    return this.call("transition_task", { session_id: sessionId, status });
  }

  async updateTaskMeta(
    sessionId: string,
    title?: string,
    planPath?: string,
  ): Promise<string> {
    const args: Record<string, unknown> = { session_id: sessionId };
    if (title) args.title = title;
    if (planPath) args.plan_path = planPath;
    return this.call("update_task_meta", args);
  }
}
