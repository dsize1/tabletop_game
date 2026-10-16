// 本地 9B 生命周期单测：参数构造 + 启动/生成/关闭 + 释放确认（进程序列，注入桩）。
// 运行：npx tsx test-profile-local-llm.mts
import assert from "node:assert";
import {
  LocalLlm,
  buildLlamaArgs,
  type ProcessHandle,
  type ReleaseProbe,
} from "./src/profile/local-llm.js";

const cfg = {
  modelPath: "/m/x.gguf",
  llamaBin: "llama-server",
  port: 8082,
  ngl: "99",
  ctx: "8192",
};

assert.deepEqual(buildLlamaArgs(cfg), [
  "-m",
  "/m/x.gguf",
  "-ngl",
  "99",
  "-c",
  "8192",
  "--port",
  "8082",
]);

// 起 → 生成 → 停：释放确认通过
{
  let state = { pid: true, port: true };
  const events: string[] = [];
  const child: ProcessHandle = {
    pid: 4242,
    exitCode: null,
    kill(sig) {
      events.push(`kill:${sig ?? "default"}`);
      state = { pid: false, port: false }; // 模拟进程已退出、端口释放
      (child as { exitCode: number | null }).exitCode = 0;
      return true;
    },
    once() {},
  };
  const probe: ReleaseProbe = {
    async pidAlive() {
      return state.pid;
    },
    async portOpen() {
      return state.port;
    },
  };
  const llm = new LocalLlm(cfg, {
    spawnFn: () => child,
    probe,
    sleep: async () => {},
    fetchImpl: (async (url: string) => {
      if (String(url).includes("/health")) {
        if (!state.port) throw new Error("ECONNREFUSED");
        return { ok: true };
      }
      return { ok: true, json: async () => ({ choices: [{ message: { content: " 事实 " } }] }) };
    }) as unknown as typeof fetch,
  });
  await llm.start();
  assert.equal(await llm.generate([{ role: "user", content: "hi" }]), "事实");
  const check = await llm.stop();
  assert.ok(events.includes("kill:SIGTERM"), "应先优雅退出");
  assert.equal(check.released, true, check.detail);
  assert.ok(check.detail.includes("PID 4242 已退出"), check.detail);
  assert.ok(check.detail.includes("端口 8082 已释放"), check.detail);
  assert.ok(check.detail.includes("健康探针不可达"), check.detail);
}

// 释放确认未通过：探针显示 PID/端口仍在
{
  const child: ProcessHandle = {
    pid: 7,
    exitCode: null,
    kill() {
      (child as { exitCode: number | null }).exitCode = 0;
      return true;
    },
    once() {},
  };
  const probe: ReleaseProbe = {
    async pidAlive() {
      return true;
    },
    async portOpen() {
      return true;
    },
  };
  const llm = new LocalLlm(cfg, {
    spawnFn: () => child,
    probe,
    sleep: async () => {},
    fetchImpl: (async () => ({ ok: true })) as unknown as typeof fetch,
  });
  await llm.start();
  const check = await llm.stop();
  assert.equal(check.released, false, "仍有残留应判未释放");
  assert.ok(check.detail.includes("仍存活"));
}

console.log("✅ profile-local-llm 单测全部通过");
