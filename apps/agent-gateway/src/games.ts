// 桌游目录：`#开局` 的可选列表（单一数据源）。
//
// 新增桌游引擎时：在 `crates/engines/<name>/` 建 crate、在 `crates/game-engine-mcp` 注册，
// 然后往下面的 `GAME_OPTIONS` 追加一项（id 必须与引擎 `game_id()` 一致），
// 群里的「#开局」列表会自动出现该选项。
//
// 启动方式：
//   lobby  —— 有等待厅（create_game → #加入 / #开始），如拉斯维加斯
//   direct —— 无等待厅，建局即开打，如猜拳

export type GameLaunchMode = "lobby" | "direct";

export interface GameOption {
  /** 引擎 `game_id()`。 */
  id: string;
  /** 群消息里的展示名。 */
  name: string;
  /** 启动方式。 */
  mode: GameLaunchMode;
  /** 一句话介绍（列表里展示）。 */
  summary: string;
  /** 可用别名（`#开局 <别名>`，大小写不敏感）。 */
  aliases: string[];
}

export const GAME_OPTIONS: GameOption[] = [
  {
    id: "las-vegas",
    name: "拉斯维加斯",
    mode: "lobby",
    summary: "2–6 人 · 骰子下注 + 16 个小游戏",
    aliases: ["vegas", "lv", "拉维"],
  },
  {
    id: "rps",
    name: "猜拳",
    mode: "direct",
    summary: "1 人 · 和机器人一轮定胜负",
    aliases: ["rps", "石头剪刀布"],
  },
];

/** `#开局` 的桌游菜单（编号与 `#开局 <编号>` 一致）。 */
export function formatGameMenu(): string {
  const lines = GAME_OPTIONS.map((g, i) => `${i + 1}) ${g.name} — ${g.summary}`);
  return [
    "🎲 想开哪一局？回复「#开局 <编号或名称>」：",
    ...lines,
    `例如：#开局 1 或 #开局 ${GAME_OPTIONS[0]?.name ?? "拉斯维加斯"}`,
  ].join("\n");
}

/**
 * 解析 `#开局 <参数>`：支持 1 起的列表编号、引擎 id、展示名、别名（大小写不敏感）。
 * 空参数 / 未命中返回 null（调用方回菜单）。
 */
export function resolveGameOption(token?: string): GameOption | null {
  const t = (token ?? "").trim().replace(/^#/, "");
  if (!t) return null;

  if (/^\d+$/.test(t)) {
    const index = Number(t);
    return index >= 1 && index <= GAME_OPTIONS.length ? GAME_OPTIONS[index - 1] : null;
  }

  const key = t.toLowerCase();
  return (
    GAME_OPTIONS.find(
      (g) =>
        g.id.toLowerCase() === key ||
        g.name.toLowerCase() === key ||
        g.aliases.some((a) => a.toLowerCase() === key),
    ) ?? null
  );
}

/** direct 模式的建局后引导文案。 */
export function directLaunchPrompt(game: GameOption, host: string): string {
  if (game.id === "rps") {
    return `✊ ${host} 开了一局「猜拳」。直接发「石头」「剪刀」或「布」出拳（和机器人一轮定胜负）。`;
  }
  return `🎲 ${host} 开了一局「${game.name}」。发「#局面」查看局势。`;
}
