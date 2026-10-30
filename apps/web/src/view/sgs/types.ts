// 三国杀视图层数据契约（P3）。
//
// 与引擎 `view.rs` 的三分区投影一致（`docs/games/sanguosha/data-model.md` §4.2）。
// 本文件只描述「渲染需要什么」，不做 IO；服务端 SSR 与浏览器 bundle 共用。
// 字段集由 `apps/web/test-sgs.mts` + 引擎侧 `check:sgs-view-contract` 双重守护。

export type Suit = "spade" | "heart" | "club" | "diamond";

export interface SgsCard {
  id: string;
  def: string;
  name: string;
  suit: Suit;
  rank: number;
  category?: string | null;
  equip_slot?: string | null;
  range_value?: number | null;
  range_type?: string | null;
  target_mode?: string | null;
  delayed?: boolean;
}

export interface SgsPendingOption {
  id: string;
  label: string;
}

/** 仅本人可见的 pending（公共区只有 `{kind, actor}`）。 */
export interface SgsPending {
  kind: string;
  actor: string;
  skill?: string | null;
  options: SgsPendingOption[];
  data?: Record<string, unknown>;
}

export interface SgsEquips {
  weapon?: SgsCard | null;
  armor?: SgsCard | null;
  horse_offense?: SgsCard | null;
  horse_defense?: SgsCard | null;
  treasure?: SgsCard | null;
  [slot: string]: SgsCard | null | undefined;
}

export interface SgsSeat {
  seat: number;
  player_id: string;
  identity?: string | null;
  general?: string | null;
  general_name?: string;
  kingdom?: string | null;
  hp: number;
  max_hp: number;
  hand_count: number;
  equips: SgsEquips;
  judge_zone: SgsCard[];
  /** 武将牌上的标记（code → 牌）。 */
  marks?: Record<string, SgsCard[]>;
  /** 计数型标记（code → 数量，如怀橘「橘」）。 */
  marks_count?: Record<string, number>;
  /** 已废除的装备栏（SP-2：决堰/雄乱）。 */
  abolished_slots?: string[];
  /** 判定区是否已废除（SP-2 雄乱）。 */
  judge_abolished?: boolean;
  /** 翻面状态（背面朝上）。 */
  flipped?: boolean;
  alive: boolean;
  is_bot?: boolean;
}

export interface SgsTurn {
  seat: number;
  phase: string;
  player_id?: string | null;
}

export interface SgsHistoryEntry {
  seq: number;
  turn: number;
  actor?: string | null;
  text: string;
}

export interface SgsPublicBoard {
  phase: string;
  round: number;
  turn_index: number;
  turn: SgsTurn;
  config?: Record<string, number | boolean>;
  seat_count: number;
  seats: SgsSeat[];
  deck: { draw: number; discard: number; processing: number; void: number };
  discard_top?: SgsCard | null;
  /** 弃牌堆全量（顶 → 底，引擎按入堆顺序返回）。 */
  discard?: SgsCard[];
  pending?: { kind: string; actor: string } | null;
  setup?: unknown;
  winner?: { team: string; players: string[] } | null;
  aborted?: string | null;
  history?: SgsHistoryEntry[];
}

export interface SgsSkill {
  code: string;
  name: string;
  type: string;
  implemented?: boolean;
}

export interface SgsPlayerArea {
  seat: number;
  player_id: string;
  identity: string;
  general?: string | null;
  general_gid?: number | null;
  kingdom?: string | null;
  general_name?: string;
  hp: number;
  max_hp: number;
  hand: SgsCard[];
  equips: SgsEquips;
  judge_zone: SgsCard[];
  marks?: Record<string, SgsCard[]>;
  marks_count?: Record<string, number>;
  /** 转换技当前面（技能 code → yang/yin）。 */
  transforms?: Record<string, string>;
  /** 已废除的装备栏（SP-2）。 */
  abolished_slots?: string[];
  /** 判定区是否已废除（SP-2）。 */
  judge_abolished?: boolean;
  flipped?: boolean;
  skills?: SgsSkill[];
  is_bot?: boolean;
  is_current?: boolean;
  equipment_skills?: { slot: string; code: string; name: string; card: string; trigger: string }[];
  pending?: SgsPending | null;
  setup?: unknown;
}

/** 特殊交互区（按 visibility 已由服务端过滤）。 */
export interface SgsGuanxingZone {
  owner?: string;
  /** 本次取出的牌（owner 可见；端侧渲染槽位用）。 */
  cards?: SgsCard[];
  visibility: string;
  rows: number;
  cols: number;
  slots: Record<string, string | null>;
  rev: number;
  skill?: string;
  interaction?: { kind: string; commit: string; slots?: string };
}

export interface SgsWuguZone {
  visibility: string;
  cards: SgsCard[];
  choosers: string[];
  cursor: number;
  current?: string | null;
  picked: Array<{ player: string; card: SgsCard }>;
  interaction?: { kind: string; commit: string };
}

export interface SgsSpecialZones {
  guanxing?: SgsGuanxingZone;
  wugu?: SgsWuguZone;
  judge?: {
    owner?: string;
    delayed?: SgsCard;
    judge_card?: SgsCard;
    effect?: string;
  };
  [zone: string]: unknown;
}

/** 引擎视图 `data`（三分区根）。 */
export interface SgsViewData {
  schema_version: number;
  game: string;
  phase: string;
  public_board: SgsPublicBoard;
  player_area?: SgsPlayerArea | null;
  special_zones?: SgsSpecialZones;
}
