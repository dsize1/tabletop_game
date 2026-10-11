//! SystemOne 决策服务客户端（本地 HTTP `/v1/systemone`，StartLux-Decision）。
//! 只做「建议」，不写状态；超时/连不上/非 2xx 一律抛错，由调用方降级。
//! `fetchImpl` 可注入，便于单测（不依赖本地服务）。

export type QuestionType = "choice" | "noul" | "score";

export interface ChoiceQuestion {
  type: "choice";
  instructions: string;
  criteria: Record<string, string> | string[];
}
export interface NoulQuestion {
  type: "noul";
  instructions: string;
  criteria?: { true?: string; false?: string };
}
export interface ScoreQuestion {
  type: "score";
  instructions: string;
  criteria: string[] | Record<string, string>;
}
export type Question = ChoiceQuestion | NoulQuestion | ScoreQuestion;

export interface ChoiceAnswer {
  type: "choice";
  choice: string;
  confidence: number;
  probabilities: Record<string, number>;
}
export interface NoulAnswer {
  type: "noul";
  noul: number;
}
export interface ScoreAnswer {
  type: "score";
  score: number;
  confidence: number;
  legend: Record<string, string>;
  probabilities: Record<string, number>;
}
export type Answer = ChoiceAnswer | NoulAnswer | ScoreAnswer;

export interface SystemOneResponse {
  answers: Record<string, Answer>;
  usage?: { input_tokens: number; output_tokens: number };
  model?: string;
}

export interface SystemOneClientOptions {
  /** 服务根地址，如 http://127.0.0.1:8090（不含 /v1/systemone）。 */
  url: string;
  /** 单次请求超时（毫秒）。 */
  timeoutMs: number;
  /** 可注入的 fetch（默认全局 fetch），测试用。 */
  fetchImpl?: typeof fetch;
}

export class SystemOneClient {
  constructor(private readonly opts: SystemOneClientOptions) {}

  /** 调用 `/v1/systemone`：单次请求多问题（单次前向）。 */
  async decide(
    state: unknown,
    questions: Record<string, Question>,
  ): Promise<SystemOneResponse> {
    const base = this.opts.url.replace(/\/+$/, "");
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.opts.timeoutMs);
    try {
      const doFetch = this.opts.fetchImpl ?? fetch;
      const resp = await doFetch(`${base}/v1/systemone`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ state, questions }),
        signal: controller.signal,
      });
      if (!resp.ok) {
        const body = await resp.text().catch(() => "");
        throw new Error(`SystemOne HTTP ${resp.status}: ${body.slice(0, 200)}`);
      }
      return (await resp.json()) as SystemOneResponse;
    } finally {
      clearTimeout(timer);
    }
  }
}
