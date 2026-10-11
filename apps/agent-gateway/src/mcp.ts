//! game-engine MCP client（stdio）：agent-gateway → game-engine-mcp 进程。
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function defaultEnginePath(): string {
  const exe = process.platform === "win32" ? "game-engine-mcp.exe" : "game-engine-mcp";
  return path.resolve(__dirname, "../../../crates/target/debug", exe);
}

export class GameEngineClient {
  private client: Client | null = null;

  async connect(): Promise<void> {
    const enginePath = process.env.GAME_ENGINE_MCP ?? defaultEnginePath();
    const projectRoot = path.resolve(__dirname, "../../..");
    const transport = new StdioClientTransport({
      command: enginePath,
      args: [],
      cwd: projectRoot, // 让 game-engine-mcp 的 SQLITE_PATH 与 server 一致
      // MCP SDK 默认只继承白名单环境变量，需显式传递 SQLITE_PATH
      env: {
        SQLITE_PATH: process.env.SQLITE_PATH ?? "data/tabletop.db",
      },
    });
    this.client = new Client({ name: "agent-gateway", version: "0.1.0" });
    await this.client.connect(transport);
    console.log(`[agent-gateway] 已连接 game-engine MCP: ${enginePath}`);
  }

  private async call(name: string, args: Record<string, unknown>): Promise<string> {
    if (!this.client) throw new Error("MCP client 未连接");
    const result = await this.client.callTool({ name, arguments: args });
    const content = (result.content ?? []) as Array<{ type: string; text?: string }>;
    const block = content.find((c) => c.type === "text");
    return block?.text ? String(block.text) : "";
  }

  async startGame(gameId: string, groupId: string, players: string[]): Promise<string> {
    return this.call("start_game", { game_id: gameId, group_id: groupId, players });
  }

  async executeAction(roomId: string, playerId: string, action: string): Promise<string> {
    return this.call("execute_action", { room_id: roomId, player_id: playerId, action });
  }

  async getActiveGame(groupId: string): Promise<string> {
    return this.call("get_active_game", { group_id: groupId });
  }

  async getRules(gameId: string): Promise<string> {
    return this.call("get_rules", { game_id: gameId });
  }
}
