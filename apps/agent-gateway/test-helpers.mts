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

/** 返回固定 P(true) 的 noul（是/否）桩客户端。 */
export function noulClient(probability: number): SystemOneClient {
  const fetchImpl = (async () =>
    new Response(
      JSON.stringify({ answers: { need_fetch: { type: "noul", noul: probability } } }),
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

/** 多问题桩：返回固定 answers，并记录每次请求体（用于断言「只发 1 次请求」）。 */
export function recordingClient(answers: Record<string, unknown>) {
  const calls: Array<{ state: Record<string, unknown>; questions: Record<string, unknown> }> = [];
  const fetchImpl = (async (_url: string, init?: RequestInit) => {
    calls.push(JSON.parse(String(init?.body)));
    return new Response(JSON.stringify({ answers }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  }) as unknown as typeof fetch;
  const client = new SystemOneClient({ url: "http://stub", timeoutMs: 1000, fetchImpl });
  return { client, calls };
}

/** 内存 ListStore 桩（聊天历史读写测试用）。 */
export function memListStore() {
  const m = new Map<string, string[]>();
  return {
    map: m,
    async lrange(key: string, start: number, stop: number) {
      const arr = m.get(key) ?? [];
      const n = arr.length;
      const from = start < 0 ? Math.max(0, n + start) : start;
      const to = stop < 0 ? n + stop : stop;
      return arr.slice(from, to + 1);
    },
    async rpush(key: string, ...values: string[]) {
      const arr = m.get(key) ?? [];
      arr.push(...values);
      m.set(key, arr);
      return arr.length;
    },
    async ltrim(key: string, start: number, stop: number) {
      const arr = m.get(key) ?? [];
      const n = arr.length;
      const from = start < 0 ? Math.max(0, n + start) : start;
      const to = stop < 0 ? n + stop : stop;
      m.set(key, arr.slice(from, to + 1));
      return "OK";
    },
  };
}

/** 内存 KVStore 桩（待落位话题边界测试用）。 */
export function memKVStore() {
  const m = new Map<string, string>();
  return {
    map: m,
    async get(key: string) {
      return m.has(key) ? m.get(key)! : null;
    },
    async set(key: string, value: string, _mode: "EX", _ttl: number) {
      m.set(key, value);
      return "OK";
    },
    async del(key: string) {
      return m.delete(key) ? 1 : 0;
    },
  };
}

/**
 * 出站守卫：断言文本里不含任何「内部标记」（时间块 / 话题切换边界 / 时间锚点）。
 * 时间只进 LLM 上下文，绝不进出站消息 —— 见 docs/modules/agent-gateway.md。
 */
export function assertNoInternalMarkers(text: string, label = "文本") {
  const bad: Array<[string, RegExp]> = [
    ["时间块", /\[时间\]/],
    ["话题切换边界", /---\s*话题切换\s*---/],
    ["时间锚点", /\[\d{2}-\d{2} \d{2}:\d{2}\]/],
    ["链接摘录标记", /\[链接内容摘录/],
  ];
  const hit = bad.find(([, re]) => re.test(text));
  if (hit) {
    throw new Error(`出站守卫失败：${label}含内部标记「${hit[0]}」：${text.slice(0, 200)}`);
  }
}
