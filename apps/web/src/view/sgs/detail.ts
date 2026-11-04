// 详情浮层内容（纯函数）：牌面 / 武将 / 弃牌堆。
//
// 只渲染引擎视图里有的字段（不做规则推导）；所有外部字符串一律转义。

import { escapeHtml } from "../format.js";
import {
  EQUIP_SLOT_LABEL,
  IDENTITY_LABEL,
  KINGDOM_LABEL,
  SUIT_LABEL,
  SUIT_RED,
  avatarHtml,
  cardLabel,
  handLimitLabel,
  hpPips,
  playerName,
  rankLabel,
} from "./format.js";
import { skillDescription } from "./skill-text.js";
import type { SgsAssetMap, SgsCard, SgsPublicBoard, SgsSeat } from "./types.js";

function row(label: string, value: string): string {
  return `<tr><th>${escapeHtml(label)}</th><td>${value}</td></tr>`;
}

/** 牌面详情。 */
export function renderCardDetail(card: SgsCard): string {
  const red = SUIT_RED[card.suit] ? " red" : "";
  const cat =
    {
      basic: "基本牌",
      trick: "锦囊",
      delayed_trick: "延时锦囊",
      weapon: "武器",
      armor: "护具",
      horse: "坐骑",
      treasure: "特殊装备",
    }[card.category ?? ""] ?? card.category ?? "-";
  const rows = [
    row("牌名", escapeHtml(card.name)),
    row(
      "花色点数",
      `<span class="sgs-big${red}">${SUIT_LABEL[card.suit] ?? ""}${rankLabel(card.rank)}</span>`,
    ),
    row("类别", escapeHtml(String(cat))),
  ];
  if (card.equip_slot) {
    rows.push(row("装备槽", escapeHtml(EQUIP_SLOT_LABEL[card.equip_slot] ?? card.equip_slot)));
  }
  if (card.range_value) {
    rows.push(row("范围", `攻击距离 ${card.range_value}`));
  }
  if (card.delayed) rows.push(row("判定", "延时锦囊（判定区 LIFO）"));
  // 装备效果（P4.6）：引擎只下发已实装的文案；长按装备/武器牌即可看到。
  if (card.effect) {
    rows.push(row("效果", escapeHtml(card.effect).replace(/\n/g, "<br>")));
  }
  return `<table class="sgs-detail">${rows.join("")}</table>`;
}

/** 武将/座位详情（公共区可用字段）。 */
export function renderGeneralDetail(
  seat: SgsSeat,
  names?: Record<string, string>,
  assets?: SgsAssetMap,
): string {
  const rows = [
    row("玩家", escapeHtml(playerName(names, seat.player_id))),
    row("武将", escapeHtml(seat.general_name || "未选将")),
    row("势力", escapeHtml(seat.kingdom ? KINGDOM_LABEL[seat.kingdom] ?? seat.kingdom : "-")),
    row(
      "身份",
      escapeHtml(seat.identity ? IDENTITY_LABEL[seat.identity] ?? seat.identity : "未公开"),
    ),
    row("体力", `${hpPips(seat.hp, seat.max_hp)}<span class="muted"> ${seat.hp}/${seat.max_hp}</span>`),
    row("手牌", `×${seat.hand_count} / ${handLimitLabel(seat.hand_limit)}`),
    row(
      "状态",
      `${seat.alive ? "存活" : "阵亡"}${seat.flipped ? " · 翻面" : ""}${seat.chained ? " · 横置（连环）" : ""}`,
    ),
  ];
  const skillTypeLabel: Record<string, string> = {
    normal: "普通",
    lord: "主公技",
    locked: "锁定技",
    limited: "限定技",
    awaken: "觉醒技",
  };
  const skills = seat.skills ?? [];
  const skillsHtml =
    skills.length === 0
      ? `<p class="muted">技能：未公开</p>`
      : `<div class="sgs-detail-skills"><h4>技能</h4><ul>${skills
          .map((skill) => {
            const desc = skillDescription(seat.general_name, skill.name);
            const type = skillTypeLabel[skill.type] ?? skill.type;
            const unimplemented = skill.implemented === false ? `<span class="muted">（未实装）</span>` : "";
            return (
              `<li><div><strong>${escapeHtml(skill.name)}</strong>` +
              `<span class="muted"> · ${escapeHtml(type)}</span>${unimplemented}</div>` +
              (desc ? `<p>${escapeHtml(desc)}</p>` : "") +
              `</li>`
            );
          })
          .join("")}</ul></div>`;
  const portrait = seat.general
    ? `<div class="sgs-detail-portrait">` +
      avatarHtml(seat.general, seat.general_name, seat.kingdom, assets, {
        variant: "full",
        cls: "sgs-detail-avatar",
        size: [512, 720],
      }) +
      `</div>`
    : "";
  return (
    `<div class="sgs-detail-body">` +
    portrait +
    `<div class="sgs-detail-info">` +
    `<table class="sgs-detail">${rows.join("")}</table>` +
    `</div>` +
    `<div class="sgs-detail-skillwrap">${skillsHtml}</div>` +
    `<div class="sgs-detail-clear"></div>` +
    `</div>`
  );
}

/** 弃牌堆列表（顶部一张在最前）。 */
export function renderDiscardList(board: SgsPublicBoard): string {
  const list = (board.discard ?? []).slice().reverse();
  if (list.length === 0) return `<p class="muted">弃牌堆为空</p>`;
  const items = list
    .map(
      (c, i) =>
        `<li><span class="muted">${i === 0 ? "顶" : i + 1}</span> ${escapeHtml(cardLabel(c))}</li>`,
    )
    .join("");
  return `<p class="muted">共 ${list.length} 张（顶 → 底）</p><ol class="sgs-discard-list">${items}</ol>`;
}
