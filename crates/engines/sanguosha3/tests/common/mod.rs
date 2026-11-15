//! 测试夹具（P1–P6 门禁共用）：座位/牌/牌堆/回合推进。
#![allow(dead_code)]

use sanguosha3::kernel::resolve;
use sanguosha3::kernel::turn;
use sanguosha3::kernel::{Card, Game, Identity, Suit, Zone};

pub fn add(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add_suit(g, id, def, zone, Suit::Spade, 7);
}

pub fn add_suit(g: &mut Game, id: &str, def: &str, zone: Zone, suit: Suit, rank: u8) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card {
            id: id.to_string(),
            def: def.to_string(),
            suit,
            rank,
        },
    );
}

/// 牌堆（全部是【杀】，避免摸牌干扰响应判定）。
pub fn fill_deck(g: &mut Game, n: usize) {
    for i in 0..n {
        let id = format!("deck{i}");
        add(g, &id, "sha", Zone::Draw);
        g.deck.push(id);
    }
}

/// 空白对局：n 个存活座位，体力 4。
pub fn game(n: usize) -> Game {
    let mut g = turn::new_game(n);
    fill_deck(&mut g, 80);
    g
}

/// 开局（发身份 + 每人 4 张），**不**进入回合（测试可在判定区/牌堆顶预置）。
pub fn begin(g: &mut Game) {
    let identities: Vec<Identity> = (1..g.seats.len())
        .map(|i| {
            if i % 2 == 1 {
                Identity::Rebel
            } else {
                Identity::Loyalist
            }
        })
        .collect();
    turn::begin_game(g, &identities, 4);
}

/// 清空某人手牌（进弃牌堆）——让响应/弃牌断言可控。
pub fn clear_hand(g: &mut Game, seat: usize) {
    for id in g.hand(seat) {
        g.cards.insert(id, sanguosha3::kernel::Zone::Discard);
    }
}

/// 开局（发身份 + 每人 4 张）并进入座位 0 的出牌阶段。
pub fn start_play(g: &mut Game) {
    begin(g);
    turn::start(g).expect("开局");
}

pub fn submit(g: &mut Game, seat: usize, option: &str) -> Result<(), String> {
    resolve::submit(g, seat, option)
}

/// 结束当前回合的出牌阶段并弃到上限（返回跳过的回合数）。
pub fn end_turn(g: &mut Game) -> Result<(), String> {
    let seat = g.turn;
    submit(g, seat, "end_play")?;
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "discard_phase")
        .unwrap_or(false)
    {
        let hand = g.hand(seat);
        assert!(!hand.is_empty(), "弃牌阶段必有手牌");
        submit(g, seat, &format!("card:{}", hand[0]))?;
    }
    Ok(())
}

/// 使用一张手牌并推进到第一个交互点（与出牌阶段 `card:<id>` 选项同一结算路径）。
pub fn use_card(g: &mut Game, seat: usize, id: &str) -> Result<(), String> {
    // 夹具专用：清掉「按此刻手牌生成」的出牌待办（真实对局中待办会随动作重建，
    // 只有测试在待办生成后额外往手里塞牌）。
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, false)?;
    sanguosha3::kernel::resolve::advance(g)
}

pub fn use_card_fire(g: &mut Game, seat: usize, id: &str) -> Result<(), String> {
    g.pending = None;
    sanguosha3::kernel::card_play::begin_use(g, seat, id, true)?;
    sanguosha3::kernel::resolve::advance(g)
}

/// 重建当前交互（把待办清掉后推进：回合帧会按**此刻手牌**重新生成出牌选项）。
pub fn refresh(g: &mut Game) {
    g.pending = None;
    let _ = resolve::advance(g);
}

pub fn pending_actor(g: &Game) -> Option<usize> {
    g.pending.as_ref().map(|p| p.actor())
}

pub fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}

pub fn has_option(g: &Game, id: &str) -> bool {
    g.pending
        .as_ref()
        .map(|p| p.has_option(id))
        .unwrap_or(false)
}
