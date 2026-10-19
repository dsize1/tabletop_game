// BoardScene → SVG 字符串（服务端首屏与浏览器重渲染共用）。
//
// 只做机械映射，不含业务逻辑；每个图元都必须能对应到 react-native-svg 的同名组件。

import { escapeHtml } from "../format.js";
import type { BoardScene, SceneNode } from "./scene.js";

function attr(name: string, value: string | number | undefined): string {
  if (value === undefined || value === "") return "";
  return ` ${name}="${escapeHtml(String(value))}"`;
}

function nodeToSvg(n: SceneNode, clipId: string | null): string {
  switch (n.t) {
    case "rect": {
      const rx = n.rx ? attr("rx", n.rx) : "";
      return (
        `<rect${attr("x", n.x)}${attr("y", n.y)}${attr("width", n.w)}${attr("height", n.h)}${rx}` +
        `${attr("fill", n.fill)}${attr("stroke", n.stroke)}${attr("stroke-width", n.sw)}` +
        `${attr("opacity", n.opacity)}/>`
      );
    }
    case "circle":
      return (
        `<circle${attr("cx", n.cx)}${attr("cy", n.cy)}${attr("r", n.r)}` +
        `${attr("fill", n.fill)}${attr("stroke", n.stroke)}${attr("stroke-width", n.sw)}` +
        `${attr("opacity", n.opacity)}/>`
      );
    case "path":
      return (
        `<path${attr("d", n.d)}${attr("fill", n.fill ?? "none")}${attr("stroke", n.stroke)}` +
        `${attr("stroke-width", n.sw)}${attr("opacity", n.opacity)}/>`
      );
    case "text":
      return (
        `<text${attr("x", n.x)}${attr("y", n.y)}${attr("font-size", n.size)}` +
        `${attr("fill", n.fill)}${attr("text-anchor", n.anchor)}` +
        `${attr("font-weight", n.weight)}${attr("opacity", n.opacity)}>` +
        `${escapeHtml(n.text)}</text>`
      );
    case "image": {
      const clip = clipId ? attr("clip-path", `url(#${clipId})`) : "";
      return (
        `<image${attr("x", n.x)}${attr("y", n.y)}${attr("width", n.w)}${attr("height", n.h)}` +
        `${attr("href", n.href)}${attr("opacity", n.opacity)}` +
        ` preserveAspectRatio="xMidYMid slice"${clip}/>`
      );
    }
    default:
      return "";
  }
}

/** 渲染整张版图（SVG 字符串，可内联进 HTML）。 */
export function renderBoardSvg(scene: BoardScene, opts: { ariaLabel?: string } = {}): string {
  const parts: string[] = [];
  const defs: string[] = [];
  let clipSeq = 0;

  for (const layer of scene.layers) {
    const nodes: string[] = [];
    for (const n of layer.nodes) {
      let clipId: string | null = null;
      if (n.t === "image" && n.rx) {
        clipSeq += 1;
        clipId = `bc${clipSeq}`;
        defs.push(
          `<clipPath id="${clipId}"><rect x="${n.x}" y="${n.y}" width="${n.w}" height="${n.h}" rx="${n.rx}"/></clipPath>`,
        );
      }
      nodes.push(nodeToSvg(n, clipId));
    }
    parts.push(`<g data-layer="${escapeHtml(layer.name)}">${nodes.join("")}</g>`);
  }

  const label = escapeHtml(opts.ariaLabel ?? "拉斯维加斯版图");
  return (
    `<svg class="board-svg" viewBox="0 0 ${scene.w} ${scene.h}" ` +
    `style="display:block;width:100%;height:auto" ` +
    `preserveAspectRatio="xMidYMin meet" role="img" aria-label="${label}" ` +
    `xmlns="http://www.w3.org/2000/svg">` +
    `<title>${label}</title>` +
    (defs.length ? `<defs>${defs.join("")}</defs>` : "") +
    parts.join("") +
    `</svg>`
  );
}

/** 首屏素材预加载提示（只含实际用到的图）。 */
export function renderPreloadLinks(scene: BoardScene): string {
  return scene.assets
    .map((href) => `<link rel="preload" as="image" href="${escapeHtml(href)}">`)
    .join("");
}
