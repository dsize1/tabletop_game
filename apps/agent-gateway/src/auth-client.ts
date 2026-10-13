//! server auth HTTP 客户端：agent-gateway → server /auth/issue（签发短期 token）。

export interface IssuedToken {
  token: string;
  expires_in_secs: number;
}

/** 拼接计划文档短链（纯函数，便于单测）。自动去除 base 尾部斜杠。 */
export function buildPlanUrl(baseUrl: string, sessionId: string, token: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/dev_task/${sessionId}?token=${token}`;
}

/** 拼接开发文档预览短链（token 覆盖全部文档根 docs:all）。 */
export function buildDocsUrl(baseUrl: string, token: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/dev_docs?token=${token}`;
}

export class AuthClient {
  constructor(private baseUrl: string) {}

  /** 签发短期 token（绑定 openid + 资源范围）。 */
  async issueToken(openid: string, resource: string): Promise<IssuedToken> {
    const r = await fetch(`${this.baseUrl}/auth/issue`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ openid, resource }),
    });
    if (!r.ok) throw new Error(`auth/issue ${r.status}`);
    return (await r.json()) as IssuedToken;
  }
}
