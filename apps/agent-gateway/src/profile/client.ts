//! server 内部 API 客户端（agent-gateway → server）：对话窗口读取 + 画像读写。
//!
//! 保持「SQLite 单写者 = server」：agent-gateway 不直连库，统一经 server `/internal/*`。
//! `fetchImpl` 可注入，便于单测（不依赖 server 在线）。

export interface ChatMessageRow {
  openid: string;
  group_id: string;
  role: string;
  content: string;
  created_at: string;
}

export interface UserProfile {
  openid: string;
  profile_text: string;
  distilled_at: string | null;
  updated_at: string;
}

export interface ProfileClientOptions {
  /** server 内部基址，如 http://127.0.0.1:3000 */
  baseUrl: string;
  fetchImpl?: typeof fetch;
}

export class ProfileClient {
  private readonly fetchImpl: typeof fetch;
  constructor(private readonly opts: ProfileClientOptions) {
    this.fetchImpl = opts.fetchImpl ?? fetch;
  }

  private base(): string {
    return this.opts.baseUrl.replace(/\/+$/, "");
  }

  /** 窗口内对话（`[from, to)`，ISO8601）。 */
  async fetchChatWindow(from: string, to: string): Promise<ChatMessageRow[]> {
    const u = new URL(`${this.base()}/internal/chat/window`);
    u.searchParams.set("from", from);
    u.searchParams.set("to", to);
    const r = await this.fetchImpl(u.toString());
    if (!r.ok) throw new Error(`chat/window ${r.status}`);
    return (await r.json()) as ChatMessageRow[];
  }

  /** 窗口内出现过的 openid（去重升序）。 */
  async fetchActiveOpenids(from: string, to: string): Promise<string[]> {
    const u = new URL(`${this.base()}/internal/chat/active_openids`);
    u.searchParams.set("from", from);
    u.searchParams.set("to", to);
    const r = await this.fetchImpl(u.toString());
    if (!r.ok) throw new Error(`chat/active_openids ${r.status}`);
    return (await r.json()) as string[];
  }

  /** 读画像（无 → null）。 */
  async getProfile(openid: string): Promise<UserProfile | null> {
    const r = await this.fetchImpl(
      `${this.base()}/internal/profile/${encodeURIComponent(openid)}`,
    );
    if (r.status === 404) return null;
    if (!r.ok) throw new Error(`profile ${r.status}`);
    return (await r.json()) as UserProfile;
  }

  /** upsert 画像（`distilled_at` 增量游标，`updated_at` 写入时刻，均 ISO8601）。 */
  async upsertProfile(
    openid: string,
    profileText: string,
    distilledAt: string,
    updatedAt: string,
  ): Promise<void> {
    const r = await this.fetchImpl(`${this.base()}/internal/profile`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        openid,
        profile_text: profileText,
        distilled_at: distilledAt,
        updated_at: updatedAt,
      }),
    });
    if (!r.ok) throw new Error(`profile upsert ${r.status}`);
  }
}
