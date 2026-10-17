// 拉斯维加斯游戏命令：显式前缀解析（确定性，不经决策模型）。
//
// 约定（`docs/conventions/mcp-primitives.md`）：用户意图只能经**显式命令前缀**或决策原语进入，
// 禁止对自由文本做意图猜测。本模块只做「前缀 → 动作」的确定性映射，产出合法的 `execute_action` 动作串。
//
// 命令表：
//   #开局 / #拉斯维加斯          创建对局（进入 lobby）
//   #加入 / #退出                lobby 加入 / 退出
//   #开始 / #中断                lobby 开赛 / 房主中止
//   #踢 <openid>                房主踢人
//   #小游戏 <3~6>               房主设置小游戏数量 N
//   #放 <1~6> / #放3            下注（把该点数全部骰子放到对应赌场）
//   #重投 / #弃                 支付 1 筹码弃本回合
//   #局面 / #牌面               查看局面摘要

export type GameCommand =
  | { cmd: "create" }
  | { cmd: "join" }
  | { cmd: "leave" }
  | { cmd: "start" }
  | { cmd: "abort" }
  | { cmd: "kick"; target: string }
  | { cmd: "smallgames"; n: number }
  | { cmd: "place"; point: number }
  | { cmd: "reroll" }
  | { cmd: "addbot"; n: number }
  | { cmd: "solo"; n: number }
  | { cmd: "sg"; id: string }
  | { cmd: "board" };

const COMMAND_WORDS: Record<string, GameCommand["cmd"]> = {
  开局: "create",
  拉斯维加斯: "create",
  加入: "join",
  退出: "leave",
  开始: "start",
  中断: "abort",
  踢: "kick",
  小游戏: "smallgames",
  小游戏数: "smallgames",
  放: "place",
  重投: "reroll",
  弃: "reroll",
  加机器人: "addbot",
  机器人: "addbot",
  单人: "solo",
  单机: "solo",
  动作: "sg",
  sg: "sg",
  局面: "board",
  牌面: "board",
};

/**
 * 解析显式游戏命令；非命令返回 null（交回决策模型）。
 * 支持三种写法：`#命令 参数`（空格分隔）、`#命令<数字>`（如 `#放3`）、`#命令`（无参）。
 */
export function parseGameCommand(content: string): GameCommand | null {
  const trimmed = content.trim();
  let word: string | undefined;
  let arg = "";

  let m = trimmed.match(/^#(\S+)\s+([\s\S]*)$/);
  if (m) {
    word = m[1];
    arg = (m[2] ?? "").trim();
  } else if ((m = trimmed.match(/^#([^\s0-9]+)(\d+)$/))) {
    word = m[1];
    arg = m[2];
  } else if ((m = trimmed.match(/^#(\S+)$/))) {
    word = m[1];
  } else {
    return null;
  }

  const cmd = COMMAND_WORDS[word];
  if (!cmd) return null;

  switch (cmd) {
    case "kick":
      return arg ? { cmd, target: arg } : null;
    case "smallgames": {
      const n = Number(arg);
      return Number.isInteger(n) && n >= 3 && n <= 6 ? { cmd, n } : null;
    }
    case "place": {
      const n = Number(arg);
      return Number.isInteger(n) && n >= 1 && n <= 6 ? { cmd, point: n } : null;
    }
    case "addbot": {
      const n = arg ? Number(arg) : 1;
      return Number.isInteger(n) && n >= 1 && n <= 5 ? { cmd, n } : null;
    }
    case "solo": {
      const n = arg ? Number(arg) : 3;
      return Number.isInteger(n) && n >= 1 && n <= 5 ? { cmd, n } : null;
    }
    case "sg":
      return arg ? { cmd, id: arg } : null;
    default:
      return { cmd };
  }
}

/** 命令 → MCP `execute_action` 的动作串；`create` / `board` 无动作（返回 null）。 */
export function gameActionId(cmd: GameCommand): string | null {
  switch (cmd.cmd) {
    case "join":
      return "lobby:join";
    case "leave":
      return "lobby:leave";
    case "start":
      return "lobby:start";
    case "abort":
      return "lobby:abort";
    case "kick":
      return `lobby:kick:${cmd.target}`;
    case "smallgames":
      return `lobby:set_small_game_count:${cmd.n}`;
    case "place":
      return `place:${cmd.point}`;
    case "reroll":
      return "reroll";
    case "addbot":
      return "lobby:add_bot";
    case "sg":
      return `sg:${cmd.id}`;
    default:
      return null;
  }
}

/** 创建对局后的引导文案。 */
export function lobbyPrompt(host: string, count: number, n: number): string {
  return [
    `🎲 ${host} 发起了一局「拉斯维加斯」！`,
    `其他玩家 @我 发「#加入」入局（当前 ${count} 人，2–6 人可开赛）。`,
    `一个人测：发「#单人」直接和 3 个机器人开赛；或发「#加机器人」补位。`,
    `房主发「#开始」开赛；可在开赛前发「#小游戏 3~6」设置小游戏数量（当前 ${n}）。`,
    `开赛后：发「#放 <点数>」下注、「#重投」付 1 筹码弃回合、「#局面」看局势。`,
  ].join("\n");
}

/** 有对局但用户用了自由文本（game_action 意图）时的引导。 */
export const GAME_ACTION_HINT =
  "本群拉斯维加斯对局进行中。请用：#放 <点数> 下注 / #重投 弃回合 / #局面 查看局势。";

/** 可选的合法动作（MCP `get_legal_actions` 的元素）。 */
export interface LegalAction {
  id: string;
  description: string;
}

/** 决策模型在合法动作里选一个的上限（超过则不做单题选择，改用清单兜底）。 */
export const GAME_ACTION_CHOICE_LIMIT = 26;

/** 把合法动作列表渲染为给用户的编号清单（决策兜底/提示）。 */
export function formatActionList(actions: LegalAction[]): string {
  if (actions.length === 0) {
    return "当前没有可执行的动作。";
  }
  const lines = actions.map((a, i) => `${i + 1}) ${a.description}（${a.id}）`);
  return (
    `可选动作：\n${lines.join("\n")}\n` +
    "（也可用显式命令：#放 3 / #重投 / #局面；小游戏动作超限时用 #动作 <id>）"
  );
}
