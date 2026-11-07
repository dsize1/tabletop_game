//! B1 第一批（庸肆 / 奇制 / 进趋 / 怀橘 / 寸目）验收测试。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{Card, Game, Phase, Seat, Suit, Zone};
use sanguosha2::skills;

fn card(id: &str, name: &str, suit: Suit, rank: u8) -> Card {
    Card::new(id, name, suit, rank)
}

/// 4 人：袁术(魏) / 王基(蜀) / 陆绩(吴) / 许攸(群)——势力数 4。
fn b1_game() -> Game {
    let mut a = Seat::new("p0", "yuanshu", &["yongsi"], 4);
    a.kingdom = "wei".to_string();
    let mut b = Seat::new("p1", "wangji", &["qizhi", "jinzou"], 3);
    b.kingdom = "shu".to_string();
    let mut c = Seat::new("p2", "luji", &["huaiju"], 3);
    c.kingdom = "wu".to_string();
    let mut d = Seat::new("p3", "xuyou", &["cunmu"], 3);
    d.kingdom = "qun".to_string();
    let cards: Vec<Card> = (1..=20)
        .map(|i| {
            let suit = match i % 4 {
                0 => Suit::Spade,
                1 => Suit::Heart,
                2 => Suit::Club,
                _ => Suit::Diamond,
            };
            card(&format!("c{i}"), "杀", suit, (i % 13) as u8)
        })
        .collect();
    let mut g = Game::new(vec![a, b, c, d], cards);
    for seat in 0..4 {
        g.to_hand(&format!("c{}", seat + 1), seat).unwrap();
    }
    g
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    g.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 庸肆
// ---------------------------------------------------------------------------

#[test]
fn yongsi_draws_2_plus_kingdom_count() {
    let mut g = b1_game();
    let before = g.hand(0).len();
    g.draw_phase(0);
    assert_eq!(g.hand(0).len(), before + 6, "2 + 4 个势力");
}

#[test]
fn yongsi_discard_phase_discards_x_and_counts_them() {
    let mut g = b1_game();
    // 给袁术 6 张手牌
    for i in 5..=9 {
        g.to_hand(&format!("c{i}"), 0).unwrap();
    }
    let before = g.hand(0).len();
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    assert_eq!(g.hand(0).len(), before - 4, "弃牌阶段先弃 X=4");
    assert!(g.phase_discarded >= 4, "应计入本阶段弃牌数（溃诛用）");
}

// ---------------------------------------------------------------------------
// 奇制
// ---------------------------------------------------------------------------

#[test]
fn qizhi_triggers_on_targeted_card_use() {
    let mut g = b1_game();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c2".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("qizhi"));
    let target = pend(&g).options()[0].id.clone();
    answer(&mut g, 1, &target);
    let card = pend(&g).options()[0].id.clone();
    let target_hand = g.hand(0).len();
    let discard_before = g.discard_pile().len();
    answer(&mut g, 1, &card);
    assert_eq!(g.counter("turn.qizhi_count"), 1);
    assert_eq!(g.hand(0).len(), target_hand, "弃 1 再摸 1 → 手牌数不变");
    assert!(g.discard_pile().len() >= discard_before + 1, "应有一张牌进弃牌堆");
}

#[test]
fn qizhi_does_not_trigger_without_target() {
    let mut g = b1_game();
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c2".to_string(), vec![]); // 无目标牌（如无中生有）
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "无目标牌不得触发奇制");
}

// ---------------------------------------------------------------------------
// 进趋
// ---------------------------------------------------------------------------

#[test]
fn jinzou_not_allowed_when_qizhi_count_is_zero() {
    let mut g = b1_game();
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert!(g.pending.is_none(), "X=0 时不允许发动进趋");
}

#[test]
fn jinzou_draws_then_discards_down_to_x() {
    let mut g = b1_game();
    // 真跑一次奇制，让 X=1（也顺带验证两技能联动）
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c2".to_string(), vec![0]);
    advance(&mut g).unwrap();
    let target = pend(&g).options()[0].id.clone();
    answer(&mut g, 1, &target);
    let card = pend(&g).options()[0].id.clone();
    answer(&mut g, 1, &card);
    assert_eq!(g.counter("turn.qizhi_count"), 1);
    // 进入同一回合的结束阶段（turn 不变 → turn.* 计数保留）
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert_eq!(pend(&g).skill(), Some("jinzou"));
    answer(&mut g, 1, "choose:jinzou");
    // 摸 2 后手牌 > 1 → 逐张弃到 1
    let mut guard = 0;
    while g.pending.is_some() && guard < 12 {
        guard += 1;
        let id = pend(&g).options()[0].id.clone();
        answer(&mut g, 1, &id);
    }
    assert_eq!(g.hand(1).len(), 1, "应弃至 X=1");
}

// ---------------------------------------------------------------------------
// 怀橘 / 寸目
// ---------------------------------------------------------------------------

#[test]
fn huaiju_gives_three_ju_and_extra_draw_and_prevents_damage() {
    let mut g = b1_game();
    assert_eq!(skills::ju(&g, 2), 3, "开局 3 枚橘");
    let before = g.hand(2).len();
    g.draw_phase(2);
    assert_eq!(g.hand(2).len(), before + 3, "2 + 怀橘 1");
    let hp = g.seats[2].hp;
    sanguosha2::kernel::flow::damage(&mut g, Some(0), 2, 1);
    assert_eq!(g.seats[2].hp, hp, "怀橘应防止伤害");
    assert_eq!(skills::ju(&g, 2), 2);
}

#[test]
fn cunmu_draws_from_bottom_of_deck() {
    let mut g = b1_game();
    // 牌堆剩余牌里最后一张 = 牌堆底
    let bottom = g.ids_in(|z| *z == Zone::Draw).last().cloned().unwrap();
    let drawn = g.draw(3, 1);
    assert_eq!(drawn, vec![bottom], "寸目从牌堆底摸");
}
