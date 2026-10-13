//! docs-mcp client（stdio）：agent-gateway → docs-mcp 进程。
//! 只读原语：prepare_docs_link / list_capabilities；白名单校验在 Rust 侧。

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function defaultMcpPath(): string {
  const exe = process.platform === "win32" ? "docs-mcp.exe" : "docs-mcp";
  return path.resolve(__dirname, "../../../crates/target/debug", exe);
}

export class DocsClient {
  private client: Client | null = null;

  async connect(): Promise<void> {
    const mcpPath = process.env.DOCS_MCP ?? defaultMcpPath();
    const projectRoot = path.resolve(__dirname, "../../..");
    const transport = new StdioClientTransport({
      command: mcpPath,
      args: [],
      cwd: projectRoot, // 让 docs-mcp 的 DOCS_CONFIG 相对路径与 server 一致
      // MCP SDK 默认只继承白名单环境变量，需显式传递 docs-mcp 所需变量
      env: {
        DEV_WHITELIST: process.env.DEV_WHITELIST ?? "",
        DOCS_CONFIG: process.env.DOCS_CONFIG ?? "config/dev-docs.json",
      },
    });
    this.client = new Client({ name: "agent-gateway", version: "0.1.0" });
    await this.client.connect(transport);
    console.log(`[agent-gateway] 已连接 docs MCP: ${mcpPath}`);
  }

  private async call(name: string, args: Record<string, unknown>): Promise<string> {
    if (!this.client) throw new Error("docs MCP client 未连接");
    const result = await this.client.callTool({ name, arguments: args });
    const content = (result.content ?? []) as Array<{ type: string; text?: string }>;
    const block = content.find((c) => c.type === "text");
    return block?.text ? String(block.text) : "";
  }

  /** 白名单校验，返回覆盖全部文档根的授权资源 docs:all（token 由网关签发）。 */
  async prepareDocsLink(qqId: string): Promise<string> {
    return this.call("prepare_docs_link", { qq_id: qqId });
  }

  async listCapabilities(): Promise<string> {
    return this.call("list_capabilities", {});
  }
}
