//! B0 批次（6 技能收口）：解围 / 不屈 / 奋激 / 行殇 / 放逐 / 颂威。
//!
//! 每条对齐 `docs/games/sanguosha/align/B0.json` 的 `current` 步骤与已裁定问题（QQn）。

mod common;

use common::{add, add_suit, begin, game, has_option, pending_actor, submit, use_card};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::resolve::{advance, FrameInner, FlowKind};
use sanguosha3::kernel::skills;
use sanguosha3::kernel::{flow, judge, turn, Card, Game, Identity, Suit, Zone};

fn card(g: &mut Game, id: &str, def: &str, zone: Zone) {
    add(g, id, def, zone);
}

fn clear_all(g: &mut Game) {
    for s in 0..g.seats.len() {
        for id in g.hand(s) {
            g.cards.insert(id, Zone::Discard);
        }
    }
}

fn in_dying_frame(g: &Game, seat: usize) -> bool {
    g.stack.iter().any(|f| {
        matches!(&f.inner, FrameInner::Flow(ff)
            if ff.kind == FlowKind::Dying
                && ff.data.get("target").and_then(|v| v.as_u64()) == Some(seat as u64))
    })
}

// ---------------------------------------------------------------------------
// 解围①：装备区牌当【无懈可击】
// ---------------------------------------------------------------------------

#[test]
fn jiewei_equipment_can_be_used_as_wuxie() {
    let mut g = game(3);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    clear_all(&mut g);
    g.seats[2].skills = vec!["jiewei".into()];
    card(&mut g, "eq", "qinggang_jian", Zone::Equip(2, sanguosha3::kernel::Slot::Weapon));
    card(&mut g, "wz", "wuzhong_shengyou", Zone::Hand(0));
    let before = g.hand_size(0);
    use_card(&mut g, 0, "wz").unwrap();
    // 无懈窗口：座位 0/1 无无懈 → 座位 2 可用装备牌当无懈。
    assert_eq!(pending_actor(&g), Some(2), "轮到解围拥有者");
    assert!(has_option(&g, "equip:eq"), "解围①：装备牌当【无懈可击】");
    submit(&mut g, 2, "equip:eq").unwrap();
    assert_eq!(g.hand_size(0), before - 1, "无中被无懈：不摸牌");
    assert_eq!(g.cards.get("eq"), Some(&Zone::Discard), "装备牌作为无懈结算后进弃牌堆");
    assert!(has_option(&g, "end_play") || g.pending.is_none());
}

// ---------------------------------------------------------------------------
// 解围②：翻回正面 → 弃一张 → 移动场上一张牌
// ---------------------------------------------------------------------------

#[test]
fn jiewei_triggers_on_flip_back_and_moves_a_card() {
    let mut g = game(3);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    clear_all(&mut g);
    g.seats[0].skills = vec!["jiewei".into()];
    g.seats[0].flipped = true; // 背面 → 翻回正面
    card(&mut g, "own", "tao", Zone::Hand(0));
    card(&mut g, "opp", "tao", Zone::Hand(1));
    g.pending = None; // 测试直接注入翻面（真实对局由放逐/据守驱动，不占出牌待办）
    skills::flip_seat(&mut g, 0);
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("jiewei_ask".into()),
        "翻回正面触发解围②"
    );
    submit(&mut g, 0, "choose:jiewei").unwrap();
    // ① 弃自己一张牌
    assert_eq!(g.pending.as_ref().unwrap().kind(), "jiewei_discard");
    submit(&mut g, 0, "card:own").unwrap();
    // ② 选来源
    assert_eq!(g.pending.as_ref().unwrap().kind(), "jiewei_source");
    assert!(has_option(&g, "t:1"));
    submit(&mut g, 0, "t:1").unwrap();
    // ③ 选牌（随机手牌）
    assert_eq!(g.pending.as_ref().unwrap().kind(), "jiewei_pick");
    assert!(has_option(&g, "hand"));
    submit(&mut g, 0, "hand").unwrap();
    // ④ 选目的地（来源排除）
    assert_eq!(g.pending.as_ref().unwrap().kind(), "jiewei_dest");
    assert!(!has_option(&g, "t:1"), "来源一律排除");
    submit(&mut g, 0, "t:2").unwrap();
    assert_eq!(g.cards.get("opp"), Some(&Zone::Hand(2)), "移动一张手牌到目的地");
    assert_eq!(g.cards.get("own"), Some(&Zone::Discard), "解围弃牌进弃牌堆");
}

// ---------------------------------------------------------------------------
// 不屈：点数重复 → 仍濒死；创 → 移出游戏 + 手牌上限=创数
// ---------------------------------------------------------------------------

#[test]
fn buqu_duplicate_rank_goes_to_discard_not_void() {
    let mut g = game(3);
    begin(&mut g);
    g.seats[0].skills = vec!["buqu".into()];
    g.seats[0].hp = 0;
    g.set_counter("buqu_rank.0.7", 1); // 已有 7 点「创」
    add_suit(&mut g, "jt", "sha", Zone::Draw, Suit::Spade, 7);
    g.deck.push("jt".into());
    flow::enter_dying(&mut g, 0, None);
    assert_eq!(g.seats[0].hp, 0, "点数重复：仍濒死");
    assert_eq!(g.cards.get("jt"), Some(&Zone::Discard), "重复的判定牌进弃牌堆（QQ1=A）");
    assert!(in_dying_frame(&g, 0), "继续按濒死流程求桃");
}

// ---------------------------------------------------------------------------
// 奋激：回合外手牌变动 → 询问；自己回合/摸牌阶段不触发
// ---------------------------------------------------------------------------

#[test]
fn fenji_asks_on_other_discard_out_of_turn() {
    let mut g = game(4);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    g.seats[1].skills = vec!["fenji".into()];
    g.turn = 0; // 座位 0 的回合（座位 1 回合外）
    clear_all(&mut g);
    add(&mut g, "c1", "tao", Zone::Hand(2));
    let (hp0, hand2) = (g.seats[1].hp, g.hand_size(2));
    // 座位 2 的手牌被弃置（回合外）
    g.pending = None; // 测试直接注入失牌（真实由结算路径触发）
    flow::discard(&mut g, 2, "c1", "测试");
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into()),
        "奋激询问"
    );
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "choose:fenji").unwrap();
    assert_eq!(g.seats[1].hp, hp0 - 1, "失去 1 点体力");
    assert_eq!(g.hand_size(2), hand2 - 1 + 2, "失牌角色摸 2（先失 1 后摸 2）");

    // 自己回合内不触发。
    let mut g2 = game(3);
    begin(&mut g2);
    turn::start(&mut g2).unwrap();
    g2.seats[0].skills = vec!["fenji".into()];
    g2.turn = 0;
    add(&mut g2, "c2", "tao", Zone::Hand(1));
    flow::discard(&mut g2, 1, "c2", "测试");
    advance(&mut g2).unwrap();
    assert_ne!(
        g2.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fenji_ask".into()),
        "自己的回合不触发奋激"
    );

    // 摸牌阶段摸牌不触发。
    let mut g3 = game(3);
    begin(&mut g3);
    g3.started = true;
    g3.seats[1].skills = vec!["fenji".into()];
    g3.turn = 0;
    g3.phase = sanguosha3::kernel::Phase::Draw;
    flow::draw(&mut g3, 2, 1);
    assert!(!g3
        .stack
        .iter()
        .any(|f| matches!(&f.inner, FrameInner::Ask(a) if a.skill == "fenji")), "摸牌阶段摸牌不触发");
}

// ---------------------------------------------------------------------------
// 行殇：死亡清理前询问 → 获得手牌+装备（不含判定区）→ 收口死亡结算
// ---------------------------------------------------------------------------

#[test]
fn xingshang_on_death_gains_cards_and_finishes_death() {
    let mut g = game(4);
    turn::begin_game(&mut g, &[Identity::Rebel, Identity::Rebel, Identity::Loyalist], 4);
    g.seats[0].skills = vec!["xingshang".into()];
    add(&mut g, "h1", "tao", Zone::Hand(2));
    add(&mut g, "e1", "qinggang_jian", Zone::Equip(2, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "j1", "lebu_sishu", Zone::Judge(2));
    g.judge_stacks.entry(2).or_default().push("j1".into());
    turn::settle_death(&mut g, 2, Some(1));
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("xingshang_ask".into()),
        "行殇在死亡清理前询问"
    );
    assert_eq!(pending_actor(&g), Some(0));
    assert!(g.log.iter().all(|e| e.kind != EventKind::Death), "死亡事件尚未收口");
    submit(&mut g, 0, "choose:xingshang").unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Hand(0)), "获得手牌");
    assert_eq!(g.cards.get("e1"), Some(&Zone::Hand(0)), "获得装备");
    // 行殇获得后按持有上限弃牌，再收口死亡结算（判定区随清理进弃牌堆）。
    let mut guard = 0;
    while g
        .pending
        .as_ref()
        .map(|p| p.kind() == "skill_limit")
        .unwrap_or(false)
    {
        guard += 1;
        assert!(guard < 20, "行殇上限弃牌未收口");
        let id = g.hand(0)[0].clone();
        submit(&mut g, 0, &format!("card:{id}")).unwrap();
    }
    assert_eq!(g.cards.get("j1"), Some(&Zone::Discard), "判定区不获得（随死亡清理弃置）");
    assert!(g.log.iter().any(|e| e.kind == EventKind::Death), "死亡事件已收口");
    assert_eq!(g.check_conservation(), Ok(()));
}

// ---------------------------------------------------------------------------
// 放逐：受伤后选目标 → 先翻面 → 摸 X（无上限）
// ---------------------------------------------------------------------------

#[test]
fn fangzhu_asks_target_then_flips_and_draws() {
    let mut g = game(3);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    g.seats[0].skills = vec!["fangzhu".into()];
    g.seats[0].hp = 1; // 已损失 3
    clear_all(&mut g);
    let spec = sanguosha3::kernel::DamageSpec {
        from: Some(1),
        target: 0,
        amount: 1,
        kind: sanguosha3::kernel::DamageKind::Normal,
        source: sanguosha3::kernel::SourceKind::Skill,
    };
    flow::damage(&mut g, spec, sanguosha3::kernel::ArmorKind::None, 0);
    // 濒死优先（座位 0 体力 0）→ 先求桃；这里直接跳过：把体力写回 1 模拟「被救起」。
    g.seats[0].hp = 1;
    g.pending = None;
    while g.stack.iter().any(|f| matches!(&f.inner, FrameInner::Flow(ff) if ff.kind == FlowKind::Dying)) {
        g.stack.retain(|f| !matches!(&f.inner, FrameInner::Flow(ff) if ff.kind == FlowKind::Dying));
    }
    advance(&mut g).unwrap();
    assert_eq!(
        g.pending.as_ref().map(|p| p.kind().to_string()),
        Some("fangzhu_target".into()),
        "放逐选目标"
    );
    let hand1 = g.hand_size(1);
    submit(&mut g, 0, "t:1").unwrap();
    assert!(g.seats[1].flipped, "目标翻面（取反）");
    assert_eq!(g.hand_size(1), hand1 + 3, "摸 X=已损失体力（无 min 5 上限）");
}

// ---------------------------------------------------------------------------
// 颂威：其他魏势力黑色判定生效后 → 询问判定者
// ---------------------------------------------------------------------------

#[test]
fn songwei_asks_the_judging_player() {
    let mut g = game(3);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    g.seats[0].skills = vec!["songwei".into()];
    g.seats[1].kingdom = "wei".into();
    g.seats[1].identity = Some(Identity::Rebel);
    // 黑色判定牌
    add_suit(&mut g, "jt", "sha", Zone::Draw, Suit::Spade, 9);
    g.deck.push("jt".into());
    let lord_hand = g.hand_size(0);
    g.pending = None; // 测试直接注入判定（真实由判定阶段/延时锦囊触发）
    judge::judge(&mut g, 1, "测试判定");
    advance(&mut g).unwrap();
    // 可能先停在别处：找 songwei 询问并选择。
    if g.pending.as_ref().map(|p| p.kind().to_string()) == Some("songwei_ask".into()) {
        assert_eq!(pending_actor(&g), Some(1), "询问判定者");
        submit(&mut g, 1, "choose:songwei").unwrap();
        assert_eq!(g.hand_size(0), lord_hand + 1, "主公摸 1 张");
    } else {
        panic!("未产生颂威询问：{:?}", g.pending.as_ref().map(|p| p.kind().to_string()));
    }
}

#[test]
fn songwei_not_triggered_on_red_judge() {
    let mut g = game(3);
    begin(&mut g);
    turn::start(&mut g).unwrap();
    g.seats[0].skills = vec!["songwei".into()];
    g.seats[1].kingdom = "wei".into();
    add_suit(&mut g, "jt", "tao", Zone::Draw, Suit::Heart, 9);
    g.deck.push("jt".into());
    judge::judge(&mut g, 1, "测试判定");
    assert!(!g
        .stack
        .iter()
        .any(|f| matches!(&f.inner, FrameInner::Ask(a) if a.skill == "songwei")), "红色判定不触发颂威");
}

// 供上面用例复用的断言辅助（避免未使用导入告警）。
#[allow(dead_code)]
fn _unused(g: &Game) -> bool {
    let _ = Card {
        id: "x".into(),
        def: "sha".into(),
        suit: Suit::Spade,
        rank: 1,
    };
    let _ = card;
    g.cards.is_empty()
}
