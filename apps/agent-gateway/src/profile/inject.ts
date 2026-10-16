//! 画像注入（全局层，ADR-0007 决策 9）：带 TTL 内存缓存，避免每轮打 server。
//!
//! 画像变化频率低（日批），故缓存 TTL 取分钟级即可；无画像时跳过注入。

import type { UserProfile } from "./client.js";

/** 画像数据源（ProfileClient 结构兼容；测试可注入桩）。 */
export interface ProfileSource {
  getProfile(openid: string): Promise<UserProfile | null>;
}

interface CacheEntry {
  /** 已 trim 的画像文本（无画像时为空串，用于负缓存） */
  text: string;
  exp: number;
}

export class ProfileCache {
  private readonly map = new Map<string, CacheEntry>();

  constructor(
    private readonly src: ProfileSource,
    private readonly ttlMs: number,
    private readonly now: () => number = Date.now,
  ) {}

  /** 取画像文本（无 → undefined）。失败静默返回 undefined（不阻塞主流程）。 */
  async get(openid: string): Promise<string | undefined> {
    const t = this.now();
    const hit = this.map.get(openid);
    if (hit && hit.exp > t) return hit.text || undefined;
    try {
      const p = await this.src.getProfile(openid);
      const text = (p?.profile_text ?? "").trim();
      this.map.set(openid, { text, exp: t + this.ttlMs });
      return text || undefined;
    } catch (err) {
      console.error("[agent-gateway] 取画像失败，本轮不注入:", err);
      return undefined;
    }
  }

  /** 使缓存失效（日批写回后调用，下一轮立即生效）。 */
  invalidate(openid: string): void {
    this.map.delete(openid);
  }

  invalidateAll(): void {
    this.map.clear();
  }
}

/** 画像作为独立 system 块的头部（内部标记，不出站）。 */
export const PROFILE_BLOCK_HEADER = "[人物画像]";

/** 组装画像 system 块（无画像 → 空数组）。 */
export function buildProfileBlock(
  profileText: string | undefined,
): Array<{ role: "system"; content: string }> {
  if (!profileText) return [];
  return [{ role: "system", content: `${PROFILE_BLOCK_HEADER}\n${profileText}` }];
}
