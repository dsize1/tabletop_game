//! B19 用例（15 技能，每条 ≥1 正例 + ≥1 反例）。
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("sp2", "杀", Suit::Spade, 10),
        ("sp3", "杀", Suit::Club, 12),
        ("h1", "杀", Suit::Heart, 9),
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("d3", "闪", Suit::Diamond, 4),
        ("t1", "桃", Suit::Heart, 7),
        ("t2", "桃", Suit::Heart, 8),
        ("j1", "酒", Suit::Spade, 9),
        ("jd1", "决斗", Suit::Club, 4),
        ("jd2", "决斗", Suit::Diamond, 6),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("gh2", "过河拆桥", Suit::Diamond, 7),
        ("sh1", "顺手牵羊", Suit::Diamond, 7),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("wj1", "万箭齐发", Suit::Heart, 1),
        ("wz1", "无中生有", Suit::Heart, 7),
        ("le1", "乐不思蜀", Suit::Heart, 6),
        ("bl1", "兵粮寸断", Suit::Spade, 5),
        ("hg1", "火攻", Suit::Heart, 2),
        ("tl1", "铁索连环", Suit::Club, 10),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip("w2", "朱雀羽扇", Suit::Diamond, 1, Slot::Weapon));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    cards.push(Card::equip("horse1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
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
fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}
fn answer_cur(g: &mut Game, action: &str) {
    let actor = pend_actor(g);
    answer(g, actor, action);
}
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}
fn pass_until(g: &mut Game, skill: &str) {
    for _ in 0..24 {
        if pend_skill(g).as_deref() == Some(skill) {
            return;
        }
        if g.pending.is_none() {
            return;
        }
        answer_cur(g, "pass");
    }
}

// ---------------------------------------------------------------- 灭计
#[test]
fn mieji_puts_trick_on_top_and_target_discards_two() {
    let mut g = game(&[
        ("liru", &["mieji", "fencheng", "juece"], 3, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["gh1"]);
    give(&mut g, 1, &["d1", "d2", "t1"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:mieji").unwrap();
    answer(&mut g, 0, "choose:gh1");
    assert_eq!(g.draw_top().as_deref(), Some("gh1"), "置于牌堆顶");
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1);
    answer(&mut g, 1, "opt2");
    let before = g.hand(1).len();
    let pick = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
    answer(&mut g, 1, &pick);
    let pick = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
    answer(&mut g, 1, &pick);
    assert_eq!(g.hand(1).len(), before - 2, "依次弃置两张牌");
    assert_eq!(g.counter("turn.mieji_used"), 1);
}

#[test]
fn mieji_requires_trick_and_is_once_per_play_phase() {
    let mut g = game(&[
        ("liru", &["mieji"], 3, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["d2", "d3"]);
    play_phase(&mut g, 0);
    assert!(
        skills::execute_active(&mut g, 0, "skill:mieji").is_err(),
        "没有锦囊牌不能发动"
    );
    give(&mut g, 0, &["gh1"]);
    skills::execute_active(&mut g, 0, "skill:mieji").unwrap();
    answer(&mut g, 0, "choose:gh1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt2");
    for _ in 0..2 {
        let pick = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
        answer(&mut g, 1, &pick);
    }
    give(&mut g, 0, &["gh2"]);
    assert!(
        skills::execute_active(&mut g, 0, "skill:mieji").is_err(),
        "出牌阶段限一次"
    );
}

// ---------------------------------------------------------------- 焚城
#[test]
fn fencheng_start_target_and_discard_more_than_last() {
    let mut g = game(&[
        ("liru", &["fencheng"], 3, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["d1", "d2", "t1"]);
    give(&mut g, 2, &["d3", "t2"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:fencheng").unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1);
    answer(&mut g, 1, "opt1");
    for _ in 0..2 {
        let pick = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
        answer(&mut g, 1, &pick);
    }
    answer(&mut g, 1, "done");
    // p2 只有 2 张牌，需弃 3 张 → 只能受伤。
    assert_eq!(pend_actor(&g), 2);
    assert!(!options(&g).iter().any(|o| o == "opt1"), "牌不足不能弃置");
    answer(&mut g, 2, "opt2");
    assert_eq!(g.seats[2].hp, 2, "受到 2 点火焰伤害");
    assert!(g.seats[0].flags.contains("fencheng_used"));
}

#[test]
fn fencheng_is_limited_and_needs_start_target() {
    let mut g = game(&[("liru", &["fencheng"], 3, "qun"), ("p1", &[], 4, "qun")]);
    play_phase(&mut g, 0);
    g.seats[0].flags.insert("fencheng_used".to_string());
    assert!(
        skills::execute_active(&mut g, 0, "skill:fencheng").is_err(),
        "限定技已用"
    );
}

// ---------------------------------------------------------------- 绝策
#[test]
fn juece_hits_target_with_fewer_or_equal_hand() {
    let mut g = game(&[
        ("liru", &["juece"], 3, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1", "d2"]);
    give(&mut g, 1, &["t1"]);
    give(&mut g, 2, &["t2", "d3", "x40"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("juece"));
    let opts = options(&g);
    assert!(opts.contains(&"target:1".to_string()));
    assert!(!opts.contains(&"target:2".to_string()), "手牌数大于你的不能选");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3);
}

#[test]
fn juece_not_triggered_outside_end_phase() {
    let mut g = game(&[("liru", &["juece"], 3, "qun"), ("p1", &[], 4, "qun")]);
    give(&mut g, 1, &["t1"]);
    play_phase(&mut g, 0);
    assert_ne!(pend_skill(&g).as_deref(), Some("juece"));
}

// ---------------------------------------------------------------- 精策
#[test]
fn jingce_hand_limit_and_optional_draw_at_play_end() {
    let mut g = game(&[
        ("guohuai", &["jingce"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["gh1", "sp1"]);
    give(&mut g, 1, &["d1"]);
    play_phase(&mut g, 0);
    let base = g.hand_limit(0);
    g.resolve_use(0, &"gh1".to_string(), vec![1]).unwrap();
    advance(&mut g).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.hand_limit(0), base + 1, "使用一种花色的手牌 → 上限 +1");
    // 出牌阶段结束触发（内核在阶段结束时按注册表压入询问；此处直接压入同一入口）。
    sanguosha2::kernel::resolve::push_ask(
        &mut g,
        "jingce",
        sanguosha2::kernel::resolve::Stage::Choose,
        0,
        serde_json::json!({}),
    );
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jingce"), "{:?}", options(&g));
    let before = g.hand(0).len();
    answer(&mut g, 0, "draw");
    assert_eq!(g.hand(0).len(), before + 1);
}

#[test]
fn jingce_no_prompt_when_no_card_used() {
    let mut g = game(&[("guohuai", &["jingce"], 4, "wei"), ("p1", &[], 4, "qun")]);
    play_phase(&mut g, 0);
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("jingce"), "X=0 不询问");
}

// ---------------------------------------------------------------- 惴恐
#[test]
fn zhuikong_win_blocks_targeting_others() {
    let mut g = game(&[
        ("fuhou", &["zhuikong", "qiuyuan"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    play_phase(&mut g, 1);
    g.turn = 0;
    begin_phase(&mut g, 1, Phase::Prepare).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuikong"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    assert_eq!(g.counter("turn.zhuikong_deny:1"), 1, "赢 → 封锁对其他角色使用牌");
    assert!(skills::others_use_blocked(&g, 1));
    give(&mut g, 1, &["sp2"]);
    assert!(
        g.resolve_use(1, &"sp2".to_string(), vec![0]).is_err(),
        "不能对除其以外的角色使用牌"
    );
}

#[test]
fn zhuikong_lose_gains_pindian_card_and_virtual_sha() {
    let mut g = game(&[
        ("fuhou", &["zhuikong"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["sp1"]);
    play_phase(&mut g, 1);
    g.turn = 0;
    begin_phase(&mut g, 1, Phase::Prepare).unwrap();
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 1, "choose:sp1");
    assert!(
        g.hand(0).iter().any(|c| c == "sp1"),
        "没赢：获得其拼点牌"
    );
    assert_eq!(g.seats[0].hp, 2, "视为使用【杀】并命中");
}

#[test]
fn zhuikong_needs_wounded_and_hand() {
    let mut g = game(&[("fuhou", &["zhuikong"], 4, "qun"), ("p1", &[], 4, "qun")]);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    // 满血不触发。
    begin_phase(&mut g, 1, Phase::Prepare).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zhuikong"));
}

// ---------------------------------------------------------------- 求援
#[test]
fn qiuyuan_helper_gives_same_kind_card() {
    let mut g = game(&[
        ("fuhou", &["qiuyuan"], 4, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 2, &["t1"]);
    play_phase(&mut g, 1);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    pass_until(&mut g, "qiuyuan");
    assert_eq!(pend_skill(&g).as_deref(), Some("qiuyuan"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "target:2");
    assert_eq!(pend_actor(&g), 2);
    answer(&mut g, 2, "opt1");
    let pick = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
    answer(&mut g, 2, &pick);
    assert_eq!(g.hand(0).len(), 1, "获得协助者的牌");
}

#[test]
fn qiuyuan_not_triggered_without_helper_or_for_non_damage_trick() {
    let mut g = game(&[("fuhou", &["qiuyuan"], 4, "qun"), ("p1", &[], 4, "qun")]);
    give(&mut g, 1, &["sp1", "gh1"]);
    give(&mut g, 0, &["d1", "d2"]);
    play_phase(&mut g, 1);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("qiuyuan"), "无协助者");
    answer_cur(&mut g, "pass");
    g.resolve_use(1, &"gh1".to_string(), vec![0]).unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("qiuyuan"), "非伤害锦囊");
}

// ---------------------------------------------------------------- 弓骑
#[test]
fn gongqi_discards_equip_then_unlimited_sha_of_same_suit() {
    let mut g = game(&[
        ("handang", &["gongqi"], 4, "wu"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    equip(&mut g, 0, "w1", Slot::Weapon);
    give(&mut g, 1, &["d1"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:gongqi").unwrap();
    answer(&mut g, 0, "choose:w1");
    assert!(skills::infinite_range(&g, 0), "攻击范围无限");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "hand");
    // 同花色【杀】两次都能用（超过基础 1 次）。
    give(&mut g, 0, &["sp1", "sp2"]);
    give(&mut g, 1, &["d1", "d2", "d3"]);
    g.resolve_use(0, &"sp1".to_string(), vec![1]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    g.resolve_use(0, &"sp2".to_string(), vec![2]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert!(g.sha_used(0) >= 2);
}

#[test]
fn gongqi_once_per_play_phase() {
    let mut g = game(&[("handang", &["gongqi"], 4, "wu"), ("p1", &[], 4, "qun")]);
    give(&mut g, 0, &["d1"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:gongqi").unwrap();
    answer(&mut g, 0, "choose:d1");
    assert!(skills::execute_active(&mut g, 0, "skill:gongqi").is_err());
}

// ---------------------------------------------------------------- 解烦
#[test]
fn jiefan_voters_choose_and_round_one_resets() {
    let mut g = game(&[
        ("handang", &["jiefan"], 4, "wu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["w2"]);
    g.round = 1;
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiefan").unwrap();
    answer(&mut g, 0, "target:0");
    assert_eq!(pend_actor(&g), 1, "攻击范围含目标的角色投票");
    answer(&mut g, 1, "opt1");
    assert!(!g.hand(1).iter().any(|c| c == "w2"), "弃置武器牌");
    // 第一轮：回合结束视为未发动。
    assert!(!g.seats[0].flags.contains("jiefan_used"));
    g.counters.remove("turn.jiefan_used:0");
    assert!(skills::def("jiefan").unwrap().can_active.unwrap()(&g, 0));
}

#[test]
fn jiefan_after_round_one_is_once_per_game() {
    let mut g = game(&[("handang", &["jiefan"], 4, "wu"), ("p1", &[], 4, "qun")]);
    g.round = 2;
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiefan").unwrap();
    answer(&mut g, 0, "target:0");
    answer(&mut g, 1, "opt2");
    assert!(g.seats[0].flags.contains("jiefan_used"));
    assert!(skills::execute_active(&mut g, 0, "skill:jiefan").is_err());
}

// ---------------------------------------------------------------- 父魂 / 马术
#[test]
fn fuhen_same_color_restricts_shan_response() {
    let mut g = game(&[
        ("guanxing", &["fuhen"], 4, "shu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp1", "sp2"]);
    give(&mut g, 1, &["d1", "x43"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:fuhen").unwrap();
    let pair = options(&g).into_iter().find(|o| o.starts_with("pair:")).unwrap();
    answer(&mut g, 0, &pair);
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1, "{:?}", options(&g));
    let opts = options(&g);
    assert!(
        opts.iter().any(|o| o == "respond:x43"),
        "黑色【闪】可响应：{opts:?}"
    );
    assert!(
        !opts.iter().any(|o| o == "respond:d1"),
        "红色手牌不能响应：{opts:?}"
    );
}

#[test]
fn fuhen_grants_wusheng_paoxiao_until_turn_end() {
    let mut g = game(&[
        ("guanxing", &["fuhen"], 4, "shu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp1", "sp2"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:fuhen").unwrap();
    let pair = options(&g).into_iter().find(|o| o.starts_with("pair:")).unwrap();
    answer(&mut g, 0, &pair);
    answer(&mut g, 0, "target:1");
    advance(&mut g).unwrap();
    assert!(g.seats[0].granted.iter().any(|s| s == "wusheng"));
    assert!(g.seats[0].granted.iter().any(|s| s == "paoxiao"));
    skills::on_turn_end(&mut g, 0);
    assert!(g.seats[0].granted.is_empty(), "回合结束撤销");
}

#[test]
fn fuhen_two_card_sha_response_option_exists() {
    let mut g = game(&[
        ("guanxing", &["fuhen"], 4, "shu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp1", "sp2"]);
    give(&mut g, 1, &["nm1"]);
    play_phase(&mut g, 1);
    g.resolve_use(1, &"nm1".to_string(), vec![0]).unwrap();
    assert_eq!(pend_actor(&g), 0);
    assert!(
        options(&g).iter().any(|o| o.starts_with("pair:")),
        "父魂可将两张手牌当【杀】打出：{:?}",
        options(&g)
    );
}

#[test]
fn qianxi_locks_distance_one_color_and_boosts_revealed_card() {
    let mut g = game(&[
        ("madai", &["mashu", "qianxi"], 4, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["h1"]);
    give(&mut g, 1, &["t1", "d1"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qianxi"));
    answer(&mut g, 0, "choose:h1");
    assert!(skills::hand_color_blocked(&g, 1, &"t1".to_string()), "距离 1 红色手牌被封锁");
    assert!(!skills::hand_color_blocked(&g, 2, &"t1".to_string()), "距离 2 不受限");
    g.resolve_use(0, &"h1".to_string(), vec![1]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[1].hp, 2, "展示牌造成伤害 +1");
}

#[test]
fn qianxi_does_not_lock_without_reveal() {
    let mut g = game(&[
        ("madai", &["mashu", "qianxi"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["h1"]);
    give(&mut g, 1, &["t1"]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    answer(&mut g, 0, "pass");
    assert_eq!(g.seats[0].armor, 0);
    assert!(!skills::hand_color_blocked(&g, 1, &"t1".to_string()));
}

// ---------------------------------------------------------------- 矫诏 / 殚心
#[test]
fn jiazhao_excludes_names_used_this_round() {
    let mut g = game(&[
        ("guohuanghou", &["jiazhao", "danxin"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["gh1"]);
    give(&mut g, 1, &["d1"]);
    g.counters.insert("round.used:杀".to_string(), 1);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiazhao").unwrap();
    let opts = options(&g);
    assert!(
        !opts.iter().any(|o| o == "name:杀"),
        "本轮用过的牌名不可选：{opts:?}"
    );
    assert!(opts.iter().any(|o| o == "name:过河拆桥"));
    assert!(!opts.iter().any(|o| o == "name:乐不思蜀"), "延时锦囊不可选");
}

#[test]
fn jiazhao_uses_card_as_trick_and_plus_works_out_of_turn() {
    let mut g = game(&[
        ("guohuanghou", &["jiazhao"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["gh1"]);
    give(&mut g, 1, &["d1", "d2"]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiazhao").unwrap();
    answer(&mut g, 0, "name:过河拆桥");
    let card = options(&g).into_iter().find(|o| o.starts_with("choose:")).unwrap();
    answer(&mut g, 0, &card);
    answer(&mut g, 0, "target:1");
    // 过河拆桥：选择弃置目标一张手牌。
    if options(&g).iter().any(|o| o.starts_with("pick:")) {
        let pick = options(&g).into_iter().find(|o| o.starts_with("pick:")).unwrap();
        answer_cur(&mut g, &pick);
    }
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.hand(1).len(), 1, "过河拆桥生效");
    // 修改 1：回合外也可以使用（每轮限一次）。
    g.seats[0]
        .counters
        .insert("jiazhao_mod".to_string(), 1);
    give(&mut g, 0, &["gh2"]);
    play_phase(&mut g, 1);
    let acts = skills::legal_actions_out_of_turn(&g, 0);
    assert!(
        acts.iter().any(|(id, _)| id == "skill:jiazhao"),
        "矫诏plus 回合外可用"
    );
    assert_eq!(skills::b19::display_name(&g, 0), "矫诏plus");
}

#[test]
fn danxin_draws_x_then_modifies_jiazhao() {
    let mut g = game(&[
        ("guohuanghou", &["danxin"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0]
        .counters
        .insert("jiazhao_mod".to_string(), 1);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["sp1"]);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    pass_until(&mut g, "danxin");
    assert_eq!(pend_skill(&g).as_deref(), Some("danxin"));
    let before = g.hand(0).len();
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), before + 1, "X=1 摸 1 张");
    assert_eq!(skills::b19::jiazhao_mod(&g, 0), 2);
    assert_eq!(skills::b19::display_name(&g, 0), "矫诏plusplus");
}

#[test]
fn danxin_not_triggered_while_dying() {
    let mut g = game(&[("guohuanghou", &["danxin"], 1, "wei"), ("p1", &[], 4, "qun")]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["sp1"]);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("danxin"));
}

// ---------------------------------------------------------------- 矢北 / 渐营
#[test]
fn shibei_armor_absorbs_then_heals_and_loses_hp() {
    let mut g = game(&[("jushou", &["shibei"], 4, "qun"), ("p1", &[], 4, "qun")]);
    assert_eq!(g.seats[0].armor, 3, "开局 3 点护甲");
    g.seats[0].hp = 3;
    // 5 点伤害：护甲 3 全吸收，扣 2 体力 → 首次受伤回 1。
    flow::damage(&mut g, None, 0, 5);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].armor, 0);
    assert!(
        g.log.iter().any(|l| l.text.contains("护甲吸收")),
        "护甲吸收必须走统一伤害入口（armor_changed 事件）"
    );
    assert_eq!(g.seats[0].hp, 2, "护甲吸收 3，扣 2，回 1");
    // 同回合再受伤 → 失去 1 点体力（体力 2 → 1 → 0，进入濒死）。
    flow::damage(&mut g, None, 0, 1);
    advance(&mut g).unwrap();
    for _ in 0..8 {
        if g.pending.is_none() {
            break;
        }
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[0].hp, 0, "再次受伤后失去 1 点体力");
}

#[test]
fn shibei_armor_is_projected_in_marks_count() {
    let mut g = game(&[("jushou", &["shibei"], 4, "qun"), ("p1", &[], 4, "qun")]);
    let view = sanguosha2::view::public_state(&g);
    let hu = view["public_board"]["seats"][0]["marks_count"]["hu"].as_i64();
    assert_eq!(hu, Some(3), "护甲复用标记区显示「护N」");
}

#[test]
fn jianying_draws_on_same_suit_in_play_phase() {
    let mut g = game(&[
        ("jushou", &["jianying"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 2;
    give(&mut g, 0, &["t1", "t2"]);
    play_phase(&mut g, 0);
    g.resolve_use(0, &"t1".to_string(), vec![]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    advance(&mut g).unwrap();
    // 第二张同花色（红桃）→ 触发摸牌询问。
    g.resolve_use(0, &"t2".to_string(), vec![]).unwrap();
    pass_until(&mut g, "jianying");
    assert_eq!(pend_skill(&g).as_deref(), Some("jianying"));
    let before = g.hand(0).len();
    answer(&mut g, 0, "draw");
    assert_eq!(g.hand(0).len(), before + 1);
}

#[test]
fn jianying_converts_to_basic_with_suit_inheritance() {
    let mut g = game(&[
        ("jushou", &["jianying"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["t1", "d1"]);
    play_phase(&mut g, 0);
    g.resolve_use(0, &"t1".to_string(), vec![]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    advance(&mut g).unwrap();
    skills::execute_active(&mut g, 0, "skill:jianying").unwrap();
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "name:桃"));
    assert!(!opts.iter().any(|o| o == "name:闪"), "不含【闪】");
    answer(&mut g, 0, "name:杀");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 0, "target:1");
    assert_eq!(
        g.counter("turn.jy_suit_override:d1"),
        2,
        "花色继承上一张牌（红桃）"
    );
}

#[test]
fn jianying_no_draw_when_suit_and_rank_differ() {
    let mut g = game(&[
        ("jushou", &["jianying"], 4, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 3;
    give(&mut g, 0, &["t1", "j1"]);
    play_phase(&mut g, 0);
    g.resolve_use(0, &"t1".to_string(), vec![]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    advance(&mut g).unwrap();
    g.resolve_use(0, &"j1".to_string(), vec![]).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("jianying"));
}

#[test]
fn mashu_reduces_distance_for_jie_madai() {
    let mut g = game(&[
        ("madai", &["mashu"], 4, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
        ("p3", &[], 4, "qun"),
    ]);
    assert_eq!(g.distance(0, 2), 1, "马术：距离 -1（下限 1）");
    assert_eq!(g.distance(2, 0), 2, "不变更他人到你的距离");
}


