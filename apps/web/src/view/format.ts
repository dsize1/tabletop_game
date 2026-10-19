// 共享格式化工具（纯函数，无 IO）。

const ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESCAPE_MAP[c] ?? c);
}

/** 玩家 id → 展示名（缺失时用 id）。 */
export function who(names: Record<string, string> | undefined, id: string): string {
  return escapeHtml(names?.[id] ?? id);
}

/** 原始显示名（不转义，供 SVG / 文本节点自行处理）。 */
export function displayName(names: Record<string, string> | undefined, id: string): string {
  return names?.[id] ?? id;
}

/** 分 → 「X 万」（钞票面额以「分」存储时为 /10000；本项目直接是「万」。 */
export function wan(n: number): string {
  return `${Math.round(n / 10000)} 万`;
}

/** 钞票总额（面额单位：元/万，与引擎一致，直接相加）。 */
export function banknoteTotal(notes: number[] | undefined): number {
  return (notes ?? []).reduce((a, b) => a + Number(b || 0), 0);
}

export function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}
