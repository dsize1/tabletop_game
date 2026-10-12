// 短链拼接单测：buildPlanUrl（PUBLIC_BASE_URL + token 拼接正确）。
// 运行：npx tsx test-short-link.mts
import assert from "node:assert";
import { buildPlanUrl } from "./src/auth-client.js";

assert.equal(
  buildPlanUrl("https://tabletop.example.com", "abc-123", "tok-1"),
  "https://tabletop.example.com/dev_task/abc-123?token=tok-1",
);

// 尾部斜杠去除
assert.equal(
  buildPlanUrl("https://tabletop.example.com/", "abc-123", "tok-1"),
  "https://tabletop.example.com/dev_task/abc-123?token=tok-1",
);

// 多斜杠去除
assert.equal(
  buildPlanUrl("https://tabletop.example.com///", "x", "y"),
  "https://tabletop.example.com/dev_task/x?token=y",
);

// 子路径基址保留
assert.equal(
  buildPlanUrl("https://example.com/boardgame", "s", "t"),
  "https://example.com/boardgame/dev_task/s?token=t",
);

console.log("test-short-link: all passed");
