// 轮询策略（纯函数，P0）：待本人操作时降频，避免无意义刷新。
//
// 状态机在等待本人选将/响应/出牌时不会推进，10s 轮询没有收益；
// 保留 60s 保活用于会话/外部变更兜底，提交动作或页面恢复时立即刷新。

import type { SgsViewData } from "./types.js";

export const BASE_POLL_MS = 10_000;
export const AWAIT_POLL_MS = 60_000;

type ActionLike = { id: string; description: string };

/** 当前是否在等待本人操作（选将 / pending 响应 / 本人回合等待出牌）。 */
export function awaitingLocal(
  data: SgsViewData | null | undefined,
  actions: ActionLike[] | undefined,
): boolean {
  const area = data?.player_area;
  if (!area) return false;
  if (area.pending) return true;
  const setup = area.setup;
  if (setup && !setup.picked && (setup.candidate_details?.length ?? 0) > 0) return true;
  return Boolean(area.is_current && (actions?.length ?? 0) > 0);
}

/** 下一次轮询间隔：待本人操作 → 60s；否则用调用方基准（默认 10s）。 */
export function nextPollDelay(
  data: SgsViewData | null | undefined,
  actions: ActionLike[] | undefined,
  baseMs = BASE_POLL_MS,
): number {
  return awaitingLocal(data, actions) ? Math.max(AWAIT_POLL_MS, baseMs) : baseMs;
}

/** 应用态签名（不含 history）：相同则不重建 `#sgs-app`。 */
export function stateSignature(
  data: SgsViewData | null | undefined,
  actions: ActionLike[] | undefined,
  names?: Record<string, string> | null,
): string {
  if (!data) return "";
  const b = data.public_board;
  return JSON.stringify({
    phase: data.phase,
    pb: b
      ? {
          phase: b.phase,
          round: b.round,
          turn_index: b.turn_index,
          turn: b.turn,
          seat_count: b.seat_count,
          seats: b.seats,
          deck: b.deck,
          discard_top: b.discard_top,
          discard: b.discard,
          pending: b.pending,
          setup: b.setup,
          winner: b.winner,
          aborted: b.aborted,
        }
      : null,
    pa: data.player_area,
    zones: data.special_zones,
    actions: actions ?? [],
    names: names ?? {},
  });
}

/** 侧栏签名：history 末 seq + 阶段/回合/待办（侧栏展示历史与阶段）。 */
export function sideSignature(
  data: SgsViewData | null | undefined,
  names?: Record<string, string> | null,
): string {
  const b = data?.public_board;
  const h = b?.history;
  return [
    h && h.length ? h[h.length - 1].seq : 0,
    b?.phase ?? "",
    b?.round ?? 0,
    b?.turn?.phase ?? "",
    b?.pending?.kind ?? "",
    b?.pending?.actor ?? "",
  ].join("|") + `|${JSON.stringify(names ?? {})}`;
}
