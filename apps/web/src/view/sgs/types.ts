// 三国杀视图层数据契约（P3）。
//
// 与引擎 `view.rs` 的三分区投影一致（`docs/games/sanguosha/data-model.md` §4.2）。
// 本文件只描述「渲染需要什么」，不做 IO；服务端 SSR 与浏览器 bundle 共用。
// 字段集由 `apps/web/test-sgs.mts` + 引擎侧 `check:sgs-view-contract` 双重守护。

/** 素材引用（server `/internal/gameprops/asset-map` 投影；URL 由 web 同源静态服务）。 */
export interface SgsAssetRef {
  url: string;
  thumb: string;
  width?: number | null;
  height?: number | null;
}

/** 武将 code → 素材（无图/未绑定时缺省）。 */
export type SgsAssetMap = Record<string, SgsAssetRef>;

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
  /** 装备牌效果文案（引擎只对已实装效果下发；非装备为 null/缺省）。 */
  effect?: string | null;
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
  /** 手牌上限（`null` = 本回合无上限，如克己）。 */
  hand_limit?: number | null;
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
  /** 连环状态（横置）：火/雷属性伤害会传导。 */
  chained?: boolean;
  /** 存活（I2：未选将/未开局为 null，不展示）。 */
  alive: boolean | null;
  is_bot?: boolean;
  /** 已公开武将的技能（公开信息；未公示时为空/缺省）。 */
  /** 转换技当前面：code → "yang" | "yin"（公开信息）。 */
  transforms?: Record<string, string>;
  skills?: SgsSkill[];
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
  /** 当前公开亮出的牌（火攻展示/五谷亮牌/蛊惑翻开等）。 */
  revealed?: SgsCard[];
  /** 弃牌堆全量（顶 → 底，引擎按入堆顺序返回）。 */
  discard?: SgsCard[];
  pending?: { kind: string; actor: string } | null;
  setup?: SgsPublicSetup | null;
  winner?: { team: string; players: string[] } | null;
  aborted?: string | null;
  history?: SgsHistoryEntry[];
}

/** 公共区选将进度（`public_board.setup`）。 */
export interface SgsPublicSetup {
  lord?: string | null;
  lord_picked?: boolean;
  picked_count?: number;
}

/** 选将候选（个人资源区 `setup.candidate_details`，引擎已投影为可直接渲染的详情）。 */
export interface SgsSetupCandidate {
  code: string;
  name: string;
  kingdom?: string;
  max_hp?: number;
  is_lord?: boolean;
  skills?: string[];
}

/** 个人资源区选将态（`player_area.setup`）。 */
export interface SgsPlayerSetup {
  /** 候选武将 code（既有契约，保留）。 */
  candidates?: string[];
  /** 候选详情（名字/势力/体力/技能名），与 `candidates` 一一对应。 */
  candidate_details?: SgsSetupCandidate[];
  /** 本人已选武将 code（未选为 undefined/null）。 */
  picked?: string | null;
  is_lord?: boolean;
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
  /** 手牌上限（`null` = 本回合无上限）。 */
  hand_limit?: number | null;
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
  /** 连环状态（横置）。 */
  chained?: boolean;
  skills?: SgsSkill[];
  is_bot?: boolean;
  is_current?: boolean;
  equipment_skills?: { slot: string; code: string; name: string; card: string; trigger: string }[];
  pending?: SgsPending | null;
  setup?: SgsPlayerSetup | null;
  /** 私密展示（观星/攻心等）：只对本人可见的牌。 */
  private_reveals?: SgsCard[];
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
  round?: number;
  /** 终局房规（第 100 轮后【桃】/【桃园结义】回血无效；濒死救援不限）。 */
  endgame?: { no_heal: boolean; round: number; since: number };
  public_board: SgsPublicBoard;
  player_area?: SgsPlayerArea | null;
  special_zones?: SgsSpecialZones;
}
