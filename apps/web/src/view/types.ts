// 视图层数据契约（服务端 SSR 与浏览器 bundle 共用；RN 端复用本文件）。
//
// 只描述「渲染需要什么」，不做任何 IO：不得 import express / node:fs / DOM。

/** 小游戏子状态（引擎 `sg` 字段；缺失时各视图自行降级）。 */
export interface SgState {
  f2_slots?: Array<number | null> | null;
  h1_slots?: Array<{ owner: string; dice: number; big?: number } | null> | null;
  gray_groups?: number[];
  /** E2 酒吧：玩家 → {small,big}（大骰也算 1 颗）。 */
  bars?: Record<string, { small?: number; big?: number }>;
  game_field?: Record<string, number>;
  /** H2 金框：{owner, small, big}（大骰也算 1 颗）。 */
  gold_frame?: { owner: string; small?: number; big?: number } | null;
  banned?: number | null;
  arrow?: number;
  lottery?: number[];
  indicator?: number;
  /** C1：本轮「先到 5 颗」的玩家（拿到即得 $10 万）。 */
  c1_winner?: string | null;
  privilege?: string | null;
  /** 只在**个人视图**出现：加密选项（A1/G1）代码 → 真实含义。 */
  pending_options?: {
    game?: string;
    stage?: string;
    multi?: boolean;
    options?: Array<{
      code: string;
      count?: number;
      reward?: string;
      value?: number;
      label?: string;
    }>;
  } | null;
}

export interface SeatData {
  player_id: string;
  small_remaining?: number;
  big_remaining?: number;
  chips?: number;
  banknotes?: number[];
  is_bot?: boolean;
  banknote_count?: number;
  markers?: string[];
}

export interface CasinoData {
  dice?: Record<string, number>;
  /** 各玩家在该赌场的大骰颗数（`dice` 已把大骰计为 2 点权重）。 */
  big_dice?: Record<string, number>;
  banknotes?: number[];
  small_games?: string[];
}

export interface GameData {
  phase?: string;
  round?: number;
  turn?: number;
  start_player?: string;
  seats?: SeatData[];
  casinos?: CasinoData[];
  roll?: { small?: number[]; big?: number };
  winners?: string[];
  scores?: Record<string, number>;
  sg?: SgState;
}

/** 游戏资源视图（server `/resources/game/{gameId}` 返回）。 */
export interface GameView {
  game_id: string;
  room_id: string;
  openid: string;
  public: { status?: string; players?: string[]; data?: GameData };
  player: { status?: string; players?: string[]; data?: GameData };
  /** openid → 群昵称（server 从 Redis `names:{group_id}` 读取）。 */
  names?: Record<string, string>;
}

/** 页内双模式。 */
export type ViewMode = "board" | "text";
