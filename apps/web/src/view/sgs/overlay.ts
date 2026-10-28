// 居中浮层模板（纯函数，P3.2）。
//
// 规范（ui-spec §6）：单层、遮罩、滚动锁、Esc / 遮罩 / 关闭按钮关闭；
// 交互浮层（观星等）内容由调用方传入 body；需要看二级详情时用同层展开区，不叠第二层。

import { escapeHtml } from "../format.js";

export interface OverlayShellOpts {
  /** 额外 class（如 `wide` 用于观星）。 */
  size?: "normal" | "wide";
}

/** 浮层外壳：遮罩 + 卡片 + 标题 + 关闭按钮 + 内容。 */
export function renderOverlayShell(
  title: string,
  bodyHtml: string,
  opts: OverlayShellOpts = {},
): string {
  const wide = opts.size === "wide" ? " wide" : "";
  return (
    `<div class="sgs-overlay-mask" data-overlay-close="mask"></div>` +
    `<div class="sgs-overlay-card${wide}" role="document">` +
    `<div class="sgs-overlay-head"><h2>${escapeHtml(title)}</h2>` +
    `<button type="button" class="sgs-overlay-close" data-overlay-close="button" aria-label="关闭">×</button></div>` +
    `<div class="sgs-overlay-body">${bodyHtml}</div>` +
    `</div>`
  );
}
