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

  /** 创建对局（进入 lobby；群内已有 Active 则返回「错误: ...」）。 */
  async createGame(gameId: string, groupId: string, hostId: string): Promise<string> {
    return this.call("create_game", { game_id: gameId, group_id: groupId, host_id: hostId });
  }

  /** 对局状态（公共视图，已脱敏）。 */
  async getState(roomId: string): Promise<string> {
    return this.call("get_state", { room_id: roomId });
  }

  /** 玩家当前合法动作。 */
  async getLegalActions(roomId: string, playerId: string): Promise<string> {
    return this.call("get_legal_actions", { room_id: roomId, player_id: playerId });
  }

  /** 让内置 AI 替指定玩家行动（bot_action，机器人补位）。 */
  async botAction(roomId: string, playerId: string): Promise<string> {
    return this.call("bot_action", { room_id: roomId, player_id: playerId });
  }

  async executeAction(roomId: string, playerId: string, action: string): Promise<string> {
    return this.call("execute_action", { room_id: roomId, player_id: playerId, action });
  }

  async getActiveGame(groupId: string): Promise<string> {
    return this.call("get_active_game", { group_id: groupId });
  }

  /** 对局局面摘要（紧凑文本，给主持人叙述用）。 */
  async getStateSummary(roomId: string): Promise<string> {
    return this.call("state_summary", { room_id: roomId });
  }

  async getRules(gameId: string): Promise<string> {
    return this.call("get_rules", { game_id: gameId });
  }
}
