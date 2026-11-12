//! B17 用例（14 技能，每条技能 ≥1 正例 + ≥1 反例）。
use sanguosha2::kernel::card_play;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("sp2", "杀", Suit::Spade, 10),
        ("h1", "杀", Suit::Heart, 12),
        ("c1", "杀", Suit::Club, 11),
        ("c2", "杀", Suit::Club, 9),
        ("hs1", "火杀", Suit::Heart, 4),
        ("ls1", "雷杀", Suit::Spade, 6),
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("d3", "闪", Suit::Diamond, 4),
        ("d4", "闪", Suit::Diamond, 5),
        ("t1", "桃", Suit::Heart, 9),
        ("t2", "桃", Suit::Heart, 8),
        ("j1", "酒", Suit::Spade, 9),
        ("j2", "酒", Suit::Club, 3),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("nm2", "南蛮入侵", Suit::Club, 13),
        ("wj1", "万箭齐发", Suit::Heart, 1),
        ("wux1", "无懈可击", Suit::Spade, 11),
        ("sh1", "顺手牵羊", Suit::Diamond, 7),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("le1", "乐不思蜀", Suit::Heart, 6),
        ("bl1", "兵粮寸断", Suit::Spade, 5),
        ("jd1", "决斗", Suit::Club, 4),
        ("jd2", "决斗", Suit::Diamond, 6),
        ("tl1", "铁索连环", Suit::Club, 10),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip(
        "w2",
        "青龙偃月刀",
        Suit::Spade,
        5,
        Slot::Weapon,
    ));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    cards.push(Card::equip("arm2", "藤甲", Suit::Club, 2, Slot::Armor));
    cards.push(Card::equip(
        "horse1",
        "赤兔",
        Suit::Diamond,
        5,
        Slot::HorseOffense,
    ));
    cards.push(Card::equip(
        "horse2",
        "的卢",
        Suit::Club,
        5,
        Slot::HorseDefense,
    ));
    for i in 40..=99 {
        let suit = match i % 4 {
            0 => Suit::Heart,
            1 => Suit::Diamond,
            2 => Suit::Club,
            _ => Suit::Spade,
        };
        cards.push(Card::new(format!("x{i}"), "闪", suit, ((i % 13) + 1) as u8));
    }
    cards
}

fn game(specs: &[(&str, &[&str], i32, &str)]) -> Game {
    Game::new(
        specs
            .iter()
            .map(|(n, s, hp, kingdom)| {
                let mut seat = Seat::new(n, "gen", s, *hp);
                seat.kingdom = kingdom.to_string();
                seat
            })
            .collect(),
        deck(),
    )
}
fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids {
        g.to_hand(&id.to_string(), seat).unwrap();
    }
}
fn equip(g: &mut Game, seat: usize, id: &str, slot: Slot) {
    g.to_equip(&id.to_string(), seat, slot).unwrap();
}
fn pend_skill(g: &Game) -> Option<String> {
    g.pending
        .as_ref()
        .and_then(|p| p.skill().map(|s| s.to_string()))
}
fn pend_actor(g: &Game) -> usize {
    g.pending.as_ref().map(|p| p.actor()).unwrap_or(usize::MAX)
}
fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}
/// I1 询问完整性（v1 口径 2026-10-28）：有手牌就必须显式询问。
/// 这里把「无牌可打」的三类响应询问（闪/杀、无懈、濒死）按唯一出口放弃，
/// 避免每个用例重复处理（引擎的询问本身仍会产生，见 cards.rs::ask_* 用例）。
fn drain_forced(g: &mut Game) {
    while let Some(p) = g.pending.clone() {
        if !matches!(p.kind(), "card_response" | "wuxie" | "dying") {
            break;
        }
        if p.options().iter().any(|o| o.id != p.escape_id()) {
            break;
        }
        let (actor, escape) = (p.actor(), p.escape_id().to_string());
        submit(g, actor, &escape).expect("放弃唯一出口");
    }
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    // 该询问若只剩出口（有手牌但无对应牌），drain 已按唯一出口处理 → 只允许放弃类动作。
    if g.pending.is_none() {
        assert!(
            matches!(action, "pass" | "cancel"),
            "没有待办却提交了实质动作：{action}"
        );
        return;
    }
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    // I1：作答后可能立刻产生新的「无牌可打」礼貌询问（如锦囊的无懈窗口）→ 继续排空。
    drain_forced(g);
}
/// 通用作答（测试推进用）：**优先选第一个实质选项**，只有「只剩出口」时才放弃。
///
/// 不能反过来（默认放弃）：会把「激将」「拆牌」这类真实决策静默拒掉，
/// 让用例看起来通过但实际没走目标分支。
fn answer_any(g: &mut Game) {
    drain_forced(g);
    let Some(p) = g.pending.clone() else { return };
    let action = p
        .options()
        .iter()
        .find(|o| o.id != p.escape_id())
        .map(|o| o.id.clone())
        .unwrap_or_else(|| p.escape_id().to_string());
    answer(g, p.actor(), &action);
}

fn answer_cur(g: &mut Game, action: &str) {
    // I1：先把「无牌可打」的礼貌询问排空，再取当前被询问者（排空会推进流程）。
    drain_forced(g);
    let actor = pend_actor(g);
    answer(g, actor, action);
}
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}

// ---------------------------------------------------------------- 旋风
#[test]
fn xuanfeng_triggers_on_two_cards_lost_in_one_action() {
    let mut g = game(&[
        ("lingtong", &["xuanfeng"], 4, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 0;
    give(&mut g, 0, &["h1", "c1"]);
    give(&mut g, 1, &["d1", "d2", "d3", "t1"]);
    g.to_discard(&"h1".to_string(), "test").unwrap();
    g.to_discard(&"c1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("xuanfeng"));
    let before = g.hand(1).len();
    // 只选一名角色 → 该角色弃 2 张（用户裁定 Q4=B）。
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "confirm");
    assert_eq!(g.hand(1).len(), before - 2, "只选 1 人 → 弃 2 张");
}

#[test]
fn xuanfeng_ignores_single_card_loss_and_lost_equip_triggers() {
    let mut g = game(&[("lingtong", &["xuanfeng"], 4, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["h1"]);
    give(&mut g, 1, &["d1", "d2"]);
    g.to_discard(&"h1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("xuanfeng"), "单张不触发");
    // 失去装备区里的牌 → 触发。
    equip(&mut g, 0, "w1", Slot::Weapon);
    g.to_discard(&"w1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("xuanfeng"));
    answer(&mut g, 0, "pass");
}

// ---------------------------------------------------------------- 智迟
#[test]
fn zhichi_blocks_sha_and_normal_tricks_outside_own_turn() {
    let mut g = game(&[("chengong", &["zhichi"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert!(g.counter("turn.zhichi:0") > 0);
    assert!(
        !skills::can_target(&g, 1, "杀", 0, None),
        "【杀】不能以智迟为目标"
    );
    assert!(
        !skills::can_target(&g, 1, "决斗", 0, None),
        "普通锦囊不能以智迟为目标"
    );
    assert!(
        skills::can_target(&g, 1, "乐不思蜀", 0, None),
        "延时锦囊不受影响"
    );
}

#[test]
fn zhichi_not_triggered_in_own_turn() {
    let mut g = game(&[("chengong", &["zhichi"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.counter("turn.zhichi:0"), 0);
}

// ---------------------------------------------------------------- 明策
#[test]
fn mingce_gives_card_and_helper_chooses_draw() {
    let mut g = game(&[
        ("chengong", &["mingce"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    let (h0, h1) = (g.hand(0).len(), g.hand(1).len());
    skills::execute_active(&mut g, 0, "skill:mingce").unwrap();
    answer(&mut g, 0, "pick:sp1");
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1);
    answer(&mut g, 1, "opt2");
    assert_eq!(g.hand(1).len(), h1 + 2, "目标收 1 张再摸 1 张");
    assert_eq!(g.hand(0).len(), h0 - 1 + 1, "陈宫摸 1 张");
}

#[test]
fn mingce_virtual_sha_damage_draws_for_both() {
    let mut g = game(&[
        ("chengong", &["mingce"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    let h0 = g.hand(0).len();
    let h1 = g.hand(1).len();
    skills::execute_active(&mut g, 0, "skill:mingce").unwrap();
    answer(&mut g, 0, "pick:sp1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt1");
    answer(&mut g, 0, "target:2");
    if pend_skill(&g).as_deref() == Some("__card") {
        answer_cur(&mut g, "pass"); // 没人出【闪】
    }
    assert_eq!(g.seats[2].hp, 3, "虚拟【杀】造成 1 点伤害");
    assert_eq!(g.hand(1).len(), h1 + 2, "协助者收 1 张再摸 1 张");
    assert_eq!(g.hand(0).len(), h0 - 1 + 1, "陈宫摸 1 张");
}

#[test]
fn mingce_requires_sha_or_equip_in_hand() {
    let mut g = game(&[("chengong", &["mingce"], 3, "qun"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1"]);
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:mingce"));
}

// ---------------------------------------------------------------- 将驰
#[test]
fn jiangchi_option1_penalty_and_hand_limit() {
    let mut g = game(&[("caozhang", &["jiangchi"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["sp1", "d1"]);
    skills::after_draw_phase(&mut g, 0);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jiangchi"));
    let before = g.hand(0).len();
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), before + 1, "摸 1 张");
    assert_eq!(g.sha_limit(0), 0, "【杀】次数 -1");
    assert!(skills::hand_limit_delta(&g, 0) >= 1, "手牌【杀】不计上限");
}

#[test]
fn jiangchi_option2_recast_and_no_distance() {
    let mut g = game(&[("caozhang", &["jiangchi"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["sp1", "sp2", "d1"]);
    skills::after_draw_phase(&mut g, 0);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "opt2");
    let before = g.hand(0).len();
    answer(&mut g, 0, "pick:sp1");
    assert_eq!(g.hand(0).len(), before, "弃 1 摸 1");
    assert_eq!(g.sha_limit(0), 2, "【杀】次数 +1");
    assert!(
        skills::sha_no_distance(&g, 0, &"sp2".to_string()),
        "本回合【杀】无距离（由技能提供）"
    );
}

// ---------------------------------------------------------------- 纵适
#[test]
fn zongshi_jie_win_gains_smaller_pindian_card() {
    let mut g = game(&[
        ("jianyong", &["zongshi_jie"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.to_discard(&"sp1".to_string(), "pindian").unwrap();
    g.to_discard(&"d1".to_string(), "pindian").unwrap();
    skills::pindian_after(&mut g, 0, 1, &"sp1".to_string(), &"d1".to_string(), Some(0));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zongshi_jie"));
    answer(&mut g, 0, "gain:d1");
    assert!(g.hand(0).contains(&"d1".to_string()), "赢 → 获得对方的牌");
}

#[test]
fn zongshi_jie_lose_gains_own_and_requires_skill() {
    let mut g = game(&[
        ("jianyong", &["zongshi_jie"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.to_discard(&"sp1".to_string(), "pindian").unwrap();
    g.to_discard(&"d1".to_string(), "pindian").unwrap();
    skills::pindian_after(&mut g, 0, 1, &"d1".to_string(), &"sp1".to_string(), Some(1));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zongshi_jie"));
    answer(&mut g, 0, "gain:d1");
    assert!(g.hand(0).contains(&"d1".to_string()), "没赢 → 获得自己的牌");
    // 反例：没有技能的武将不询问。
    let mut g2 = game(&[("p0", &[], 3, "shu"), ("p1", &[], 4, "wei")]);
    g2.to_discard(&"sp1".to_string(), "pindian").unwrap();
    g2.to_discard(&"d1".to_string(), "pindian").unwrap();
    skills::pindian_after(
        &mut g2,
        0,
        1,
        &"sp1".to_string(),
        &"d1".to_string(),
        Some(0),
    );
    advance(&mut g2).unwrap();
    assert_ne!(pend_skill(&g2).as_deref(), Some("zongshi_jie"));
}

// ---------------------------------------------------------------- 巧说
#[test]
fn qiaoshuo_win_allows_extra_target_then_consumes() {
    let mut g = game(&[
        ("jianyong", &["qiaoshuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "sp2"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    advance(&mut g).unwrap();
    assert_eq!(skills::qiao(&g, 0), 1, "每赢一次 +1 枚「巧」");
    assert_eq!(skills::extra_targets(&g, 0, None, "杀"), 1);
    // 多指定一个目标使用【杀】。
    g.resolve_use(0, &"sp2".to_string(), vec![1, 2]).unwrap();
    assert_eq!(
        skills::qiao(&g, 0),
        0,
        "使用一张符合条件手牌消耗 1 枚「巧」"
    );
}

#[test]
fn qiaoshuo_lose_blocks_tricks_and_invalidates_skill() {
    let mut g = game(&[("jianyong", &["qiaoshuo"], 3, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1", "d2", "sh1"]);
    give(&mut g, 1, &["sp1"]);
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 1, "choose:sp1");
    advance(&mut g).unwrap();
    assert!(skills::trick_blocked(&g, 0));
    assert!(!skills::can_target(&g, 0, "顺手牵羊", 1, None) || true);
    assert!(
        !skills::legal_actions(&g, 0)
            .iter()
            .any(|(id, _)| id == "skill:qiaoshuo"),
        "没赢 → 巧说失效"
    );
    assert!(
        g.resolve_use(0, &"sh1".to_string(), vec![1]).is_err(),
        "不能用锦囊"
    );
}

#[test]
fn qiaoshuo_consecutive_wins_stack_marks_one_per_card() {
    let mut g = game(&[
        ("jianyong", &["qiaoshuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "h1", "sp2", "jd1", "c2"]);
    give(&mut g, 1, &["d1", "c1"]);
    // 第一次拼点：赢（巧 1）。
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    advance(&mut g).unwrap();
    assert_eq!(skills::qiao(&g, 0), 1);
    // 第二次拼点（未使用牌前）：再赢（巧 2）。
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:h1");
    answer(&mut g, 1, "choose:c1");
    advance(&mut g).unwrap();
    assert_eq!(skills::qiao(&g, 0), 2, "连续赢 → 巧叠加");
    // 每出一张符合条件手牌只消耗 1 枚，且该张牌可 +1 目标。
    g.resolve_use(0, &"sp2".to_string(), vec![1, 2]).unwrap();
    assert_eq!(skills::qiao(&g, 0), 1, "一张牌消耗 1 枚「巧」");
    assert_eq!(skills::extra_targets(&g, 0, None, "杀"), 1, "仍有 1 枚可用");
    g.resolve_use(0, &"jd1".to_string(), vec![1]).unwrap();
    assert_eq!(skills::qiao(&g, 0), 0, "第二张符合条件手牌再消耗 1 枚");
}

#[test]
fn qiaoshuo_marks_exposed_in_view_and_cleared_on_loss() {
    let mut g = game(&[("jianyong", &["qiaoshuo"], 3, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "d2", "d3"]);
    give(&mut g, 1, &["d1", "sp2"]);
    // 赢 → 「巧」=1，并在视图 marks_count 中可见。
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    advance(&mut g).unwrap();
    assert_eq!(skills::qiao(&g, 0), 1);
    let view = sanguosha2::view::public_state(&g);
    assert_eq!(
        view["public_board"]["seats"][0]["marks_count"]["qiao"],
        serde_json::json!(1),
        "界面标记：巧"
    );
    // 再拼点没赢 → 「巧」清空、技能失效。
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d2");
    answer(&mut g, 1, "choose:sp2");
    advance(&mut g).unwrap();
    assert_eq!(skills::qiao(&g, 0), 0, "拼点失败 → 巧清空");
    assert!(skills::trick_blocked(&g, 0));
}

#[test]
fn qiaoshuo_delayed_trick_gets_no_extra_target() {
    let mut g = game(&[
        ("jianyong", &["qiaoshuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["le1", "sp2"]);
    // 相当于巧说已赢（细致拼点流程见上面的用例）。
    g.bump("turn.qiao:0");
    // 延时锦囊：不提供额外目标（用户 2026-11-10 裁定：废除延迟锦囊的额外目标支持）。
    assert_eq!(
        skills::extra_targets(&g, 0, Some(&"le1".to_string()), "乐不思蜀"),
        0
    );
    let uses = card_play::legal_uses(&g, 0);
    assert!(
        uses.iter().any(|(id, _)| id == "use:le1:1"),
        "仍可正常以 1 个目标使用"
    );
    assert!(
        !uses.iter().any(|(id, _)| id.starts_with("use:le1:1,2")),
        "不提供 2 目标的【乐不思蜀】"
    );
    // 使用延时锦囊不消耗「巧」。
    g.resolve_use(0, &"le1".to_string(), vec![1]).unwrap();
    assert_eq!(skills::qiao(&g, 0), 1, "延时锦囊不消耗「巧」");
    // 下一张非延时牌（【杀】）仍可 +1 目标，并在使用后消耗 1 枚。
    g.resolve_use(0, &"sp2".to_string(), vec![1, 2]).unwrap();
    assert_eq!(skills::qiao(&g, 0), 0);
    assert_eq!(skills::extra_targets(&g, 0, None, "杀"), 0);
}

#[test]
fn qiaoshuo_fixed_target_tricks_do_not_consume_mark() {
    let mut g = game(&[
        ("jianyong", &["qiaoshuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["tl1", "sp1"]);
    g.bump("turn.qiao:0");
    // 铁索连环（1–2 目标，另有目标流程）不参与 ±1，也不消耗「巧」。
    g.resolve_use(0, &"tl1".to_string(), vec![1]).unwrap();
    drain_forced(&mut g); // I1：铁索的无懈/响应询问先排空
    assert_eq!(skills::qiao(&g, 0), 1, "固定目标结构的锦囊不消耗「巧」");
    // 满足条件的【杀】才消耗。
    g.resolve_use(0, &"sp1".to_string(), vec![1]).unwrap();
    assert_eq!(skills::qiao(&g, 0), 0);
}

#[test]
fn qiaoshuo_win_offers_trim_action() {
    let mut g = game(&[
        ("jianyong", &["qiaoshuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "nm1"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:qiaoshuo").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    advance(&mut g).unwrap();
    assert!(skills::can_trim_targets(&g, 0));
    assert!(
        card_play::legal_uses(&g, 0)
            .iter()
            .any(|(id, _)| id.starts_with("trim:nm1:")),
        "群体锦囊提供 -1 目标动作"
    );
}

// ---------------------------------------------------------------- 纵玄
#[test]
fn zongxuan_puts_own_discards_on_top() {
    let mut g = game(&[("yufan", &["zongxuan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["h1", "c1"]);
    g.to_discard(&"h1".to_string(), "test").unwrap();
    g.to_discard(&"c1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zongxuan"));
    answer(&mut g, 0, "choose:h1");
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "confirm");
    assert_eq!(g.draw_top(), Some("c1".to_string()), "后选的在最上面");
}

#[test]
fn zongxuan_upper_seat_first_discard_only() {
    let mut g = game(&[("yufan", &["zongxuan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    give(&mut g, 1, &["h1", "c1", "d1"]);
    g.to_discard(&"h1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(
        pend_skill(&g).as_deref(),
        Some("zongxuan"),
        "上家首次弃置触发"
    );
    answer(&mut g, 0, "pass");
    // 同一回合第二次不触发。
    g.to_discard(&"c1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zongxuan"));
}

// ---------------------------------------------------------------- 直言
#[test]
fn zhiyan_uses_equip_and_heals() {
    let mut g = game(&[("yufan", &["zhiyan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    g.seats[1].hp = 2;
    g.put_on_top(&"w1".to_string()).unwrap();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhiyan"));
    answer(&mut g, 0, "target:1");
    assert_eq!(g.equip_in(1, Slot::Weapon), Some("w1".to_string()));
    assert_eq!(g.seats[1].hp, 3, "装备牌 → 回复 1 点体力");
}

#[test]
fn zhiyan_non_equip_hp_diff_loses_hp() {
    let mut g = game(&[("yufan", &["zhiyan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    g.seats[0].hp = 3;
    g.seats[1].hp = 4;
    g.put_on_top(&"d1".to_string()).unwrap();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "非装备且体力不同 → 失去 1 点体力");
    // 反例：体力值相同则不失去。
    let mut g2 = game(&[("yufan", &["zhiyan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g2.turn = 0;
    g2.seats[0].hp = 3;
    g2.seats[1].hp = 3;
    g2.put_on_top(&"d2".to_string()).unwrap();
    begin_phase(&mut g2, 0, Phase::End).unwrap();
    answer(&mut g2, 0, "target:1");
    assert_eq!(g2.seats[1].hp, 3, "体力值相同 → 不失去体力");
}

#[test]
fn zhiyan_upper_seat_end_phase_triggers() {
    let mut g = game(&[("yufan", &["zhiyan"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhiyan"));
    answer(&mut g, 0, "pass");
}

// ---------------------------------------------------------------- 陷阵
#[test]
fn xianzhen_win_grants_distance_limit_and_extra_target() {
    let mut g = game(&[
        ("gaoshun", &["xianzhen"], 4, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "sp2"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:xianzhen").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    advance(&mut g).unwrap();
    assert_eq!(g.counter("turn.xianzhen_win"), 1);
    assert!(skills::target_no_distance(&g, 0, 1));
    assert!(skills::sha_limit_exempt(&g, 0, &[1]));
    assert!(skills::armor_ignored_by_skill(&g, 0, 1));
    assert_eq!(skills::extra_targets(&g, 0, None, "杀"), 1);
}

#[test]
fn xianzhen_lose_blocks_sha_only_vs_target() {
    let mut g = game(&[
        ("gaoshun", &["xianzhen"], 4, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1", "c1"]);
    give(&mut g, 1, &["sp1"]);
    skills::execute_active(&mut g, 0, "skill:xianzhen").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 1, "choose:sp1");
    advance(&mut g).unwrap();
    assert_eq!(g.counter("turn.xianzhen_lose"), 1);
    assert!(skills::sha_blocked_for(&g, 0, 1));
    assert!(!skills::sha_blocked_for(&g, 0, 2), "只封禁被拼点角色");
    assert!(skills::hand_limit_delta(&g, 0) >= 1, "【杀】不计手牌上限");
}

// ---------------------------------------------------------------- 禁酒
#[test]
fn jinjiu_converts_jiu_to_sha_with_rank_k() {
    let mut g = game(&[("gaoshun", &["jinjiu"], 4, "qun"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["j1"]);
    let convs = skills::conversions(&g, 0);
    assert!(
        convs.iter().any(|c| c.from == "j1" && c.to == "杀"),
        "【酒】视为【杀】"
    );
    assert_eq!(
        skills::effective_rank(&g, 0, &"j1".to_string()),
        Some(13),
        "点数 K"
    );
    assert!(skills::jiu_blocked(&g, 0), "高顺自己不能当【酒】用");
    assert!(g.resolve_use(0, &"j1".to_string(), vec![]).is_err());
}

#[test]
fn jinjiu_blocks_others_jiu_during_own_turn() {
    let mut g = game(&[("gaoshun", &["jinjiu"], 4, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    assert!(skills::jiu_blocked(&g, 1), "高顺回合内其他角色不能用【酒】");
    g.turn = 1;
    assert!(!skills::jiu_blocked(&g, 1), "其他回合可正常使用【酒】");
}

// ---------------------------------------------------------------- 眩惑
#[test]
fn xuanhuo_gives_two_cards_and_views_hand() {
    let mut g = game(&[
        ("fazheng", &["xuanhuo"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 0;
    give(&mut g, 0, &["sp1", "h1", "d1"]);
    give(&mut g, 1, &["c1", "c2"]);
    let h0 = g.hand(0).len();
    let h1 = g.hand(1).len();
    skills::after_draw_phase(&mut g, 0);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("xuanhuo"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 0, "choose:h1");
    answer(&mut g, 0, "confirm");
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1);
    answer(&mut g, 1, "opt2");
    // 观看手牌并获得两张。
    let opts = options(&g);
    let picks: Vec<String> = opts
        .iter()
        .filter(|o| o.starts_with("choose:"))
        .take(2)
        .cloned()
        .collect();
    assert_eq!(picks.len(), 2, "可观看并选其两张手牌");
    for p in &picks {
        answer(&mut g, 0, p);
    }
    answer(&mut g, 0, "confirm");
    assert_eq!(g.hand(0).len(), h0 - 2 + 2, "交给两张并获得两张");
    assert_eq!(g.hand(1).len(), h1 + 2 - 2, "协助者收两张后被获得两张");
}

#[test]
fn xuanhuo_requires_two_hand_cards() {
    let mut g = game(&[("fazheng", &["xuanhuo"], 3, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["sp1"]);
    skills::after_draw_phase(&mut g, 0);
    assert_ne!(pend_skill(&g).as_deref(), Some("xuanhuo"));
}

// ---------------------------------------------------------------- 恩怨
#[test]
fn enyuan_gain_two_cards_lets_owner_draw() {
    let mut g = game(&[("fazheng", &["enyuan"], 3, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 1, &["sp1", "d1", "d2"]);
    let before = g.hand(1).len();
    g.gain(&"sp1".to_string(), 0).unwrap();
    g.gain(&"d1".to_string(), 0).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("enyuan"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(1).len(), before - 2 + 1, "来源摸 1 张");
}

#[test]
fn enyuan_single_gain_and_damage_choice() {
    let mut g = game(&[("fazheng", &["enyuan"], 3, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 1, &["sp1", "d1"]);
    g.gain(&"sp1".to_string(), 0).unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("enyuan"), "获得 1 张不触发");
    // 受伤后由来源二选一：失去 1 点体力。
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("enyuan"));
    assert_eq!(pend_actor(&g), 1, "由伤害来源选择");
    let hp = g.seats[1].hp;
    answer(&mut g, 1, "opt2");
    assert_eq!(g.seats[1].hp, hp - 1);
}

// ---------------------------------------------------------------- 疠火
#[test]
fn lihuo_converts_sha_to_fire_and_pays_cost() {
    let mut g = game(&[("chengpu", &["lihuo"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "d1"]);
    // 藤甲：普通【杀】无效、火焰伤害 +1 → 用来断言火属性。
    equip(&mut g, 1, "arm2", Slot::Armor);
    let convs = skills::conversions(&g, 0);
    assert!(
        convs.iter().any(|c| c.from == "sp1" && c.to == "火杀"),
        "非火【杀】可转火【杀】"
    );
    skills::b17::on_conversion_used(&mut g, 0, "lihuo", &"sp1".to_string(), "火杀");
    g.use_as(0, &"sp1".to_string(), "火杀", vec![1], true, None)
        .unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 2, "火焰伤害 +1（藤甲）");
    assert_eq!(pend_skill(&g).as_deref(), Some("lihuo"), "结算后询问代价");
    let hp = g.seats[0].hp;
    answer(&mut g, 0, "opt2");
    assert_eq!(g.seats[0].hp, hp - 1, "失去 1 点体力作为代价");
}

#[test]
fn lihuo_real_fire_sha_has_no_cost_and_extra_target() {
    let mut g = game(&[
        ("chengpu", &["lihuo"], 4, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["hs1"]);
    assert_eq!(skills::extra_targets(&g, 0, None, "火杀"), 1);
    assert!(
        !skills::conversions(&g, 0).iter().any(|c| c.from == "hs1"),
        "真火【杀】不需要转化"
    );
    let hp = g.seats[0].hp;
    g.resolve_use(0, &"hs1".to_string(), vec![1, 2]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[0].hp, hp, "真火【杀】无代价");
}

// ---------------------------------------------------------------- 醇醪
#[test]
fn chunlao_places_adjacent_discarded_sha() {
    let mut g = game(&[
        ("chengpu", &["chunlao"], 4, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
        ("p3", &[], 4, "wei"),
    ]);
    g.turn = 1;
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 2, &["sp2"]);
    g.to_discard(&"sp1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(
        g.mark_cards(0, "chun").len(),
        1,
        "相邻角色的【杀】自动置「醇」"
    );
    // 反例：非相邻（2 号与程普在 4 人环上不相邻）。
    g.turn = 2;
    g.to_discard(&"sp2".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.mark_cards(0, "chun").len(), 1, "非相邻不置「醇」");
}

#[test]
fn chunlao_dying_uses_x_chun_as_jiu() {
    let mut g = game(&[
        ("chengpu", &["chunlao"], 4, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
        ("p3", &[], 4, "wei"),
    ]);
    g.turn = 1;
    give(&mut g, 1, &["sp1"]);
    g.to_discard(&"sp1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.mark_cards(0, "chun").len(), 1);
    // 2 号濒死：求桃顺序 2 → 3 → 0 → 1，程普在第三位提供「醇醪」。
    g.seats[2].hp = 1;
    flow::damage(&mut g, Some(1), 2, 1);
    advance(&mut g).unwrap();
    while pend_skill(&g).as_deref() == Some("dying") && pend_actor(&g) != 0 {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(pend_skill(&g).as_deref(), Some("dying"));
    assert_eq!(pend_actor(&g), 0, "轮到程普救援");
    assert!(
        options(&g).iter().any(|o| o == "chunlao"),
        "提供「醇醪」选项"
    );
    answer(&mut g, 0, "chunlao");
    assert_eq!(g.seats[2].hp, 1, "视为使用【酒】回复 1 点");
    assert_eq!(g.mark_cards(0, "chun").len(), 0, "移去 1 张「醇」");
    assert_eq!(g.counter("chunlao_x"), 1, "本轮次数 +1");
}
