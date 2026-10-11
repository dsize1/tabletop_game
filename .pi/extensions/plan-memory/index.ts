/**
 * plan-memory extension
 *
 * 计划模式 + 计划落盘到长期文档：
 * - /plan 切换只读计划模式（禁用 edit/write，bash 走白名单）
 * - /plan-save [--rewrite] 保存计划（默认分区合并；--rewrite 全量重写）
 * - /plan-show 查看当前计划文档
 * - plan_record 工具：让模型编程式地把条目写入某个分区
 * - Ctrl+Alt+P 快捷切换
 *
 * 落盘位置：memory_palace/plan/{sessionId}.md
 * 分区：计划 / 待办实现 / 待确定事项 / act 阶段变动的文件列表 / 验证事项 / harness 优化项
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { StringEnum } from "@earendil-works/pi-ai";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Key } from "@earendil-works/pi-tui";
import { Type } from "typebox";

// ---------------------------------------------------------------------------
// 配置
// ---------------------------------------------------------------------------

const SECTION_KEYS = ["plan", "todo", "open", "actFiles", "verify", "harness"] as const;
type SectionKey = (typeof SECTION_KEYS)[number];

const SECTION_HEADINGS: Record<SectionKey, string> = {
  plan: "计划",
  todo: "待办实现",
  open: "待确定事项",
  actFiles: "act 阶段变动的文件列表",
  verify: "验证事项",
  harness: "harness 优化项",
};

const SECTION_ORDER: SectionKey[] = [...SECTION_KEYS];

// 工作流阶段（写入 frontmatter status）
const STAGES = {
  plan: "规划中",
  execute: "执行中",
  harness: "harness 优化中",
  done: "已完成",
} as const;
type StageKey = keyof typeof STAGES;

// 只读计划模式下要禁用的内置工具
const READONLY_DISABLED_TOOLS = new Set<string>(["edit", "write"]);

// ---------------------------------------------------------------------------
// bash 白名单（复用官方 plan-mode 思路）
// ---------------------------------------------------------------------------

const DESTRUCTIVE_PATTERNS = [
  /\brm\b/i, /\brmdir\b/i, /\bmv\b/i, /\bcp\b/i, /\bmkdir\b/i, /\btouch\b/i,
  /\bchmod\b/i, /\bchown\b/i, /\bchgrp\b/i, /\bln\b/i, /\btee\b/i, /\btruncate\b/i,
  /\bdd\b/i, /\bshred\b/i, /(^|[^<])>(?!>)/, />>/,
  /\bnpm\s+(install|uninstall|update|ci|link|publish)/i,
  /\byarn\s+(add|remove|install|publish)/i,
  /\bpnpm\s+(add|remove|install|publish)/i,
  /\bpip\s+(install|uninstall)/i,
  /\bapt(-get)?\s+(install|remove|purge|update|upgrade)/i,
  /\bbrew\s+(install|uninstall|upgrade)/i,
  /\bgit\s+(add|commit|push|pull|merge|rebase|reset|checkout|branch\s+-[dD]|stash|cherry-pick|revert|tag|init|clone)/i,
  /\bsudo\b/i, /\bsu\b/i, /\bkill\b/i, /\bpkill\b/i, /\bkillall\b/i,
  /\breboot\b/i, /\bshutdown\b/i,
  /\bsystemctl\s+(start|stop|restart|enable|disable)/i,
  /\bservice\s+\S+\s+(start|stop|restart)/i,
  /\b(vim?|nano|emacs|code|subl)\b/i,
];

const SAFE_PATTERNS = [
  /^\s*cat\b/, /^\s*head\b/, /^\s*tail\b/, /^\s*less\b/, /^\s*more\b/,
  /^\s*grep\b/, /^\s*find\b/, /^\s*ls\b/, /^\s*pwd\b/, /^\s*echo\b/,
  /^\s*printf\b/, /^\s*wc\b/, /^\s*sort\b/, /^\s*uniq\b/, /^\s*diff\b/,
  /^\s*file\b/, /^\s*stat\b/, /^\s*du\b/, /^\s*df\b/, /^\s*tree\b/,
  /^\s*which\b/, /^\s*whereis\b/, /^\s*type\b/, /^\s*env\b/, /^\s*printenv\b/,
  /^\s*uname\b/, /^\s*whoami\b/, /^\s*id\b/, /^\s*date\b/, /^\s*cal\b/,
  /^\s*uptime\b/, /^\s*ps\b/, /^\s*top\b/, /^\s*htop\b/, /^\s*free\b/,
  /^\s*git\s+(status|log|diff|show|branch|remote|config\s+--get)/i,
  /^\s*git\s+ls-/i,
  /^\s*npm\s+(list|ls|view|info|search|outdated|audit)/i,
  /^\s*yarn\s+(list|info|why|audit)/i,
  /^\s*node\s+--version/i, /^\s*python\s+--version/i,
  /^\s*curl\s/i, /^\s*wget\s+-O\s*-/i, /^\s*jq\b/,
  /^\s*sed\s+-n/i, /^\s*awk\b/, /^\s*rg\b/, /^\s*fd\b/, /^\s*bat\b/, /^\s*eza\b/,
];

function isSafeCommand(command: string): boolean {
  const destructive = DESTRUCTIVE_PATTERNS.some((p) => p.test(command));
  const safe = SAFE_PATTERNS.some((p) => p.test(command));
  return !destructive && safe;
}

// ---------------------------------------------------------------------------
// Markdown 文档解析 / 序列化（以 ## 为分区边界）
// ---------------------------------------------------------------------------

interface ParsedDoc {
  frontmatter: string | null;
  preamble: string;
  sections: Map<string, string>;
}

function parseDocument(text: string): ParsedDoc {
  let frontmatter: string | null = null;
  let rest = text;
  if (text.startsWith("---")) {
    const end = text.indexOf("\n---", 3);
    if (end !== -1) {
      frontmatter = text.slice(4, end).trim();
      rest = text.slice(end + 4).replace(/^\n+/, "");
    }
  }

  const sections = new Map<string, string>();
  let preamble = "";
  let currentHeading: string | null = null;
  let currentBody: string[] = [];

  for (const line of rest.split("\n")) {
    const m = line.match(/^##\s+(.+)$/);
    if (m) {
      if (currentHeading !== null) {
        sections.set(currentHeading, currentBody.join("\n").trim());
      }
      currentHeading = m[1].trim();
      currentBody = [];
    } else if (currentHeading === null) {
      preamble += line + "\n";
    } else {
      currentBody.push(line);
    }
  }
  if (currentHeading !== null) {
    sections.set(currentHeading, currentBody.join("\n").trim());
  }
  return { frontmatter, preamble: preamble.trim(), sections };
}

function orderedHeadings(sections: Map<string, string>): string[] {
  const canonical = SECTION_ORDER.map((k) => SECTION_HEADINGS[k]);
  const rest = [...sections.keys()].filter((h) => !canonical.includes(h));
  return [...canonical.filter((h) => sections.has(h)), ...rest];
}

function serializeDocument(doc: ParsedDoc): string {
  const parts: string[] = [];
  if (doc.frontmatter !== null) {
    parts.push(`---\n${doc.frontmatter.trim()}\n---`);
  }
  if (doc.preamble.trim()) {
    parts.push(doc.preamble.trim());
  }
  for (const heading of orderedHeadings(doc.sections)) {
    const body = doc.sections.get(heading)?.trim() ?? "";
    parts.push(`## ${heading}\n\n${body}`);
  }
  return parts.join("\n\n").trimEnd() + "\n";
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

/** 确保 frontmatter 含 session_id / date，保留其它字段。 */
const DEFAULT_STATUS = "规划中";

function updateFrontmatter(frontmatter: string | null, sessionId: string, status?: string): string {
  const fields = new Map<string, string>();
  if (frontmatter) {
    for (const line of frontmatter.split("\n")) {
      const m = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
      if (m) fields.set(m[1], m[2]);
    }
  }
  fields.set("session_id", sessionId);
  fields.set("date", today());
  if (status !== undefined) {
    fields.set("status", status);
  } else if (!fields.has("status")) {
    fields.set("status", DEFAULT_STATUS);
  }
  const ordered = [
    "session_id",
    "date",
    "status",
    ...Array.from(fields.keys()).filter((k) => !["session_id", "date", "status"].includes(k)),
  ];
  return ordered.map((k) => `${k}: ${fields.get(k)}`).join("\n");
}

// ---------------------------------------------------------------------------
// 文件路径与读写
// ---------------------------------------------------------------------------

function planFilePath(ctx: ExtensionContext): string {
  const sessionId = ctx.sessionManager.getSessionId();
  return path.join(ctx.cwd, "memory_palace", "plan", `${sessionId}.md`);
}

function readPlanFile(filePath: string): ParsedDoc | null {
  if (!fs.existsSync(filePath)) return null;
  return parseDocument(fs.readFileSync(filePath, "utf-8"));
}

function writePlanFile(filePath: string, doc: ParsedDoc): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, serializeDocument(doc), "utf-8");
}

function mergeSections(existing: ParsedDoc, incoming: ParsedDoc): ParsedDoc {
  const merged: ParsedDoc = {
    frontmatter: incoming.frontmatter !== null ? incoming.frontmatter : existing.frontmatter,
    preamble: existing.preamble,
    sections: new Map(existing.sections),
  };
  for (const k of SECTION_ORDER) {
    const heading = SECTION_HEADINGS[k];
    const body = incoming.sections.get(heading);
    if (body !== undefined) {
      merged.sections.set(heading, body.trim());
    }
  }
  return merged;
}

/** 条目按分区格式化：计划用编号，待办/验证用复选框，其余用列表。 */
function formatItem(key: SectionKey, index: number, item: string): string {
  const text = item.trim();
  switch (key) {
    case "plan":
      return `${index + 1}. ${text}`;
    case "todo":
    case "verify":
      return `- [ ] ${text}`;
    default:
      return `- ${text}`;
  }
}

function applySectionUpdate(
  doc: ParsedDoc,
  key: SectionKey,
  items: string[],
  mode: "append" | "replace",
): ParsedDoc {
  const heading = SECTION_HEADINGS[key];
  const existingBody = doc.sections.get(heading) ?? "";
  const existingLines = existingBody
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);

  const cleaned = items.map((s) => s.trim()).filter((s) => s.length > 0);
  let next: string[];
  if (mode === "replace") {
    next = cleaned.map((item, i) => formatItem(key, i, item));
  } else {
    const base = existingLines.length;
    const appended = cleaned.map((item, i) => formatItem(key, base + i, item));
    next = [...existingLines, ...appended];
  }
  doc.sections.set(heading, next.join("\n"));
  return doc;
}

/** 勾选/取消勾选复选框条目（- [ ] ↔ - [x]），按子串匹配。 */
function markSectionItems(
  doc: ParsedDoc,
  key: SectionKey,
  patterns: string[],
  done: boolean,
): number {
  const heading = SECTION_HEADINGS[key];
  const body = doc.sections.get(heading) ?? "";
  const target = done ? "x" : " ";
  let changed = 0;
  const newLines = body.split("\n").map((line) => {
    const m = line.match(/^(\s*- \[)([ x])(\].*)$/);
    if (!m) return line;
    const matched = patterns.some((p) => p && line.includes(p));
    if (!matched || m[2] === target) return line;
    changed++;
    return `${m[1]}${target}${m[3]}`;
  });
  if (changed > 0) {
    doc.sections.set(heading, newLines.join("\n"));
  }
  return changed;
}

// ---------------------------------------------------------------------------
// 扩展主体
// ---------------------------------------------------------------------------

export default function (pi: ExtensionAPI): void {
  let planModeEnabled = false;
  let toolsBeforePlanMode: string[] | undefined;

  function persistState(): void {
    pi.appendEntry("plan-memory", { enabled: planModeEnabled, toolsBeforePlanMode });
  }

  function enableReadonlyTools(): void {
    if (toolsBeforePlanMode === undefined) {
      toolsBeforePlanMode = pi.getActiveTools();
    }
    const readonlyTools = pi
      .getActiveTools()
      .filter((name) => !READONLY_DISABLED_TOOLS.has(name));
    pi.setActiveTools(readonlyTools);
  }

  function restoreTools(): void {
    pi.setActiveTools(toolsBeforePlanMode ?? pi.getActiveTools());
    toolsBeforePlanMode = undefined;
  }

  function togglePlanMode(ctx: ExtensionContext): void {
    planModeEnabled = !planModeEnabled;
    if (planModeEnabled) {
      enableReadonlyTools();
      ctx.ui.notify("只读计划模式已开启：edit/write 已禁用，bash 仅白名单。", "info");
    } else {
      restoreTools();
      ctx.ui.notify("只读计划模式已关闭，完整访问已恢复。", "info");
    }
    persistState();
  }

  // -------------------------------------------------------------------------
  // 命令
  // -------------------------------------------------------------------------

  pi.registerCommand("plan", {
    description: "切换只读计划模式（禁用 edit/write，bash 走白名单）",
    handler: async (_args, ctx) => togglePlanMode(ctx),
  });

  pi.registerCommand("plan-show", {
    description: "查看当前会话的计划文档",
    handler: async (_args, ctx) => {
      const filePath = planFilePath(ctx);
      if (!fs.existsSync(filePath)) {
        ctx.ui.notify("尚无计划文档，先用 /plan-save 或 plan_record 创建。", "info");
        return;
      }
      const text = fs.readFileSync(filePath, "utf-8");
      if (ctx.mode !== "tui") {
        ctx.ui.notify(`计划文档：${filePath}\n\n${text}`, "info");
        return;
      }
      await ctx.ui.editor(`计划文档（只读预览，可复制）${path.basename(filePath)}`, text);
    },
  });

  pi.registerCommand("plan-save", {
    description: "保存计划到 memory_palace/plan/{sessionId}.md（默认分区合并，--rewrite 全量重写）",
    handler: async (args, ctx) => {
      const rewrite = /\b(--rewrite|--full|-r)\b/.test(args);
      const filePath = planFilePath(ctx);
      const existing = readPlanFile(filePath);
      const sessionId = ctx.sessionManager.getSessionId();

      // 分区合并：只编辑 6 个受管分区
      if (!rewrite) {
        const prefillParts = [
          "---",
          updateFrontmatter(existing?.frontmatter ?? null, sessionId),
          "---",
          "",
          `# 计划文档（分区合并）→ ${filePath.replace(/\\/g, "/")}`,
          "",
        ];
        for (const k of SECTION_ORDER) {
          const heading = SECTION_HEADINGS[k];
          const body = existing?.sections.get(heading) ?? "";
          prefillParts.push(`## ${heading}\n\n${body}`);
        }
        const edited = await ctx.ui.editor("编辑计划分区（仅这 6 个分区会合并到文档）", prefillParts.join("\n\n"));
        if (edited === undefined) return;

        const incoming = parseDocument(edited);
        const base = existing ?? { frontmatter: null, preamble: "", sections: new Map<string, string>() };
        const merged = mergeSections(base, incoming);
        merged.frontmatter = updateFrontmatter(incoming.frontmatter ?? base.frontmatter, sessionId);
        writePlanFile(filePath, merged);
        ctx.ui.notify(`已分区合并保存：${filePath}`, "info");
        return;
      }

      // 全量重写：编辑整个文档，直接覆盖
      const prefill =
        existing !== null
          ? serializeDocument(existing)
          : `---\n${updateFrontmatter(null, sessionId)}\n---\n\n# 计划文档\n`;
      const edited = await ctx.ui.editor("编辑计划文档（全量重写）", prefill);
      if (edited === undefined) return;

      const doc = parseDocument(edited);
      doc.frontmatter = updateFrontmatter(doc.frontmatter, sessionId);
      writePlanFile(filePath, doc);
      ctx.ui.notify(`已全量重写保存：${filePath}`, "info");
    },
  });

  pi.registerCommand("plan-stage", {
    description: "标记计划工作流阶段：plan / execute / harness / done",
    handler: async (args, ctx) => {
      const stageArg = args.trim().toLowerCase();
      const aliases: Record<string, StageKey> = {
        plan: "plan", planning: "plan", "规划": "plan",
        execute: "execute", "执行": "execute",
        harness: "harness", "优化": "harness",
        done: "done", "完成": "done",
      };
      const stage = aliases[stageArg];
      if (!stage) {
        ctx.ui.notify("用法：/plan-stage <plan|execute|harness|done>", "error");
        return;
      }
      const filePath = planFilePath(ctx);
      const existing = readPlanFile(filePath);
      const sessionId = ctx.sessionManager.getSessionId();
      const doc: ParsedDoc = existing ?? {
        frontmatter: null,
        preamble: "# 计划文档",
        sections: new Map<string, string>(),
      };
      doc.frontmatter = updateFrontmatter(doc.frontmatter, sessionId, STAGES[stage]);
      writePlanFile(filePath, doc);
      ctx.ui.notify(`工作流阶段已更新为「${STAGES[stage]}」。`, "info");
    },
  });

  pi.registerShortcut(Key.ctrlAlt("p"), {
    description: "切换只读计划模式",
    handler: async (ctx) => togglePlanMode(ctx),
  });

  // -------------------------------------------------------------------------
  // plan_record 工具：模型编程式写入分区
  // -------------------------------------------------------------------------

  const PlanRecordParams = Type.Object({
    section: StringEnum(SECTION_KEYS),
    items: Type.Array(Type.String({ description: "要写入该分区的条目" })),
    mode: Type.Optional(StringEnum(["append", "replace"] as const)),
  });

  pi.registerTool({
    name: "plan_record",
    label: "Plan Record",
    description:
      "把条目写入长期计划文档 memory_palace/plan/{sessionId}.md 的某个分区。section 取值：plan=计划、todo=待办实现、open=待确定事项、actFiles=act 阶段变动的文件列表、verify=验证事项、harness=harness 优化项。mode 默认 append（追加），replace 为替换该分区。",
    parameters: PlanRecordParams,

    async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
      const key = params.section as SectionKey;
      const items = params.items ?? [];
      const mode = params.mode ?? "append";

      const filePath = planFilePath(ctx);
      const existing = readPlanFile(filePath);
      const sessionId = ctx.sessionManager.getSessionId();

      const doc: ParsedDoc = existing ?? {
        frontmatter: null,
        preamble: "# 计划文档",
        sections: new Map<string, string>(),
      };
      const updated = applySectionUpdate(doc, key, items, mode);
      updated.frontmatter = updateFrontmatter(updated.frontmatter, sessionId);
      writePlanFile(filePath, updated);

      const heading = SECTION_HEADINGS[key];
      const verb = mode === "replace" ? "替换" : "追加";
      return {
        content: [
          {
            type: "text",
            text: `已将 ${items.length} 条${verb}到「${heading}」分区：${filePath}`,
          },
        ],
        details: { section: key, mode, count: items.length, filePath },
      };
    },
  });

  const PlanCheckParams = Type.Object({
    section: StringEnum(["todo", "verify"] as const),
    patterns: Type.Array(Type.String({ description: "匹配条目文本的子串，命中即标记" })),
    done: Type.Optional(Type.Boolean({ description: "默认 true：标记完成 [x]；false 取消完成 [ ]" })),
  });

  pi.registerTool({
    name: "plan_check",
    label: "Plan Check",
    description:
      "标记计划文档中「待办实现 / 验证事项」复选框条目的完成状态。patterns 为子串列表，命中条目即标记；done 默认 true（[x]）。",
    parameters: PlanCheckParams,

    async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
      const key = params.section as SectionKey;
      const patterns = params.patterns ?? [];
      const done = params.done ?? true;

      const filePath = planFilePath(ctx);
      const existing = readPlanFile(filePath);
      if (!existing) {
        return {
          content: [{ type: "text", text: "尚无计划文档，先创建计划文件。" }],
          details: { section: key, changed: 0, filePath },
        };
      }
      const changed = markSectionItems(existing, key, patterns, done);
      const sessionId = ctx.sessionManager.getSessionId();
      existing.frontmatter = updateFrontmatter(existing.frontmatter, sessionId);
      writePlanFile(filePath, existing);

      const heading = SECTION_HEADINGS[key];
      const verb = done ? "完成" : "取消完成";
      return {
        content: [
          { type: "text", text: `「${heading}」分区已${verb} ${changed} 条：${filePath}` },
        ],
        details: { section: key, changed, filePath },
      };
    },
  });

  // -------------------------------------------------------------------------
  // 事件
  // -------------------------------------------------------------------------

  // 只读模式：阻断 edit/write，bash 走白名单
  pi.on("tool_call", async (event) => {
    if (!planModeEnabled) return;
    if (event.toolName === "edit" || event.toolName === "write") {
      return {
        block: true,
        reason: "只读计划模式：edit/write 已禁用。用 /plan 退出只读模式后再执行。",
      };
    }
    if (event.toolName === "bash") {
      const command = (event as { input: { command?: string } }).input.command ?? "";
      if (!isSafeCommand(command)) {
        return {
          block: true,
          reason: `只读计划模式：命令不在白名单。用 /plan 退出只读模式。\n命令: ${command}`,
        };
      }
    }
  });

  // 注入计划模式上下文
  pi.on("before_agent_start", async () => {
    if (!planModeEnabled) return;
    return {
      message: {
        customType: "plan-memory-context",
        content: `[只读计划模式 ACTIVE]
你处于只读计划模式：edit/write 已禁用，bash 仅允许只读白名单命令。

产出的计划与各类清单，可用 plan_record 工具写入长期文档 memory_palace/plan/{sessionId}.md 的对应分区：
- plan（计划）
- todo（待办实现）
- open（待确定事项）
- actFiles（act 阶段变动的文件列表）
- verify（验证事项）
- harness（harness 优化项）

请先只做分析、澄清与规划，不要尝试修改文件。`,
        display: false,
      },
    };
  });

  // 恢复状态并应用只读工具集
  pi.on("session_start", async (_event, ctx) => {
    const entries = ctx.sessionManager.getEntries();
    const saved = entries
      .filter(
        (e: { type: string; customType?: string }) =>
          e.type === "custom" && e.customType === "plan-memory",
      )
      .pop() as { data?: { enabled?: boolean; toolsBeforePlanMode?: string[] } } | undefined;

    if (saved?.data) {
      planModeEnabled = saved.data.enabled ?? planModeEnabled;
      toolsBeforePlanMode = saved.data.toolsBeforePlanMode ?? toolsBeforePlanMode;
    }
    if (planModeEnabled) {
      enableReadonlyTools();
      if (ctx.hasUI) {
        ctx.ui.notify("已恢复只读计划模式。", "info");
      }
    }
  });
}
