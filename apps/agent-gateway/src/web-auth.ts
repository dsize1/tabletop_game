// Web 跨端登录（ADR-0031）：网关侧命令解析 + server 内部 API 客户端。
//
// 分工：
// - 网关只做「显式命令 → server 内部接口」的确定性映射（禁止自由文本猜意图）；
// - 登录会话/注册码/验证码状态全部在 server（唯一写者）；
// - 群消息里只出现「登录页链接」（短效 token），不出现正式 token。

export type AuthCommand =
  | { cmd: "verify"; code: string }
  | { cmd: "web" }
  | { cmd: "props" }
  | { cmd: "reset" };

/** 解析跨端登录相关显式命令；非命令返回 null。 */
export function parseAuthCommand(content: string): AuthCommand | null {
  const t = content.trim();

  // `#验证码#1234`（也接受 `#验证码 1234` / 全角井号）
  let m = t.match(/^#验证码\s*[#＃]?\s*(\d{4})$/);
  if (m) return { cmd: "verify", code: m[1] };
  m = t.match(/^#(网页|我的链接|登录)$/);
  if (m) return { cmd: "web" };
  if (/^#中台$/.test(t)) return { cmd: "props" };
  if (/^#(重置注册码|重置密码)$/.test(t)) return { cmd: "reset" };
  return null;
}

/** 登录页短链（与游戏页同源同前缀）。 */
export function buildLoginUrl(baseUrl: string, token: string): string {
  const base = baseUrl.replace(/\/+$/, "");
  return `${base}/login?t=${encodeURIComponent(token)}`;
}

export interface LinkRequest {
  group_id: string;
  openid: string;
  display_name?: string;
  scope: "game" | "props";
  target: string;
  room_id?: string;
  mode?: "login" | "reset";
}

export interface LinkResult {
  token: string;
  expires_in_secs: number;
  stage: string;
  registered: boolean;
}

export type VerifyResult = "confirmed" | "mismatch" | "unknown";

/** server `/internal/auth/web/*` 客户端（仅本机）。 */
export class WebAuthClient {
  constructor(private baseUrl: string) {}

  /** 申请短效登录链接；server 侧校验 scope=props 的白名单。 */
  async issueLink(req: LinkRequest): Promise<LinkResult> {
    const r = await fetch(`${this.baseUrl}/internal/auth/web/link`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(req),
    });
    if (!r.ok) {
      const text = await r.text().catch(() => "");
      throw new Error(`auth/web/link ${r.status} ${text}`.trim());
    }
    return (await r.json()) as LinkResult;
  }

  /** 群内验证码判定：码 + 回复人 openid。 */
  async verifyCode(groupId: string, code: string, openid: string): Promise<VerifyResult> {
    const r = await fetch(`${this.baseUrl}/internal/auth/web/verify`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ group_id: groupId, code, openid }),
    });
    if (!r.ok) throw new Error(`auth/web/verify ${r.status}`);
    const body = (await r.json()) as { result?: VerifyResult };
    return body.result ?? "unknown";
  }
}

/** 群内「链接列表」的单行格式（昵称 + 链接）。 */
export function formatLinkLine(name: string, url: string): string {
  return `· ${name}：${url}`;
}
