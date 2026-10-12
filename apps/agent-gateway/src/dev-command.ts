//! dev 固定前缀命令解析与纯函数（无副作用，便于单测）。

export interface DevCommand {
  cmd: "任务" | "进度" | "确认" | "取消" | "能力";
  arg?: string;
}

/** 固定前缀命令硬判断（人 = 决策模型，置信度 100%）。 */
export function parseDevCommand(content: string): DevCommand | null {
  const m = content.trim().match(/^#(任务|进度|确认|取消|能力)(?:[\s　]+(.+))?$/);
  if (!m) return null;
  return { cmd: m[1] as DevCommand["cmd"], arg: m[2]?.trim() || undefined };
}

/** 从消息中提取 UUID（sessionId 引用）。 */
export function extractSessionId(content: string): string | null {
  const m = content.match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i);
  return m ? m[0].toLowerCase() : null;
}

export type DevMessageAction =
  | { type: "query"; sessionId: string }
  | { type: "confirm"; sessionId: string }
  | { type: "cancel"; sessionId: string }
  | { type: "capabilities" }
  | { type: "list" }
  | { type: "create" };

/** 语义分类 dev 消息：带 sessionId 的引用 → 查询/确认/取消；能力/列表 → 查询；其余 → 新建。 */
export function classifyDevMessage(content: string): DevMessageAction {
  const sessionId = extractSessionId(content);
  if (sessionId) {
    if (/确认|开始执行|执行吧|验收/.test(content)) return { type: "confirm", sessionId };
    if (/取消|放弃|停止|终止/.test(content)) return { type: "cancel", sessionId };
    return { type: "query", sessionId };
  }
  if (/能力|capabilit/i.test(content) && /查询|列表|有哪些|清单/i.test(content)) {
    return { type: "capabilities" };
  }
  if (/任务列表|全部任务|所有任务|列出.*任务/.test(content)) return { type: "list" };
  return { type: "create" };
}

/** 去掉 #开发 / [开发] 前缀取需求正文。 */
export function extractRequirement(content: string): string {
  const stripped = content
    .replace(/^#(开发|dev|develop)\s*[:：]?\s*/i, "")
    .replace(/^\[(开发|dev|develop)\]\s*[:：]?\s*/i, "");
  return stripped.trim() || content.trim();
}

/** 从子会话输出取标题（「标题：xxx」），否则用需求截断。 */
export function extractTitle(output: string, requirement: string): string {
  const m = output.match(/标题[:：]\s*(.+)/);
  return (m?.[1] ?? requirement).trim().slice(0, 50);
}

/** 格式化能力清单 JSON 为 QQ 可读文本。 */
export function formatCapabilities(json: string): string {
  let caps: Array<{ name: string; rw: string; description: string; usage: string }>;
  try {
    caps = JSON.parse(json);
  } catch {
    return "无能力信息";
  }
  if (!Array.isArray(caps) || caps.length === 0) return "无能力信息";
  const lines = caps.map((c) => `${c.name} [${c.rw}] ${c.description}\n  ${c.usage}`);
  return "开发任务 MCP 能力：\n" + lines.join("\n");
}
