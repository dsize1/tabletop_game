// SystemOneClient 客户端层单测：非 2xx 抛错、超时 abort、URL 归一化。
// 运行：npx tsx test-systemone.mts
import assert from "node:assert";
import { SystemOneClient } from "./src/systemone.js";
import { httpErrorClient, hangingClient } from "./test-helpers.mts";

const noulQ = { q: { type: "noul" as const, instructions: "x" } };

// 非 2xx → 抛错（含状态码）
await assert.rejects(() => httpErrorClient(500).decide({}, noulQ), /HTTP 500/);
await assert.rejects(() => httpErrorClient(404).decide({}, noulQ), /HTTP 404/);

// 超时 → abort 抛错（不应挂死）
await assert.rejects(() => hangingClient(200).decide({}, noulQ));

// URL 末尾斜杠归一化：不产生 //v1/systemone
{
  let seen = "";
  const fetchImpl = (async (url: string) => {
    seen = String(url);
    return new Response(JSON.stringify({ answers: {} }), { status: 200 });
  }) as unknown as typeof fetch;
  const c = new SystemOneClient({ url: "http://x:8090/", timeoutMs: 1000, fetchImpl });
  await c.decide({}, noulQ);
  assert.equal(seen, "http://x:8090/v1/systemone");
}

console.log("✅ systemone 客户端单测全部通过");
