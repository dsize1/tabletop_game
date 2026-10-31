//! 三国杀（标准身份局）引擎。
//!
//! 实现 `game-engine-core::EnginePlugin`。规则见 `docs/games/sanguosha/`。
//!
//! **P2.1 范围**：crate 骨架 + lobby（5–8 人）+ 发身份 + 选将（主公 N 选 1、其他同时 M 选 1）
//! + 标准牌堆洗牌/发初始手牌 + 状态 `schema_version=1` + 视图三分区投影。
//! 回合流程（判定/摸牌/出牌/弃牌）、基本牌、濒死与胜负在 P2.2–P2.3 实现。

mod data;
mod resolve;
mod resolve_fire;
mod resolve_forest;
mod resolve_jiexian;
mod resolve_jiexian2;
mod resolve_jiexian3;
mod resolve_jiexian4;
mod resolve_jiexian5;
mod resolve_jiexian6;
mod resolve_jiexian7;
mod resolve_more;
mod resolve_mountain;
mod resolve_skill;
mod resolve_sp;
mod resolve_sp2;
mod resolve_wind;
mod resolve_yijiang;
mod resolve_yijiang2;
mod resolve_yijiang3;
mod resolve_yijiang4;
mod resolve_yijiang5;
mod resolve_yijiang6;
mod resolve_yijiang7;
mod resolve_yijiang8;
mod resolve_yijiang9;
mod resolve_zone;
mod skills;
mod state;
mod turn;
mod view;

pub use data::{
    FULL_DECK_SIZE, GENERAL_COUNT, JUNZHENG_DECK_SIZE, STANDARD_DECK_SIZE, STANDARD_GENERAL_COUNT,
};
pub use state::*;

use game_engine_core::{
    Action, ActionResult, EngineError, EngineEvent, EnginePlugin, GameState, GameStatus,
    RandomSource,
};
use serde_json::{json, Value};

pub(crate) fn illegal(msg: impl Into<String>) -> EngineError {
    EngineError::IllegalAction(msg.into())
}

pub(crate) fn internal(msg: impl Into<String>) -> EngineError {
    EngineError::Internal(msg.into())
}

fn require_host(st: &SgsState, player_id: &str) -> Result<(), EngineError> {
    if st.host != player_id {
        return Err(EngineError::PermissionDenied("仅房主可执行该操作".into()));
    }
    Ok(())
}

/// 标准身份配比（与中台 `sgs_identity_setup` seed 一致）。
pub fn identity_list(player_count: usize, double_spy: bool) -> Result<Vec<Identity>, EngineError> {
    use Identity::*;
    let ids = match (player_count, double_spy) {
        (5, false) => vec![Lord, Loyalist, Rebel, Rebel, Spy],
        (6, false) => vec![Lord, Loyalist, Rebel, Rebel, Rebel, Spy],
        (7, false) => vec![Lord, Loyalist, Loyalist, Rebel, Rebel, Rebel, Spy],
        (8, false) => vec![Lord, Loyalist, Loyalist, Rebel, Rebel, Rebel, Rebel, Spy],
        (8, true) => vec![Lord, Loyalist, Rebel, Rebel, Rebel, Rebel, Spy, Spy],
        _ => {
            return Err(illegal(format!(
                "人数不合法：{player_count}（双内仅支持 8 人）"
            )))
        }
    };
    Ok(ids)
}

fn identity_counts(seats: &[Seat]) -> Value {
    let mut lord = 0;
    let mut loyalist = 0;
    let mut rebel = 0;
    let mut spy = 0;
    for s in seats {
        match s.identity {
            Identity::Lord => lord += 1,
            Identity::Loyalist => loyalist += 1,
            Identity::Rebel => rebel += 1,
            Identity::Spy => spy += 1,
        }
    }
    json!({ "lord": lord, "loyalist": loyalist, "rebel": rebel, "spy": spy })
}

/// 构建新 `GameState`（内部状态完整；对外视图由 `public_state` / `player_view` 投影）。
fn build_state(old: &GameState, st: &SgsState) -> GameState {
    GameState {
        game_id: old.game_id.clone(),
        room_id: old.room_id.clone(),
        group_id: old.group_id.clone(),
        status: if st.phase.is_over() {
            GameStatus::Finished
        } else {
            GameStatus::Active
        },
        players: st.seats.iter().map(|s| s.player_id.clone()).collect(),
        data: serde_json::to_value(st).unwrap_or_else(|_| json!({})),
    }
}

fn parse(data: &Value) -> Result<SgsState, EngineError> {
    SgsState::parse(data).map_err(internal)
}

// ---------------------------------------------------------------------------
// 选将 / 发牌
// ---------------------------------------------------------------------------

/// 当前对局可用的武将码：`Config.general_packs` 过滤 + 技能全部已实现 + 同名去重。
///
/// 同名去重（ADR-0024 口径 A「界优先」）：同时启用 `jiexian` 与标准/风林火山时，
/// 与「界化」武将同名的原版不进选将池（界化武将名如「界刘备」去掉前导「界」即原名）。
fn available_general_codes(st: &SgsState) -> Vec<&'static str> {
    available_codes_for_packs(&st.config.general_packs)
}

/// 按包组合计算选将池（含实现门槛 + 同名去重）。
fn available_codes_for_packs(packs: &[String]) -> Vec<&'static str> {
    let codes: Vec<&'static str> = data::general_codes_in_packs(packs)
        .into_iter()
        .filter(|code| skills::general_ready(code))
        .collect();
    prefer_jiexian(codes)
}

/// 同名去重（ADR-0024 口径 A「界优先」）：与「界化」武将同名的原版从池中剔除。
///
/// 纯函数（不依赖技能实现状态），便于单测与后续界标 25 将接入后直接生效。
pub(crate) fn prefer_jiexian(codes: Vec<&'static str>) -> Vec<&'static str> {
    let jie_names: Vec<&'static str> = codes
        .iter()
        .filter_map(|c| data::general(c))
        .filter(|g| g.pack == "jiexian")
        .filter_map(|g| g.name.strip_prefix('界'))
        .collect();
    if jie_names.is_empty() {
        return codes;
    }
    codes
        .into_iter()
        .filter(|code| {
            let Some(g) = data::general(code) else {
                return false;
            };
            g.pack == "jiexian" || !jie_names.contains(&g.name)
        })
        .collect()
}

fn general_pack_mode_label(mode: &str) -> (String, usize) {
    let packs = parse_general_packs(mode).unwrap_or_default();
    // 与实际选将池一致：含同名去重（界优先）。
    let count = available_codes_for_packs(&packs).len();
    let mut label = String::from("标准");
    for p in packs.iter().filter(|p| p.as_str() != "standard") {
        label.push_str(" + ");
        label.push_str(match p.as_str() {
            "wind_fire_forest_mountain" => "风林火山",
            "yijiang" => "一将成名",
            "jiexian" => "界限突破",
            other => other,
        });
    }
    (label, count)
}

/// lobby 提供的武将包组合：`standard` + 其余包的全部子集（2^(N-1)，含全量）。
///
/// 顺序按 `data::GENERAL_PACK_KEYS` 生成、再按包数升序稳定排序（单包 → 全量）。
/// 标准包恒为基数（与牌堆 `standard` 基数一致）。
fn general_pack_presets() -> Vec<String> {
    let others: Vec<&str> = crate::data::GENERAL_PACK_KEYS
        .iter()
        .copied()
        .filter(|p| *p != "standard")
        .collect();
    let mut out: Vec<String> = Vec::new();
    for mask in 0usize..(1usize << others.len()) {
        let mut keys = vec!["standard"];
        for (i, p) in others.iter().enumerate() {
            if mask & (1 << i) != 0 {
                keys.push(*p);
            }
        }
        out.push(keys.join("+"));
    }
    out.sort_by_key(|mode| mode.matches('+').count());
    out
}

/// 给主公发候选：启用包内全部主公将 + `lord_candidate_count` 张非主公。
fn offer_to_lord(st: &mut SgsState) -> Result<(), EngineError> {
    let lord = st.setup.lord.clone();
    let pool = available_general_codes(st);
    let lord_codes: Vec<&'static str> = pool
        .iter()
        .copied()
        .filter(|c| data::general(c).map(|g| g.is_lord).unwrap_or(false))
        .collect();
    let mut non_lord: Vec<&'static str> = pool
        .into_iter()
        .filter(|c| data::general(c).map(|g| !g.is_lord).unwrap_or(false))
        .collect();
    st.shuffle(&mut non_lord);
    let extra = (st.config.lord_candidate_count as usize).min(non_lord.len());
    let mut offers: Vec<String> = lord_codes.iter().map(|c| (*c).to_string()).collect();
    offers.extend(non_lord[..extra].iter().map(|c| (*c).to_string()));
    st.setup.offered.extend(offers.iter().cloned());
    st.setup.offers.insert(lord, offers);
    Ok(())
}

/// 主公选完后，按「主公左手侧」顺序给其他玩家发候选（玩家间不重复；未选回池不再发）。
fn distribute_offers_to_others(st: &mut SgsState) -> Result<(), EngineError> {
    let lord_idx = st.lord_index().ok_or_else(|| internal("缺少主公座位"))?;
    let picks: Vec<String> = st.setup.picks.values().cloned().collect();
    let mut pool: Vec<String> = available_general_codes(st)
        .into_iter()
        .map(str::to_string)
        .filter(|c| !picks.contains(c))
        .collect();
    st.shuffle(&mut pool);

    let targets: Vec<usize> = st
        .seats_from_left_of(lord_idx)
        .into_iter()
        .filter(|i| {
            !st.seats[*i].identity.is_lord()
                && !st.setup.picks.contains_key(&st.seats[*i].player_id)
        })
        .collect();
    if targets.is_empty() {
        st.setup.pool = pool;
        return Ok(());
    }

    // P2.1 内置武将池只有标准 25 将（P4 扩池）：8 人局 7×M 可能超过池子，
    // 此时按「均分」降级（每人不低于 1、不超过 M，余数给左手侧靠前的玩家）。
    let per = st.config.other_candidate_count as usize;
    let n = targets.len();
    let base = (pool.len() / n).min(per);
    let remainder = pool.len() % n;
    for (k, seat_idx) in targets.into_iter().enumerate() {
        let take_n = if base < per && k < remainder {
            base + 1
        } else {
            base
        }
        .min(pool.len());
        let take: Vec<String> = pool.drain(..take_n).collect();
        let pid = st.seats[seat_idx].player_id.clone();
        st.setup.offered.extend(take.iter().cloned());
        st.setup.offers.insert(pid, take);
    }
    st.setup.pool = pool;
    Ok(())
}

/// 记录一次选将（校验候选），主公选完则给其他人发候选。不含自动补选。
fn do_pick(
    st: &mut SgsState,
    player_id: &str,
    code: &str,
) -> Result<Vec<EngineEvent>, EngineError> {
    if st.phase != Phase::Setup {
        return Err(illegal("当前不是选将阶段"));
    }
    let seat_idx = st
        .seat_index(player_id)
        .ok_or_else(|| EngineError::PermissionDenied("不在座玩家".into()))?;
    if st.setup.picks.contains_key(player_id) {
        return Err(illegal("已选过武将"));
    }
    let offers = st
        .setup
        .offers
        .get(player_id)
        .cloned()
        .ok_or_else(|| illegal("尚未收到候选武将"))?;
    if !offers.iter().any(|c| c == code) {
        return Err(illegal("该武将不在你的候选中"));
    }
    let def = data::general(code).ok_or_else(|| illegal("未知武将"))?;
    st.setup
        .picks
        .insert(player_id.to_string(), code.to_string());

    let events = vec![EngineEvent::new(
        "general_picked",
        json!({ "player": player_id, "general": code, "general_name": def.name }),
    )
    .with_actor(player_id)];

    if st.seats[seat_idx].identity.is_lord() {
        st.setup.lord_picked = true;
        distribute_offers_to_others(st)?;
    }
    Ok(events)
}

/// 机器人自动选将（从自己的候选中随机取一张）。
fn auto_pick_bots(st: &mut SgsState) -> Result<Vec<EngineEvent>, EngineError> {
    let mut events = Vec::new();
    loop {
        let candidate = st
            .seats
            .iter()
            .find(|s| {
                s.is_bot
                    && !st.setup.picks.contains_key(&s.player_id)
                    && st
                        .setup
                        .offers
                        .get(&s.player_id)
                        .is_some_and(|o| !o.is_empty())
            })
            .map(|s| s.player_id.clone());
        let Some(pid) = candidate else { break };
        let offers = st.setup.offers.get(&pid).cloned().unwrap_or_default();
        let code = offers[st.next_index(offers.len())].clone();
        events.extend(do_pick(st, &pid, &code)?);
    }
    Ok(events)
}

/// 选将完成后：定体力、建牌堆、发初始手牌、进入回合。
fn finish_setup(st: &mut SgsState) -> Result<Vec<EngineEvent>, EngineError> {
    if st.setup.picks.len() != st.seats.len() {
        return Err(internal("选将未完成"));
    }
    for i in 0..st.seats.len() {
        let pid = st.seats[i].player_id.clone();
        let code = st.setup.picks.get(&pid).cloned().unwrap_or_default();
        let def = data::general(&code).ok_or_else(|| internal(format!("未知武将 {code}")))?;
        let bonus = if st.seats[i].identity.is_lord() { 1 } else { 0 };
        let seat = &mut st.seats[i];
        seat.general = Some(code);
        seat.general_name = def.name.to_string();
        seat.max_hp = (def.max_hp + bonus).min(MAX_HP_LIMIT);
        seat.hp = seat.max_hp;
    }

    // 化身（左慈）：开局随机获得两张化身牌。
    for i in 0..st.seats.len() {
        crate::resolve_mountain::init_huashen(st, i);
    }
    // 怀橘（陆绩）：开局获得 3 枚「橘」。
    for i in 0..st.seats.len() {
        crate::resolve_sp::init_huaiju(st, i);
    }
    // 宗祚（刘虞）：开局按势力数增加体力上限并回复 1。
    crate::resolve_yijiang8::init_zongzuo(st);
    // 血裔（界袁绍）：开局按群势力角色数获得「裔」。
    crate::resolve_jiexian2::init_xueyi(st);
    // 巧变（界张郃）：开局获得 2 枚「变」。
    crate::resolve_jiexian4::init_qiaobian(st);
    // 明任（卢植）：开局摸 2 并选择「任」。
    for i in 0..st.seats.len() {
        crate::resolve_sp2::init_mingren(st, i);
    }
    // 建牌堆：按 `Config.packs` 组堆（standard → junzheng），实例 id 按牌堆顺序编号，再整体洗牌。
    let deck = data::deck_cards(&st.config.packs).map_err(illegal)?;
    let deck_size = deck.len();
    let mut ids: Vec<String> = Vec::with_capacity(deck_size);
    for (i, pc) in deck.iter().enumerate() {
        let id = format!("c{}", i + 1);
        st.cards.insert(
            id.clone(),
            CardInst {
                def: pc.def.to_string(),
                suit: pc.suit,
                rank: pc.rank,
            },
        );
        ids.push(id);
    }
    st.shuffle(&mut ids);
    st.deck.draw = ids;

    let per = st.config.initial_hand as usize;
    for _ in 0..per {
        for i in 0..st.seats.len() {
            if let Some(id) = st.deck.draw.pop() {
                st.seats[i].hand.push(id);
            }
        }
    }

    let lord_idx = st.lord_index().ok_or_else(|| internal("缺少主公座位"))?;
    st.phase = Phase::Turn;
    st.turn = lord_idx;
    st.turn_phase = TurnPhase::Prepare;
    st.round = 1;
    st.turn_index = 0;

    st.check_conservation().map_err(internal)?;

    let generals: Vec<Value> = st
        .seats
        .iter()
        .map(|s| json!({ "player": s.player_id, "general": s.general, "hp": s.hp, "max_hp": s.max_hp }))
        .collect();
    let mut events = vec![
        EngineEvent::new("setup_done", json!({ "seats": generals })),
        EngineEvent::new(
            "deck_shuffled",
            json!({
                "size": deck_size,
                "packs": st.config.packs,
                "seed": st.deck.seed
            }),
        ),
        EngineEvent::new(
            "initial_hand_dealt",
            json!({ "per_player": per, "total": per * st.seats.len() }),
        ),
    ];
    events.push(EngineEvent::new(
        "turn_started",
        json!({ "round": st.round, "seat": st.turn, "player": st.current_player() }),
    ));
    st.push_log(None, "选将完成，对局开始");
    // 自动推进到需要玩家输入的位置（准备/判定/摸牌 → 出牌或 pending）。
    resolve::advance(st, &mut events)?;
    Ok(events)
}

/// 开赛：发身份 → 主公候选 → 主公先选。
fn start_setup(st: &mut SgsState) -> Result<Vec<EngineEvent>, EngineError> {
    let n = st.seats.len();
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
        return Err(illegal(format!(
            "人数需在 {MIN_PLAYERS}–{MAX_PLAYERS} 之间（当前 {n}）"
        )));
    }
    if st.config.double_spy && n != 8 {
        return Err(illegal("双内局仅支持 8 人"));
    }
    let mut ids = identity_list(n, st.config.double_spy)?;
    st.shuffle(&mut ids);
    for (seat, id) in st.seats.iter_mut().zip(ids.into_iter()) {
        seat.identity = id;
    }
    let lord_idx = st.lord_index().ok_or_else(|| internal("未分配主公身份"))?;
    st.setup = SetupState::default();
    st.setup.lord = st.seats[lord_idx].player_id.clone();
    st.phase = Phase::Setup;
    st.turn = lord_idx;
    st.turn_phase = TurnPhase::Prepare;
    st.round = 1;
    st.turn_index = 0;
    offer_to_lord(st)?;

    let mut events = vec![EngineEvent::new(
        "game_started",
        json!({
            "players": n,
            "lord": st.setup.lord,
            "identities": identity_counts(&st.seats),
            "double_spy": st.config.double_spy,
        }),
    )];

    // 主公是机器人时自动选将（随后会给其他人发候选）。
    events.extend(auto_pick_bots(st)?);
    if st.setup.picks.len() == st.seats.len() {
        events.extend(finish_setup(st)?);
    }
    Ok(events)
}

fn apply_config(st: &mut SgsState, key: &str, value: &str) -> Result<(), EngineError> {
    // 包组合不是数字：先单独处理（如 `standard` / `standard+junzheng`）。
    if key == "packs" {
        st.config.packs = parse_packs(value).map_err(illegal)?;
        return Ok(());
    }
    if key == "general_packs" {
        st.config.general_packs = parse_general_packs(value).map_err(illegal)?;
        return Ok(());
    }
    let v: u32 = value
        .parse()
        .map_err(|_| illegal(format!("非法配置值：{value}")))?;
    match key {
        "lord_candidate_count" => {
            if v > MAX_CANDIDATE_COUNT {
                return Err(illegal(format!("主公候选张数需 ≤ {MAX_CANDIDATE_COUNT}")));
            }
            st.config.lord_candidate_count = v;
        }
        "other_candidate_count" => {
            if v == 0 || v > MAX_CANDIDATE_COUNT {
                return Err(illegal(format!(
                    "其他玩家候选张数需在 1–{MAX_CANDIDATE_COUNT}"
                )));
            }
            st.config.other_candidate_count = v;
        }
        "max_hand_cards" => {
            if v == 0 || v > MAX_HAND_CARDS_LIMIT {
                return Err(illegal(format!("持有上限需在 1–{MAX_HAND_CARDS_LIMIT}")));
            }
            st.config.max_hand_cards = v;
        }
        "double_spy" => st.config.double_spy = v != 0,
        _ => return Err(illegal(format!("未知配置项：{key}"))),
    }
    Ok(())
}

fn new_bot_id(st: &SgsState) -> String {
    for i in 1..=1000 {
        let id = format!("bot:{i}");
        if !st.seats.iter().any(|s| s.player_id == id) {
            return id;
        }
    }
    "bot:overflow".to_string()
}

fn describe(st: &SgsState) -> String {
    match st.phase {
        Phase::Lobby => format!("等待加入（{}/{}）", st.seats.len(), MAX_PLAYERS),
        Phase::Setup => format!(
            "选将中（已选 {}/{}）；主公已选：{}",
            st.setup.picks.len(),
            st.seats.len(),
            st.setup.lord_picked
        ),
        Phase::Turn | Phase::Dying => {
            let cur = st.current_player().unwrap_or("-");
            format!("第 {} 轮，轮到 {cur}", st.round)
        }
        Phase::Finished => "对局已结束".to_string(),
    }
}

pub struct SanguoshaEngine;

impl EnginePlugin for SanguoshaEngine {
    fn game_id(&self) -> &'static str {
        "sanguosha"
    }

    fn rules(&self) -> String {
        "三国杀（标准身份局）：5–8 人。随机发身份（主公/忠臣/反贼/内奸，主公公开）；\
         主公先选将（全部主公将 + N 张非主公，N 默认 5），其他玩家同时各获得 M 张候选（默认 5，互不重复）；\
         主公体力 +1；每人初始手牌 4 张。回合：准备→判定→摸牌→出牌→弃牌→结束；\
         判定区延时锦囊后进先判（LIFO）；持有上限默认 20。胜利：主公/忠臣消灭所有反贼与内奸；\
         反贼杀死主公（主公先死即反贼胜）；内奸需在其他角色全部死亡后杀死主公。"
            .to_string()
    }

    fn new_game(
        &self,
        room_id: &str,
        group_id: &str,
        players: Vec<String>,
        rng: &mut dyn RandomSource,
    ) -> (GameState, Vec<EngineEvent>) {
        // `create_game` 保证 players = [host]
        let host = players
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let seed = rng.next_u64();
        let st = SgsState {
            schema_version: SCHEMA_VERSION,
            phase: Phase::Lobby,
            host: host.clone(),
            config: Config::default(),
            seats: players
                .iter()
                .map(|p| Seat::new(p.clone(), false))
                .collect(),
            turn: 0,
            turn_phase: TurnPhase::Prepare,
            turn_index: 0,
            round: 1,
            cards: Default::default(),
            virtual_defs: Default::default(),
            deck: Deck::empty(seed),
            setup: SetupState::default(),
            pending: None,
            pending_queue: Vec::new(),
            skill_subscriptions: Vec::new(),
            special_zones: Default::default(),
            phase_discarded: Vec::new(),
            shenshi_marks: Vec::new(),
            scheduled: Vec::new(),
            void_return: Vec::new(),
            pending_liyu: Vec::new(),
            extra_turn: None,
            winner: None,
            abort_reason: None,
            turn_counters: Default::default(),
            resolution: Vec::new(),
            outcome: None,
            history: Vec::new(),
            history_seq: 0,
            rng_state: seed | 1,
        };
        let state = GameState {
            game_id: self.game_id().to_string(),
            room_id: room_id.to_string(),
            group_id: group_id.to_string(),
            status: GameStatus::Active,
            players,
            data: serde_json::to_value(&st).unwrap_or_else(|_| json!({})),
        };
        let events = vec![EngineEvent::new(
            "game_created",
            json!({ "host": host, "group_id": group_id }),
        )];
        (state, events)
    }

    fn legal_actions(&self, state: &GameState, player_id: &str) -> Vec<Action> {
        let Ok(st) = parse(&state.data) else {
            return Vec::new();
        };
        let seated = st.seat_index(player_id);
        let mut out = Vec::new();
        match st.phase {
            Phase::Lobby => {
                if seated.is_none() {
                    if st.seats.len() < MAX_PLAYERS {
                        out.push(Action {
                            id: "lobby:join".into(),
                            description: "加入游戏".into(),
                        });
                    }
                    return out;
                }
                out.push(Action {
                    id: "lobby:leave".into(),
                    description: "退出房间".into(),
                });
                if st.host == player_id {
                    if (MIN_PLAYERS..=MAX_PLAYERS).contains(&st.seats.len())
                        && !(st.config.double_spy && st.seats.len() != 8)
                    {
                        out.push(Action {
                            id: "lobby:start".into(),
                            description: "开始游戏（发身份 → 选将）".into(),
                        });
                    }
                    if st.seats.len() < MAX_PLAYERS {
                        out.push(Action {
                            id: "lobby:add_bot".into(),
                            description: "加入机器人".into(),
                        });
                    }
                    out.push(Action {
                        id: "lobby:abort".into(),
                        description: "中断本局（不结算）".into(),
                    });
                    for s in &st.seats {
                        if s.player_id != st.host && !s.is_bot {
                            out.push(Action {
                                id: format!("lobby:kick:{}", s.player_id),
                                description: format!("踢出 {}", s.player_id),
                            });
                        }
                    }
                    let presets_lord = [3u32, 5, 7];
                    for v in presets_lord {
                        if v != st.config.lord_candidate_count {
                            out.push(Action {
                                id: format!("lobby:set_lord_candidate_count:{v}"),
                                description: format!("主公候选非主公将 {v} 张"),
                            });
                        }
                    }
                    for v in [3u32, 5, 7] {
                        if v != st.config.other_candidate_count {
                            out.push(Action {
                                id: format!("lobby:set_other_candidate_count:{v}"),
                                description: format!("其他玩家候选 {v} 张"),
                            });
                        }
                    }
                    for v in [10u32, 20, 30] {
                        if v != st.config.max_hand_cards {
                            out.push(Action {
                                id: format!("lobby:set_max_hand_cards:{v}"),
                                description: format!("手牌持有上限 {v}"),
                            });
                        }
                    }
                    if st.seats.len() == 8 {
                        out.push(Action {
                            id: format!("lobby:set_double_spy:{}", u8::from(!st.config.double_spy)),
                            description: if st.config.double_spy {
                                "关闭双内局".into()
                            } else {
                                "开启双内局（1 主 1 忠 4 反 2 内）".into()
                            },
                        });
                    }
                    // 牌堆包组合（全局开关；旧对局按创建时的 config 固定）
                    for mode in ["standard", "standard+junzheng"] {
                        if st.config.packs_mode() != mode {
                            out.push(Action {
                                id: format!("lobby:set_packs:{mode}"),
                                description: if mode == "standard" {
                                    "牌堆：标准包（108 张）".into()
                                } else {
                                    "牌堆：标准 + 军争（160 张）".into()
                                },
                            });
                        }
                    }
                    // 武将包组合（全局开关；旧对局按创建时的 config 固定）
                    for mode in general_pack_presets() {
                        if st.config.general_packs_mode() != mode {
                            let (label, count) = general_pack_mode_label(&mode);
                            out.push(Action {
                                id: format!("lobby:set_general_packs:{mode}"),
                                description: format!("武将包：{label}（可用 {count} 位）"),
                            });
                        }
                    }
                }
            }
            Phase::Setup => {
                if let Some(idx) = seated {
                    let pid = &st.seats[idx].player_id;
                    if !st.setup.picks.contains_key(pid) {
                        if let Some(offers) = st.setup.offers.get(pid) {
                            for code in offers {
                                if let Some(def) = data::general(code) {
                                    out.push(Action {
                                        id: format!("setup:choose_general:{code}"),
                                        description: format!("选择武将：{}", def.name),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Phase::Turn | Phase::Dying => {
                out = turn::legal_actions(&st, player_id);
            }
            Phase::Finished => {}
        }
        out
    }

    fn execute_action(
        &self,
        state: &GameState,
        player_id: &str,
        action: &str,
        _rng: &mut dyn RandomSource,
    ) -> Result<ActionResult, EngineError> {
        let mut st = parse(&state.data)?;
        if st.phase.is_over() {
            return Err(illegal("对局已结束"));
        }

        if action == "resume" || action == "hint" {
            return Ok(ActionResult {
                state: build_state(state, &st),
                message: describe(&st),
                finished: st.phase.is_over(),
                events: Vec::new(),
            });
        }

        let mut events: Vec<EngineEvent> = Vec::new();
        let message: String;

        if let Some(rest) = action.strip_prefix("lobby:") {
            // `abort` 是运维动作：任何未结束阶段都可中断（P7 `#中断` 依赖）。
            if st.phase != Phase::Lobby && rest != "abort" {
                return Err(illegal("已开局，不能修改房间"));
            }
            let seated = st.seat_index(player_id).is_some();
            message = match rest {
                "join" => {
                    if seated {
                        return Err(illegal("已在房间中"));
                    }
                    if st.seats.len() >= MAX_PLAYERS {
                        return Err(illegal("房间已满"));
                    }
                    st.seats.push(Seat::new(player_id.to_string(), false));
                    events.push(
                        EngineEvent::new(
                            "lobby_join",
                            json!({ "player": player_id, "seats": st.seats.len() }),
                        )
                        .with_actor(player_id),
                    );
                    format!("已加入（{}/{}）", st.seats.len(), MAX_PLAYERS)
                }
                "leave" => {
                    let idx = st
                        .seat_index(player_id)
                        .ok_or_else(|| illegal("不在房间中"))?;
                    if st.host == player_id {
                        st.phase = Phase::Finished;
                        st.abort_reason = Some("房主离开，对局废弃".into());
                        events.push(
                            EngineEvent::new("lobby_abort", json!({ "reason": "host_left" }))
                                .with_actor(player_id),
                        );
                        "房主已离开，对局废弃".to_string()
                    } else {
                        st.seats.remove(idx);
                        events.push(
                            EngineEvent::new("lobby_leave", json!({ "player": player_id }))
                                .with_actor(player_id),
                        );
                        format!("已退出（{}/{}）", st.seats.len(), MAX_PLAYERS)
                    }
                }
                "start" => {
                    require_host(&st, player_id)?;
                    if st.seats.len() < MIN_PLAYERS {
                        return Err(illegal(format!("至少 {MIN_PLAYERS} 人才能开始")));
                    }
                    events.extend(start_setup(&mut st)?);
                    if st.phase == Phase::Setup {
                        format!(
                            "已开局：{} 人，主公 {} 先选将",
                            st.seats.len(),
                            st.setup.lord
                        )
                    } else {
                        "已开局：选将完成，主公先手".to_string()
                    }
                }
                "abort" => {
                    require_host(&st, player_id)?;
                    st.phase = Phase::Finished;
                    st.abort_reason = Some("房主中断".into());
                    events.push(
                        EngineEvent::new("lobby_abort", json!({ "reason": "host_abort" }))
                            .with_actor(player_id),
                    );
                    "已中断本局（不结算）".to_string()
                }
                "add_bot" => {
                    require_host(&st, player_id)?;
                    if st.seats.len() >= MAX_PLAYERS {
                        return Err(illegal("房间已满"));
                    }
                    let bot = new_bot_id(&st);
                    st.seats.push(Seat::new(bot.clone(), true));
                    events.push(
                        EngineEvent::new("lobby_add_bot", json!({ "bot": bot }))
                            .with_actor(player_id),
                    );
                    format!("已加入机器人 {bot}（{}/{}）", st.seats.len(), MAX_PLAYERS)
                }
                _ if rest.starts_with("kick:") => {
                    require_host(&st, player_id)?;
                    let target = &rest["kick:".len()..];
                    if target == st.host {
                        return Err(illegal("不能踢出房主"));
                    }
                    let idx = st
                        .seat_index(target)
                        .ok_or_else(|| illegal("目标不在房间中"))?;
                    st.seats.remove(idx);
                    events.push(
                        EngineEvent::new("lobby_kick", json!({ "player": target }))
                            .with_actor(player_id),
                    );
                    format!("已踢出 {target}")
                }
                _ if rest.starts_with("set_") => {
                    require_host(&st, player_id)?;
                    let mut it = rest.splitn(2, ':');
                    let key = it.next().unwrap_or_default();
                    let value = it.next().ok_or_else(|| illegal("缺少配置值"))?;
                    let key = key.trim_start_matches("set_");
                    apply_config(&mut st, key, value)?;
                    events.push(
                        EngineEvent::new("lobby_config", json!({ "key": key, "value": value }))
                            .with_actor(player_id),
                    );
                    format!("已设置 {key} = {value}")
                }
                _ => return Err(illegal(format!("未知房间动作：{rest}"))),
            };
        } else if let Some(rest) = action.strip_prefix("setup:choose_general:") {
            if st.phase != Phase::Setup {
                return Err(illegal("当前不是选将阶段"));
            }
            let def = data::general(rest).ok_or_else(|| illegal("未知武将"))?;
            events.extend(do_pick(&mut st, player_id, rest)?);
            message = format!("已选择武将：{}", def.name);
        } else if matches!(st.phase, Phase::Turn | Phase::Dying) {
            let (msg, evs) = turn::execute(&mut st, player_id, action)?;
            events.extend(evs);
            message = msg;
        } else {
            return Err(illegal(format!("未知动作：{action}")));
        }

        // 自动补选机器人 + 全员选完则发牌
        events.extend(auto_pick_bots(&mut st)?);
        if st.phase == Phase::Setup && st.setup.picks.len() == st.seats.len() {
            events.extend(finish_setup(&mut st)?);
        }

        st.check_conservation().map_err(internal)?;
        let finished = st.phase.is_over();
        Ok(ActionResult {
            state: build_state(state, &st),
            message,
            finished,
            events,
        })
    }

    fn public_state(&self, state: &GameState) -> GameState {
        let Ok(st) = parse(&state.data) else {
            return state.clone();
        };
        let mut out = state.clone();
        out.data = view::view_data(&st, None);
        out
    }

    fn player_view(&self, state: &GameState, viewer: &str) -> GameState {
        let Ok(st) = parse(&state.data) else {
            return state.clone();
        };
        let mut out = state.clone();
        out.data = view::view_data(&st, Some(viewer));
        out
    }

    fn summarize(&self, state: &GameState) -> String {
        let Ok(st) = parse(&state.data) else {
            return "三国杀：状态异常".to_string();
        };
        let status = if st.phase.is_over() {
            "已结束"
        } else {
            "进行中"
        };
        match st.phase {
            Phase::Lobby => format!(
                "三国杀（{status}）：{}/{} 人在房间，等待房主开始",
                st.seats.len(),
                MAX_PLAYERS
            ),
            Phase::Setup => format!(
                "三国杀（{status}）：{} 人，选将中（{}/{}），主公已选：{}",
                st.seats.len(),
                st.setup.picks.len(),
                st.seats.len(),
                st.setup.lord_picked
            ),
            Phase::Turn | Phase::Dying => {
                let cur = st
                    .current_player()
                    .map(|p| {
                        st.seat(p)
                            .map(|s| s.general_name.clone())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                format!(
                    "三国杀（{status}）：第 {} 轮，轮到 {cur}，牌堆余 {}",
                    st.round,
                    st.deck.draw.len()
                )
            }
            Phase::Finished => match &st.winner {
                Some(w) => format!("三国杀（{status}）：{} 阵营胜利", w.team),
                None => format!(
                    "三国杀（{status}）：{}",
                    st.abort_reason.as_deref().unwrap_or("已结束")
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests;
