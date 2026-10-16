// 意图确认单测：内存存根（不依赖 Redis）+ 词表匹配 + 存取/覆盖语义。
// 运行：npx tsx test-intent-confirm.mts
import assert from "node:assert";
import {
  clearPending,
  loadPending,
  matchConfirmReply,
  savePending,
  intentPendingKey,
  DEFAULT_CONFIRM_TTL_SEC,
  type ConfirmStore,
  type PendingIntent,
} from "./src/intent-confirm.js";

/** 内存 ConfirmStore 桩（记录 TTL，便于断言）。 */
function memStore(): ConfirmStore & { data: Map<string, string>; ttls: Map<string, number> } {
  const data = new Map<string, string>();
  const ttls = new Map<string, number>();
  return {
    data,
    ttls,
    async get(k) {
      return data.get(k) ?? null;
    },
    async set(k, v, _mode, ttl) {
      data.set(k, v);
      ttls.set(k, ttl);
      return "OK";
    },
    async del(k) {
      data.delete(k);
      ttls.delete(k);
      return 1;
    },
  };
}

// —— 词表匹配：严格整句（trim + 去尾部标点 + 小写）——
assert.equal(matchConfirmReply("y"), "yes");
assert.equal(matchConfirmReply(" Y "), "yes");
assert.equal(matchConfirmReply("yep!"), "yes");
assert.equal(matchConfirmReply("是的。"), "yes");
assert.equal(matchConfirmReply("n"), "no");
assert.equal(matchConfirmReply("不用"), "no");
assert.equal(matchConfirmReply("是，继续"), null); // 长句不判确认
assert.equal(matchConfirmReply("来玩猜拳 布"), null);
assert.equal(matchConfirmReply(""), null);
assert.equal(matchConfirmReply("   "), null);

// —— 存取/清除 + 按 (群, openid) 隔离 ——
const s = memStore();
assert.equal(await loadPending(s, "g1", "u1"), null);
const p: PendingIntent = {
  intent: "dev",
  confidence: 0.72,
  content: "帮评估下",
  cid: "msg-a",
  reasked: false,
  ts: 1,
};
await savePending(s, "g1", "u1", p);
assert.equal(s.ttls.get(intentPendingKey("g1", "u1")), DEFAULT_CONFIRM_TTL_SEC);
assert.deepEqual(await loadPending(s, "g1", "u1"), p);
assert.equal(await loadPending(s, "g1", "u2"), null); // 不同人互不影响
assert.equal(await loadPending(s, "g2", "u1"), null); // 不同群互不影响
await clearPending(s, "g1", "u1");
assert.equal(await loadPending(s, "g1", "u1"), null);

// —— 同 key 后写覆盖前写（同 openid 覆盖语义）——
await savePending(s, "g1", "u1", p);
await savePending(s, "g1", "u1", { ...p, intent: "game_action", confidence: 0.7 });
assert.equal((await loadPending(s, "g1", "u1"))?.intent, "game_action");

// —— 脏数据 → null（不抛）——
s.data.set(intentPendingKey("g9", "u9"), "{not json");
assert.equal(await loadPending(s, "g9", "u9"), null);

console.log("✅ intent-confirm 单测全部通过");
