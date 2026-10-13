//! dev 固定前缀命令解析与纯函数（无副作用，便于单测）。
//!
//! 约定（2026-10-13 整改）：dev 引擎的所有对话一律走**显式命令 → MCP 原语**，
//! 禁止用正则/硬编码对自由文本做意图猜测（历史上 `确认|执行` 子串匹配曾误判
//! 「不要解除围栏开始执行」→ confirm）。未命中显式命令的开发类消息由网关提示
//! 用户使用 `#发起`，**不自动建任务**。

export interface DevCommand {
  cmd: "任务" | "进度" | "确认" | "取消" | "能力" | "发起" | "补充" | "文档";
  arg?: string;
}

/**
 * 固定前缀命令硬判断（**显式调用**，非意图猜测）。
 * - `#开发` 作为 `#发起` 的等价别名（历史命令）。
 */
export function parseDevCommand(content: string): DevCommand | null {
  const m = content.trim().match(/^#(任务|进度|确认|取消|能力|发起|补充|开发|文档)(?:[\s　]+([\s\S]+))?$/);
  if (!m) return null;
  const cmd = (m[1] === "开发" ? "发起" : m[1]) as DevCommand["cmd"];
  return { cmd, arg: m[2]?.trim() || undefined };
}

/** 从消息中提取 UUID（sessionId 引用）。 */
export function extractSessionId(content: string): string | null {
  const m = content.match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i);
  return m ? m[0].toLowerCase() : null;
}

/**
 * 解析 `#补充 <引用> <正文>`：从参数中拆出 sessionId 与正文（正文可多行）。
 * 支持 `#补充 <uuid> 正文`、`#补充 任务：<uuid>，正文` 等写法。
 */
export function splitTaskRef(arg: string): { sessionId: string | null; content: string } {
  const sessionId = extractSessionId(arg);
  let content = sessionId ? arg.replace(sessionId, "") : arg;
  content = content
    .replace(/^\s*任务\s*[：:，,]?/, "")
    .replace(/^[：:，,、\s　]+/, "")
    .trim();
  return { sessionId, content };
}

/** 去掉 `#发起` / `#开发` 前缀取需求正文（无正文时返回空串）。 */
export function extractRequirement(content: string): string {
  return content
    .replace(/^#(发起|开发|dev|develop)\s*[:：]?\s*/i, "")
    .replace(/^\[(开发|dev|develop)\]\s*[:：]?\s*/i, "")
    .trim();
}

/** 从子会话输出取标题（「标题：xxx」），否则用需求截断。 */
export function extractTitle(output: string, requirement: string): string {
  const m = output.match(/标题[:：]\s*(.+)/);
  return (m?.[1] ?? requirement).trim().slice(0, 50);
}

/** 格式化能力清单 JSON 为 QQ 可读文本（含使用示例）。 */
export function formatCapabilities(json: string): string {
  let caps: Array<{ name: string; rw: string; description: string; usage: string; example?: string }>;
  try {
    caps = JSON.parse(json);
  } catch {
    return "无能力信息";
  }
  if (!Array.isArray(caps) || caps.length === 0) return "无能力信息";
  const lines = caps.map((c) => {
    const head = `${c.name} [${c.rw}] ${c.description}`;
    const usage = `  ${c.usage}`;
    return c.example ? `${head}\n${usage}\n  示例：${c.example}` : `${head}\n${usage}`;
  });
  return "开发任务 MCP 能力：\n" + lines.join("\n");
}
