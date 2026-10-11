//! 开发任务子会话：pi-sdk AgentSession 的创建 / 恢复 / 执行。
//! 子会话只做「内容」（规划 / 执行 / 写计划文档），任务状态机由网关 + Rust 引擎驱动。
//! 子会话 id = 统一键 sessionId（网关预生成，经 SessionManager.create options.id 落定，
//! plan-memory 据此写 memory_palace/plan/{sessionId}.md，web 页以此标识）。

import {
  createAgentSession,
  DefaultResourceLoader,
  getAgentDir,
  ModelRuntime,
  SessionManager,
  type AgentSession,
} from "@earendil-works/pi-coding-agent";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, "../../..");
const SESSION_DIR = path.join(PROJECT_ROOT, "data", "dev-sessions");

const LLM_API_KEY = process.env.LLM_API_KEY ?? "";
const PROVIDER = "deepseek";
const MODEL_ID = process.env.DEV_LLM_MODEL ?? "deepseek-v4-pro";

interface Services {
  modelRuntime: ModelRuntime;
  model: any;
  resourceLoader: DefaultResourceLoader;
}

export interface SubSession {
  session: AgentSession;
  sessionId: string;
}

/** 子会话首轮 prompt：加载 plan-workflow 并进入只读规划阶段。 */
export function devTaskPrompt(requirement: string): string {
  return [
    "你是桌游助手项目的「开发任务执行者」。请先读取 .pi/skills/plan-workflow/SKILL.md 并严格遵循该计划工作流。",
    "",
    "开发需求：",
    requirement,
    "",
    "当前处于规划阶段（只读）。只做分析、澄清与规划：用 plan_record 工具把 6 分区计划（计划/待办实现/待确定事项/act 阶段变动的文件列表/验证事项/harness 优化项）写入计划文档，然后停下，向用户汇报计划摘要并请求确认。",
    "最终回复格式：第一行给出任务标题（格式「标题：xxx」），空一行后给计划摘要。",
    "未获用户明确确认前，禁止 edit/write 或执行任何修改。",
  ].join("\n");
}

export class DevSessionManager {
  private services: Services | null = null;
  private locks = new Map<string, Promise<void>>();

  private async ensureServices(): Promise<Services> {
    if (this.services) return this.services;
    if (!LLM_API_KEY) throw new Error("LLM_API_KEY 未配置，无法创建开发任务子会话");
    const modelRuntime = await ModelRuntime.create();
    await modelRuntime.setRuntimeApiKey(PROVIDER, LLM_API_KEY);
    const model = modelRuntime.getModel(PROVIDER, MODEL_ID);
    if (!model) throw new Error(`未找到模型 ${PROVIDER}/${MODEL_ID}`);
    const resourceLoader = new DefaultResourceLoader({
      cwd: PROJECT_ROOT,
      agentDir: getAgentDir(),
    });
    await resourceLoader.reload();
    this.services = { modelRuntime, model, resourceLoader };
    return this.services;
  }

  /** 创建新子会话（sessionId 由网关预生成）。 */
  async spawn(sessionId: string): Promise<SubSession> {
    const svc = await this.ensureServices();
    const { session } = await createAgentSession({
      cwd: PROJECT_ROOT,
      agentDir: getAgentDir(),
      modelRuntime: svc.modelRuntime,
      model: svc.model,
      thinkingLevel: "medium",
      resourceLoader: svc.resourceLoader,
      sessionManager: SessionManager.create(PROJECT_ROOT, SESSION_DIR, { id: sessionId }),
    });
    return { session, sessionId };
  }

  /** 恢复已有子会话（按 sessionId 在 SESSION_DIR 下定位 sessionFile）。 */
  async resume(sessionId: string): Promise<SubSession> {
    const svc = await this.ensureServices();
    const sessions = await SessionManager.list(PROJECT_ROOT, SESSION_DIR);
    const info = sessions.find((s) => s.id === sessionId);
    if (!info) throw new Error(`未找到子会话文件: ${sessionId}`);
    const { session } = await createAgentSession({
      cwd: PROJECT_ROOT,
      agentDir: getAgentDir(),
      modelRuntime: svc.modelRuntime,
      model: svc.model,
      thinkingLevel: "medium",
      resourceLoader: svc.resourceLoader,
      sessionManager: SessionManager.open(info.path),
    });
    return { session, sessionId };
  }

  /** 串行锁：同一子会话消息排队，避免并发 prompt。 */
  async withLock<T>(sessionId: string, fn: () => Promise<T>): Promise<T> {
    const prev = this.locks.get(sessionId) ?? Promise.resolve();
    const next = prev.then(fn, fn);
    this.locks.set(
      sessionId,
      next.then(
        () => {},
        () => {},
      ),
    );
    return next;
  }
}
