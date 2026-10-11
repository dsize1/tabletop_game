// 决策层测试桩：可注入的 SystemOneClient，避免单测依赖本地决策服务。
// 供 test-decision.mts / test-systemone.mts 复用。
import { SystemOneClient } from "./src/systemone.js";

/** 返回固定 choice/confidence 的桩客户端。 */
export function stubClient(choice: string, confidence: number): SystemOneClient {
  const fetchImpl = (async () =>
    new Response(
      JSON.stringify({
        answers: {
          intent: { type: "choice", choice, confidence, probabilities: { [choice]: confidence } },
        },
      }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    )) as unknown as typeof fetch;
  return new SystemOneClient({ url: "http://stub", timeoutMs: 1000, fetchImpl });
}

/** 请求即失败的桩客户端（模拟连不上）。 */
export function failingClient(): SystemOneClient {
  const fetchImpl = (async () => {
    throw new Error("ECONNREFUSED");
  }) as unknown as typeof fetch;
  return new SystemOneClient({ url: "http://stub", timeoutMs: 1000, fetchImpl });
}

/** 返回指定 HTTP 状态码的桩客户端（模拟非 2xx）。 */
export function httpErrorClient(status: number, body = "server error"): SystemOneClient {
  const fetchImpl = (async () => new Response(body, { status })) as unknown as typeof fetch;
  return new SystemOneClient({ url: "http://stub", timeoutMs: 1000, fetchImpl });
}

/** 永不响应、直到超时被 abort 的桩客户端（模拟超时）。 */
export function hangingClient(timeoutMs: number): SystemOneClient {
  const fetchImpl = ((_url: string, init?: RequestInit) =>
    new Promise((_resolve, reject) => {
      init?.signal?.addEventListener("abort", () =>
        reject(Object.assign(new Error("aborted"), { name: "AbortError" })),
      );
    })) as unknown as typeof fetch;
  return new SystemOneClient({ url: "http://stub", timeoutMs, fetchImpl });
}
