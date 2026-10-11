//! dev-task MCP client（stdio）：agent-gateway → dev-task-mcp 进程。
//! 与 game-engine MCP 同构：读写原语 + 白名单/状态机校验在 Rust 侧。

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function defaultMcpPath(): string {
  const exe = process.platform === "win32" ? "dev-task-mcp.exe" : "dev-task-mcp";
  return path.resolve(__dirname, "../../../crates/target/debug", exe);
}

export class DevTaskClient {
  private client: Client | null = null;

  async connect(): Promise<void> {
    const mcpPath = process.env.DEV_TASK_MCP ?? defaultMcpPath();
    const projectRoot = path.resolve(__dirname, "../../..");
    const transport = new StdioClientTransport({
      command: mcpPath,
      args: [],
      cwd: projectRoot, // 让 dev-task-mcp 的 SQLITE_PATH 相对路径与 server 一致
      // MCP SDK 默认只继承白名单环境变量，需显式传递 dev-task-mcp 所需变量
      env: {
        DEV_WHITELIST: process.env.DEV_WHITELIST ?? "",
        SQLITE_PATH: process.env.SQLITE_PATH ?? "data/tabletop.db",
      },
    });
    this.client = new Client({ name: "agent-gateway", version: "0.1.0" });
    await this.client.connect(transport);
    console.log(`[agent-gateway] 已连接 dev-task MCP: ${mcpPath}`);
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
