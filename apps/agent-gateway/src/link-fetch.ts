//! 链接内容抓取（Plan B 单跳）：仅 https、防 SSRF 字面量、手动跟随重定向、抽纯文本、限长。
//! 仅用于「聊天判断为需要查询链接」后抓取一次正文注入上下文；失败一律返回 null（静默降级）。

export interface FetchLinkOptions {
  /** 单次请求超时（毫秒，覆盖整个重定向链） */
  timeoutMs: number;
  /** 抽取文本的最大字符数 */
  maxChars: number;
}

const DEFAULT_OPTS: FetchLinkOptions = { timeoutMs: 10000, maxChars: 2000 };
const MAX_REDIRECTS = 3;

/** 是否是可安全抓取的 https URL：https + 无凭据 + 主机不是本机/内网字面量。 */
export function isSafePublicHttpsUrl(raw: string): boolean {
  let u: URL;
  try {
    u = new URL(raw);
  } catch {
    return false;
  }
  if (u.protocol !== "https:") return false;
  if (u.username || u.password) return false;
  const host = u.hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (!host) return false;
  if (host === "localhost" || host.endsWith(".localhost") || host.endsWith(".local")) return false;
  if (host === "::1" || host === "0:0:0:0:0:0:0:1") return false;
  const m = host.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (m) {
    const a = Number(m[1]);
    const b = Number(m[2]);
    if (a === 0 || a === 10 || a === 127) return false;
    if (a === 192 && b === 168) return false;
    if (a === 172 && b >= 16 && b <= 31) return false;
    if (a === 169 && b === 254) return false;
    if (a >= 224) return false; // 组播/保留
  }
  return true;
}

/** 粗暴 HTML → 文本：去注释/脚本/样式/标签、解码常见实体、压缩空白。 */
export function htmlToText(html: string): string {
  return html
    .replace(/<!--[\s\S]*?-->/g, " ")
    .replace(/<(script|style|noscript|svg|head|template)[\s\S]*?<\/\1>/gi, " ")
    .replace(/<[^>]*>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/&quot;/gi, '"')
    .replace(/&#0*39;/gi, "'")
    .replace(/&#x27;/gi, "'")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * 抓取 https 链接并返回纯文本（≤ maxChars）。任何失败（非 https/不安全/超时/非 2xx/非文本）返回 null。
 * 重定向手动跟随并逐跳校验，避免跳转到内网地址。
 */
export async function fetchLinkText(
  raw: string,
  opts: Partial<FetchLinkOptions> = {},
): Promise<string | null> {
  const { timeoutMs, maxChars } = { ...DEFAULT_OPTS, ...opts };
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    let url = raw;
    for (let hop = 0; hop <= MAX_REDIRECTS; hop++) {
      if (!isSafePublicHttpsUrl(url)) return null;
      const resp = await fetch(url, {
        signal: controller.signal,
        redirect: "manual",
        headers: {
          "User-Agent": "Mozilla/5.0 (compatible; TabletopGameBot/0.1)",
          Accept: "text/html,text/plain;q=0.9,application/json;q=0.5,*/*;q=0.1",
        },
      });

      if (resp.status >= 300 && resp.status < 400) {
        const loc = resp.headers.get("location");
        if (!loc) return null;
        url = new URL(loc, url).toString();
        continue;
      }
      if (!resp.ok) return null;

      const ct = resp.headers.get("content-type") ?? "";
      if (!/text\/|application\/(xhtml\+xml|json|xml)/i.test(ct)) return null;
      const body = await resp.text();
      const text = /json/i.test(ct) ? body.trim() : htmlToText(body);
      if (!text) return null;
      return text.length > maxChars ? text.slice(0, maxChars) : text;
    }
    return null;
  } catch (err) {
    console.error("[link-fetch] 抓取失败:", (err as Error)?.message ?? err);
    return null;
  } finally {
    clearTimeout(timer);
  }
}
