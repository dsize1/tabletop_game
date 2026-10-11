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

/** 去掉 #开发 前缀取需求正文。 */
export function extractRequirement(content: string): string {
  return content.replace(/^#(开发|dev|develop)\s*/i, "").trim() || content.trim();
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
