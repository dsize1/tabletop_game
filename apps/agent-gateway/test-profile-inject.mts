// 画像注入单测：ProfileCache TTL/负缓存/失败静默 + buildProfileBlock。
// 运行：npx tsx test-profile-inject.mts
import assert from "node:assert";
import {
  PROFILE_BLOCK_HEADER,
  ProfileCache,
  buildProfileBlock,
} from "./src/profile/inject.js";

// TTL 缓存 + 负缓存
{
  let calls = 0;
  let now = 1000;
  const src = {
    async getProfile(openid: string) {
      calls++;
      return openid === "u1"
        ? { openid, profile_text: "  喜欢竞技  ", distilled_at: null, updated_at: "x" }
        : null;
    },
  };
  const cache = new ProfileCache(src, 500, () => now);
  assert.equal(await cache.get("u1"), "喜欢竞技", "trim 后返回");
  assert.equal(await cache.get("u1"), "喜欢竞技");
  assert.equal(calls, 1, "TTL 内命中缓存");
  assert.equal(await cache.get("u2"), undefined);
  assert.equal(calls, 2, "无画像也缓存（负缓存）");
  now = 2000; // 过期
  await cache.get("u1");
  assert.equal(calls, 3, "过期后重取");
  // invalidate 立即失效
  cache.invalidate("u1");
  await cache.get("u1");
  assert.equal(calls, 4);
}

// 失败静默（不抛）
{
  const cache = new ProfileCache(
    {
      async getProfile() {
        throw new Error("server down");
      },
    },
    1000,
  );
  assert.equal(await cache.get("u1"), undefined);
}

// buildProfileBlock
assert.deepEqual(buildProfileBlock(undefined), []);
assert.deepEqual(buildProfileBlock(""), []);
{
  const blk = buildProfileBlock("喜欢竞技");
  assert.equal(blk.length, 1);
  assert.equal(blk[0]!.role, "system");
  assert.ok(blk[0]!.content.startsWith(PROFILE_BLOCK_HEADER));
  assert.ok(blk[0]!.content.includes("喜欢竞技"));
}

console.log("✅ profile-inject 单测全部通过");
