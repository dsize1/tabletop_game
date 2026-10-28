// 无 JS 降级文本态（ui-spec §2/§10）。
//
// - SSR 直出，可读即可；`<noscript>` 也会显示它；
// - **pending 文本提交**：一个表单 + 文本输入（动作 id / `arrange:<csv>`），
//   POST 到 `/tabletop_game/:gameId/action`（token 隐藏字段），满足「观星文字顺序提交」。

import { escapeHtml } from "../format.js";
import {
  EQUIP_SLOT_LABEL,
  EQUIP_SLOT_ORDER,
  IDENTITY_LABEL,
  KINGDOM_LABEL,
  PHASE_LABEL,
  cardLabel,
  playerName,
} from "./format.js";
import type { SgsViewData } from "./types.js";

export interface SgsActionFormOpts {
  gameId: string;
  token: string;
  roomId: string;
}

function actionForm(data: SgsViewData, opts: SgsActionFormOpts): string {
  const area = data.player_area;
  if (!area) return "";
  const options = area.pending?.options ?? [];
  const defaultId = options[0]?.id ?? "";
  const listId = "sgs-actions";
  const list = options
    .map((o) => `<option value="${escapeHtml(o.id)}">${escapeHtml(o.label)}</option>`)
    .join("");
  const pendingLine = area.pending
    ? `<p class="muted">待办：${escapeHtml(area.pending.kind)}（观星可填 <code>arrange:牌id,牌id,...</code>）</p>`
    : "";
  return (
    `<form class="sgs-form" method="post" action="/tabletop_game/${encodeURIComponent(opts.gameId)}/action">` +
    `<input type="hidden" name="token" value="${escapeHtml(opts.token)}">` +
    `<input type="hidden" name="room_id" value="${escapeHtml(opts.roomId)}">` +
    pendingLine +
    `<label>动作 <input name="action" value="${escapeHtml(defaultId)}" list="${listId}"></label>` +
    `<datalist id="${listId}">${list}</datalist>` +
    `<button type="submit">提交</button>` +
    `</form>`
  );
}

/** 文本态正文（公共 + 个人 + 特殊区 + 动作提交）。 */
export function renderSgsTextBody(
  data: SgsViewData,
  names: Record<string, string> | undefined,
  formOpts?: SgsActionFormOpts,
): string {
  const board = data.public_board;
  const seats = board.seats
    .map((s) => {
      const identity = s.identity ? IDENTITY_LABEL[s.identity] ?? s.identity : "?";
      const general = s.general_name || "未选将";
      const equips = EQUIP_SLOT_ORDER.map((slot) => {
        const c = s.equips?.[slot];
        return c ? `${EQUIP_SLOT_LABEL[slot]}:${cardLabel(c)}` : "";
      })
        .filter(Boolean)
        .join("，");
      const judge = (s.judge_zone ?? []).map((c) => cardLabel(c)).join("，");
      return (
        `<li><b>${escapeHtml(playerName(names, s.player_id))}</b>（${escapeHtml(general)}` +
        `${s.kingdom ? ` ${KINGDOM_LABEL[s.kingdom] ?? s.kingdom}` : ""}，${escapeHtml(identity)}）` +
        ` 体力 ${s.hp}/${s.max_hp}，手牌 ${s.hand_count}` +
        (equips ? `，装备 ${escapeHtml(equips)}` : "") +
        (judge ? `，判定 ${escapeHtml(judge)}` : "") +
        (s.alive ? "" : "（阵亡）") +
        `</li>`
      );
    })
    .join("");

  const area = data.player_area;
  const personal = area
    ? `<h3>我的资源</h3>` +
      `<p>${escapeHtml(area.general_name ?? "未选将")} · 体力 ${area.hp}/${area.max_hp} · ` +
      `${escapeHtml(IDENTITY_LABEL[area.identity] ?? area.identity)}</p>` +
      `<p>手牌：${(area.hand ?? []).map((c) => escapeHtml(cardLabel(c))).join("，") || "无"}</p>` +
      `<p>装备：${EQUIP_SLOT_ORDER.map((slot) => {
        const c = area.equips?.[slot];
        return c ? `${EQUIP_SLOT_LABEL[slot]}：${cardLabel(c)}` : "";
      })
        .filter(Boolean)
        .join("；") || "无"}</p>` +
      `<p>判定区：${(area.judge_zone ?? []).map((c) => escapeHtml(cardLabel(c))).join("，") || "无"}</p>` +
      `<p>技能：${(area.skills ?? []).map((s) => escapeHtml(s.name)).join("，") || "无"}</p>` +
      (area.pending ? `<p>待办：${escapeHtml(area.pending.kind)}</p>` : "")
    : `<p class="muted">个人资源不可见</p>`;

  const gx = data.special_zones?.guanxing;
  const guanxing = gx
    ? `<h3>观星（文字顺序）</h3><p>` +
      `左列：${Array.from({ length: gx.rows }, (_, i) => gx.slots?.[`L${i + 1}`] ?? "空").join(" → ")}<br>` +
      `右列：${Array.from({ length: gx.rows }, (_, i) => gx.slots?.[`R${i + 1}`] ?? "空").join(" → ")}` +
      `</p><p class="muted">提交格式：<code>arrange:L1,L2,...,R5</code>（跳过空槽）</p>`
    : "";
  const wuguZone = data.special_zones?.wugu;
  const wugu = wuguZone
    ? `<h3>五谷丰登</h3><p>当前：${escapeHtml(
        wuguZone.current ? playerName(names, wuguZone.current) : "-",
      )}</p><p>亮出：${(wuguZone.cards ?? []).map((c) => escapeHtml(cardLabel(c))).join("，")}</p>`
    : "";

  return (
    `<div class="sgs-text">` +
    `<h2>三国杀 · 第 ${board.round} 轮 · ${escapeHtml(PHASE_LABEL[board.turn.phase] ?? board.turn.phase)}</h2>` +
    `<p class="muted">牌堆 ${board.deck.draw} · 弃牌 ${board.deck.discard} · 当前 ${escapeHtml(
      board.turn.player_id ? playerName(names, board.turn.player_id) : "-",
    )}</p>` +
    `<h3>公共局面</h3><ul>${seats}</ul>` +
    personal +
    guanxing +
    wugu +
    (formOpts ? actionForm(data, formOpts) : "") +
    `</div>`
  );
}
