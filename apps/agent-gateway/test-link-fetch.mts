// 链接抓取单测：链接提取 + noul（是/否）判断（注入桩，不依赖网络）+ URL 安全 + HTML→文本。
// 运行：npx tsx test-link-fetch.mts
import assert from "node:assert";
import { extractHttpsLinks, decideLinkFetch } from "./src/decision.js";
import { isSafePublicHttpsUrl, htmlToText } from "./src/link-fetch.js";
import { noulClient, failingClient } from "./test-helpers.mts";

// —— 链接提取：只认 https://（必须有协议头），剔除尾部标点，去重 ——
assert.deepEqual(
  extractHttpsLinks("看这个 https://example.com/a?b=1，还有 https://example.com/a?b=1"),
  ["https://example.com/a?b=1"],
);
assert.deepEqual(extractHttpsLinks("http://example.com 不算"), []);
assert.deepEqual(extractHttpsLinks("见 https://a.cn/x。"), ["https://a.cn/x"]);
assert.deepEqual(extractHttpsLinks("没有链接"), []);

// —— URL 安全：拒绝 http / 本机 / 内网字面量，允许公网 https ——
assert.equal(isSafePublicHttpsUrl("https://example.com/x"), true);
assert.equal(isSafePublicHttpsUrl("http://example.com/x"), false);
assert.equal(isSafePublicHttpsUrl("https://localhost/x"), false);
assert.equal(isSafePublicHttpsUrl("https://127.0.0.1/x"), false);
assert.equal(isSafePublicHttpsUrl("https://192.168.1.1/x"), false);
assert.equal(isSafePublicHttpsUrl("https://10.0.0.5/x"), false);
assert.equal(isSafePublicHttpsUrl("https://172.16.0.1/x"), false);
assert.equal(isSafePublicHttpsUrl("https://169.254.1.1/x"), false);
assert.equal(isSafePublicHttpsUrl("https://user:pass@example.com/x"), false);

// —— HTML → 文本：去脚本/样式/标签、解码实体 ——
const txt = htmlToText(
  "<html><head><title>T</title><style>a{}</style></head><body><script>x()</script><p>你好&amp;世界</p></body></html>",
);
assert.ok(txt.includes("你好&世界"), txt);
assert.ok(!txt.includes("x()"));
assert.ok(!txt.includes("a{}"));

// —— noul 判断：高 P(true) → 抓取 ——
{
  const d = await decideLinkFetch("帮我看看 https://example.com 讲了啥", ["https://example.com"], noulClient(0.9));
  assert.equal(d.fetch, true);
  assert.equal(d.probability, 0.9);
}

// —— noul 判断：低 P(true) → 不抓取 ——
{
  const d = await decideLinkFetch("分享个链接 https://example.com", ["https://example.com"], noulClient(0.1));
  assert.equal(d.fetch, false);
}

// —— 阈值可配 ——
{
  const d = await decideLinkFetch("x", ["https://e.com"], noulClient(0.5), { threshold: 0.6 });
  assert.equal(d.fetch, false);
}

// —— 服务不可用/超时 → 安全降级为不抓取（不抛错）——
{
  const d = await decideLinkFetch("x", ["https://e.com"], failingClient());
  assert.equal(d.fetch, false);
}

console.log("✅ link-fetch 单测全部通过");
