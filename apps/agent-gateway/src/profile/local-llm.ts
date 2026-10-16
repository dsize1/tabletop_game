//! 本地 9B 蒸馏服务生命周期（ADR-0007 决策 13）：
//! 凌晨**按需启动** llama-server → 探活 → 生成 → **关闭**；
//! 关闭后用**进程序列**（PID 消失 + 端口释放 + 健康探针不可达）做**释放确认**，
//! 不做显存快照 diff。模型位置固定（不迁移、不进仓库），经 env 配置。
//!
//! 进程与探针均可注入，便于离线单测。

import { spawn as nodeSpawn } from "node:child_process";
import net from "node:net";

export interface LocalLlmConfig {
  /** 9B GGUF 绝对路径（不迁移） */
  modelPath: string;
  /** llama-server 可执行文件 */
  llamaBin: string;
  port: number;
  ngl: string | number;
  ctx: string | number;
  /** 启动健康探活总超时（ms），默认 120s（大模型加载慢） */
  startTimeoutMs?: number;
  /** 优雅退出等待（ms），超时强杀，默认 10s */
  stopTimeoutMs?: number;
}

export interface ProcessHandle {
  pid?: number;
  exitCode: number | null;
  kill(signal?: NodeJS.Signals): boolean;
  once(event: "exit", cb: (code: number | null) => void): void;
}

export type SpawnFn = (cmd: string, args: string[]) => ProcessHandle;

export interface ReleaseProbe {
  pidAlive(pid: number): Promise<boolean>;
  portOpen(port: number): Promise<boolean>;
}

export interface LocalLlmDeps {
  spawnFn?: SpawnFn;
  probe?: ReleaseProbe;
  fetchImpl?: typeof fetch;
  sleep?: (ms: number) => Promise<void>;
  now?: () => number;
}

export interface LlmMessage {
  role: string;
  content: string;
}

export interface ReleaseCheck {
  released: boolean;
  detail: string;
}

const defaultSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

const defaultSpawn: SpawnFn = (cmd, args) => {
  const c = nodeSpawn(cmd, args, { stdio: ["ignore", "pipe", "pipe"] });
  c.stdout?.on("data", (b) => process.stdout.write(`[profile-llm] ${b}`));
  c.stderr?.on("data", (b) => process.stderr.write(`[profile-llm] ${b}`));
  return c as unknown as ProcessHandle;
};

const defaultProbe: ReleaseProbe = {
  async pidAlive(pid: number): Promise<boolean> {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  },
  async portOpen(port: number): Promise<boolean> {
    return new Promise((resolve) => {
      const s = net.connect({ host: "127.0.0.1", port });
      const done = (v: boolean) => {
        s.destroy();
        resolve(v);
      };
      s.once("connect", () => done(true));
      s.once("error", () => done(false));
      s.setTimeout(1000, () => done(false));
    });
  },
};

/** llama-server 参数（纯函数，便于单测）。 */
/** 是否禁用 9B 的思考（默认 **不禁用**：让它思考完再输出正文）。 */
const PROFILE_LLM_DISABLE_THINKING =
  (process.env.PROFILE_LLM_DISABLE_THINKING ?? "0") === "1";

/** 单次生成的 token 上限（思考 + 正文；思考型模型需足够大）。 */
const PROFILE_LLM_MAX_TOKENS = Number(process.env.PROFILE_LLM_MAX_TOKENS ?? 4096);

export function buildLlamaArgs(cfg: LocalLlmConfig): string[] {
  return [
    "-m",
    cfg.modelPath,
    "-ngl",
    String(cfg.ngl),
    "-c",
    String(cfg.ctx),
    "--port",
    String(cfg.port),
  ];
}

export class LocalLlm {
  private child: ProcessHandle | null = null;
  private readonly spawnFn: SpawnFn;
  private readonly probe: ReleaseProbe;
  private readonly fetchImpl: typeof fetch;
  private readonly sleep: (ms: number) => Promise<void>;
  private readonly now: () => number;

  constructor(
    private readonly cfg: LocalLlmConfig,
    deps: LocalLlmDeps = {},
  ) {
    this.spawnFn = deps.spawnFn ?? defaultSpawn;
    this.probe = deps.probe ?? defaultProbe;
    this.fetchImpl = deps.fetchImpl ?? fetch;
    this.sleep = deps.sleep ?? defaultSleep;
    this.now = deps.now ?? Date.now;
  }

  private healthUrl(): string {
    return `http://127.0.0.1:${this.cfg.port}/health`;
  }

  private async healthOk(): Promise<boolean> {
    try {
      const r = await this.fetchImpl(this.healthUrl(), { method: "GET" });
      return r.ok;
    } catch {
      return false;
    }
  }

  /** 启动 llama-server 并等待健康（幂等：已在运行则直接返回）。 */
  async start(): Promise<void> {
    if (this.child && this.child.exitCode === null) return;
    const args = buildLlamaArgs(this.cfg);
    console.log(
      `[profile-llm] 启动 llama-server（model=${this.cfg.modelPath} port=${this.cfg.port}）`,
    );
    this.child = this.spawnFn(this.cfg.llamaBin, args);
    const timeout = this.cfg.startTimeoutMs ?? 120_000;
    const deadline = this.now() + timeout;
    while (this.now() < deadline) {
      if (await this.healthOk()) {
        console.log(`[profile-llm] 就绪（pid=${this.child.pid ?? "-"}）`);
        return;
      }
      if (this.child.exitCode !== null) {
        throw new Error(`llama-server 提前退出 code=${this.child.exitCode}`);
      }
      await this.sleep(500);
    }
    throw new Error(`llama-server 健康探活超时（${timeout}ms）`);
  }

  /** 调 OpenAI 兼容 chat completions（llama-server 提供）。思考型模型：先思考后输出正文。 */
  async generate(messages: LlmMessage[], maxTokens = PROFILE_LLM_MAX_TOKENS): Promise<string> {
    const r = await this.fetchImpl(`http://127.0.0.1:${this.cfg.port}/v1/chat/completions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        model: "profile",
        messages,
        temperature: Number(process.env.PROFILE_LLM_TEMPERATURE ?? 0.2),
        max_tokens: maxTokens,
        stream: false,
        // 该 9B 为思考型：不禁思考时 content 为空（内容全在 reasoning_content）
        ...(PROFILE_LLM_DISABLE_THINKING
          ? { chat_template_kwargs: { enable_thinking: false } }
          : {}),
      }),
    });
    if (!r.ok) throw new Error(`/v1/chat/completions ${r.status}`);
    const data = (await r.json()) as {
      choices?: Array<{ message?: { content?: string; reasoning_content?: string } }>;
    };
    const msg = data.choices?.[0]?.message;
    const content = msg?.content?.trim() ?? "";
    // 兼底：思考型模型若正文为空（思考未完/被截），退而取思考内容拼结果
    if (!content && msg?.reasoning_content) return msg.reasoning_content.trim();
    return content;
  }

  private async waitExit(child: ProcessHandle, timeoutMs: number): Promise<void> {
    const start = this.now();
    while (child.exitCode === null && this.now() - start < timeoutMs) {
      await this.sleep(100);
    }
  }

  /** 关闭并做释放确认（PID 消失 + 端口释放 + 健康探针不可达）。 */
  async stop(): Promise<ReleaseCheck> {
    const child = this.child;
    if (!child) return { released: true, detail: "无运行中的进程" };
    const pid = child.pid;
    if (child.exitCode === null) {
      child.kill("SIGTERM");
      await this.waitExit(child, this.cfg.stopTimeoutMs ?? 10_000);
      if (child.exitCode === null) {
        console.warn(`[profile-llm] 优雅退出超时，强杀 pid=${pid ?? "-"}`);
        child.kill("SIGKILL");
        await this.waitExit(child, 5_000);
      }
    }
    return this.verifyReleased(pid);
  }

  /** 释放确认：PID 消失 + 端口释放 + 健康探针不可达。 */
  async verifyReleased(pid?: number): Promise<ReleaseCheck> {
    let released = true;
    const parts: string[] = [];
    if (typeof pid === "number") {
      const alive = await this.probe.pidAlive(pid);
      if (alive) {
        released = false;
        parts.push(`PID ${pid} 仍存活`);
      } else {
        parts.push(`PID ${pid} 已退出`);
      }
    }
    const open = await this.probe.portOpen(this.cfg.port);
    if (open) {
      released = false;
      parts.push(`端口 ${this.cfg.port} 仍监听`);
    } else {
      parts.push(`端口 ${this.cfg.port} 已释放`);
    }
    const healthy = await this.healthOk();
    if (healthy) {
      released = false;
      parts.push("健康探针仍可达");
    } else {
      parts.push("健康探针不可达");
    }
    this.child = null;
    return { released, detail: parts.join("；") };
  }
}

/** 起→用→停（无论成功失败都关闭并做释放确认）。 */
export async function withLocalLlm<T>(
  llm: LocalLlm,
  fn: (llm: LocalLlm) => Promise<T>,
): Promise<T> {
  await llm.start();
  try {
    return await fn(llm);
  } finally {
    const check = await llm.stop();
    if (!check.released) {
      console.error(`[profile-llm] ⚠️ 释放确认未通过：${check.detail}`);
    } else {
      console.log(`[profile-llm] 释放确认通过：${check.detail}`);
    }
  }
}
