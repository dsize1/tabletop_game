// 三国杀的 Q 群侧文案与命令白名单（P7 最小化，ADR-0031）。
//
// 原则（用户 2026-11-01 定稿）：
// - 群里只做「开局设定 + 极简局面播报 + 登录链接」，**不做逐动作交互**；
// - 局面播报一行：轮次 / 回合阶段 / 等待者 / 存活；不播报公共局面细节、不列合法动作；
// - 关闭 LLM 叙述；机器人推进归 P6。

import type { GameCommand } from "./game-command.js";

/** 换行常量（避免模板里出现裸换行转义）。 */
const NL = String.fromCharCode(10);

/** 群里允许的三国杀命令（其余一律引导到 Web）。 */
const SGS_ALLOWED = new Set<GameCommand["cmd"]>([
  "create",
  "join",
  "leave",
  "start",
  "abort",
  "board",
  "resume",
  "kick",
  // P6.3：机器人补位 + 武将包（都是「开局设定」范畴，ADR-0031）
  "addbot",
  "generalpacks",
]);

/** 该命令在三国杀对局中是否允许在群里执行。 */
export function isSgsChatCommandAllowed(cmd: GameCommand["cmd"]): boolean {
  return SGS_ALLOWED.has(cmd);
}

/** 建局后的引导（lobby）。 */
export function sgsLobbyPrompt(host: string, count: number): string {
  return [
    `🀄 ${host} 发起了一局「三国杀」（5–8 人身份局）。`,
    `其他玩家发「#加入」入局（当前 ${count} 人）。`,
    `人数不够：房主「#加机器人 4」补位（机器人由服务器自动行动）。`,
    `武将包：房主「#武将包」看预设、「#武将包 all」用全部已实现武将。`,
    `房主发「#开始」发身份并进入选将；本局在网页里选将与出牌。`,
    `中途退出：房主「#中断」；随时「#局面」看轮次。`,
  ].join("\n");
}

/** 开赛后的引导（含登录链接时由调用方追加）。 */
export function sgsStartedPrompt(count: number): string {
  return `🀄 已开局（${count} 人）。请在网页完成选将与出牌；链接 10 分钟内有效。`;
}

/** 极简局面播报：一行摘要 + 可选链接。 */
export function sgsBoardText(summary: string, linkLine?: string): string {
  const head = summary.trim() || "三国杀：进行中";
  return linkLine ? `${head}\n🔗 你的牌局页：${linkLine}` : head;
}

/** 动作类命令被拒绝时的引导。 */
export function sgsRefuseHint(statusUrl?: string): string {
  return [
    "三国杀的出牌/选将请在网页完成（群里暂不支持逐动作操作）。",
    statusUrl ? `你的牌局页：${statusUrl}` : "发「#网页」获取你的登录链接。",
    "看进度：发「#局面」。",
  ].join("\n");
}

/** 无对局时的提示（三国杀）。 */
export const SGS_NO_GAME = "本群没有进行中的三国杀对局。房主发「#开局 三国杀」建局。";

/** 登录链接列表（开局/开始时列出所有真人玩家）。 */
export function sgsLinkList(
  title: string,
  entries: { name: string; url: string }[],
): string {
  if (entries.length === 0) return title;
  return [title, ...entries.map((e) => `· ${e.name}：${e.url}`)].join("\n");
}

/** `#武将包` 菜单（预设来自引擎 lobby 合法动作，单一数据源，不硬编码）。 */
export function sgsGeneralPackMenu(entries: { label: string }[]): string {
  if (entries.length === 0) {
    return "当前没有可切换的武将包（可能已开局，或只有默认一个）。";
  }
  const lines = entries.map((e, i) => `${i + 1}) ${e.label}`);
  return [
    "🎴 可选武将包（发「#武将包 <编号>」设置，或「#武将包 all」用全部已实现武将）：",
    ...lines,
  ].join(NL);
}

/** `#武将包` 设置成功回执。 */
export function sgsGeneralPackApplied(label: string): string {
  return `✅ 已设置${label}。人齐后发「#开始」。`;
}

/** 机器人补位回执（三国杀）。 */
export function sgsBotsAdded(count: number, seats: number): string {
  return [
    `🤖 已加入 ${count} 个机器人（当前 ${seats}/8）。`,
    "机器人由服务器自动行动；你只需在网页操作自己的回合。",
    `人齐后发「#开始」。`,
  ].join(NL);
}
