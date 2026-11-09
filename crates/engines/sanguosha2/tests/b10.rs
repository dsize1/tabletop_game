//! B10 用例：界甘宁（奇袭/奋威）、界吕蒙（克己/勤学/博图）、界黄盖（苦肉/诈降）、
//! 界周瑜（英姿/反间）、界大乔（流离/国色）、界陆逊（谦逊/连营）、界孙权（制衡/救援）、界孙尚香（枭姬/结姻）。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn seat(name: &str, skills: &[&str], hp: i32) -> Seat {
    Seat::new(name, "gen", skills, hp)
}

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("c1", "杀", Suit::Spade, 7),
        ("c2", "闪", Suit::Heart, 2),
        ("c3", "桃", Suit::Diamond, 3),
        ("c4", "过河拆桥", Suit::Club, 4),
        ("c5", "顺手牵羊", Suit::Spade, 5),
        ("c6", "乐不思蜀", Suit::Heart, 6),
        ("c7", "无中生有", Suit::Heart, 8),
        ("c8", "杀", Suit::Diamond, 9),
        ("c9", "杀", Suit::Heart, 10),
        ("c10", "青釭剑", Suit::Spade, 6),
        ("c11", "八卦阵", Suit::Heart, 2),
        ("c12", "南蛮入侵", Suit::Spade, 7),
        ("c13", "方片杀", Suit::Diamond, 1),
        ("c14", "方片闪", Suit::Diamond, 4),
        ("c15", "黑桃杀", Suit::Spade, 13),
        ("c16", "梅花杀", Suit::Club, 3),
        ("c17", "赤兔", Suit::Heart, 5),
        ("c18", "决斗", Suit::Club, 1),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| match *name {
            "青釭剑" => Card::equip(*id, name, *suit, *rank, Slot::Weapon),
            "八卦阵" => Card::equip(*id, name, *suit, *rank, Slot::Armor),
            "赤兔" => Card::equip(*id, name, *suit, *rank, Slot::HorseOffense),
            "方片杀" => Card::new(*id, "杀", Suit::Diamond, *rank),
            "方片闪" => Card::new(*id, "闪", Suit::Diamond, *rank),
            "黑桃杀" => Card::new(*id, "杀", Suit::Spade, *rank),
            "梅花杀" => Card::new(*id, "杀", Suit::Club, *rank),
            _ => Card::new(*id, name, *suit, *rank),
        })
        .collect();
    for i in 19..=90 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
    }
    cards
}

fn game(specs: &[(&str, &[&str], i32)]) -> Game {
    let seats: Vec<Seat> = specs.iter().map(|(n, s, hp)| seat(n, s, *hp)).collect();
    Game::new(seats, deck())
}

fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids {
        g.to_hand(&id.to_string(), seat).unwrap();
    }
}

fn equip(g: &mut Game, seat: usize, id: &str, slot: Slot) {
    g.to_equip(&id.to_string(), seat, slot).unwrap();
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn pend_skill(g: &Game) -> Option<String> {
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

fn start_phase(g: &mut Game, seat: usize, phase: Phase) {
    g.phase = if phase == Phase::Prepare {
        Phase::Discard
    } else {
        Phase::Prepare
    };
    begin_phase(g, seat, phase).unwrap();
}

// --------------------------------------------------------------- 奇袭 / 奋威
#[test]
fn qixi_converts_black_cards_including_equip_zone() {
    let mut g = game(&[("ganning", &["qixi"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c4"]); // 梅花过河拆桥（黑）
    equip(&mut g, 0, "c10", Slot::Weapon); // 黑桃装备
    let convs = skills::conversions(&g, 0);
    assert!(convs.iter().any(|c| c.from == "c4" && c.to == "过河拆桥"));
    assert!(convs.iter().any(|c| c.from == "c10"), "装备区的黑色牌也能奇袭");
    assert!(convs.iter().all(|c| c.from != "c2"), "红色牌不能奇袭");
}

#[test]
fn fenwei_nullifies_chosen_targets_of_multi_target_card() {
    let mut g = game(&[
        ("ganning", &["fenwei"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    // 单元级验证：压入奋威询问 → 选目标 → 确认 → fenwei_void 生效
    assert!(skills::start_fenwei(&mut g, &[0, 1, 2], 2));
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("fenwei"));
    answer(&mut g, 0, "choose:1");
    assert!(options(&g).iter().any(|o| o == "confirm"), "{:?}", options(&g));
    answer(&mut g, 0, "confirm");
    assert!(g.fenwei_void.contains(&1), "被选目标进入无效集合");
    assert!(!g.fenwei_void.contains(&0));
    assert!(g.seats[0].flags.contains("fenwei_used"), "限定技已用");
    g.check_conservation().expect("守恒");
}

// --------------------------------------------------------------- 克己 / 勤学 / 博图
#[test]
fn keji_skips_discard_phase_without_sha() {
    let mut g = game(&[("lumeng", &["keji"], 4), ("p1", &[], 4)]);
    assert!(skills::skip_discard_phase(&g, 0));
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    if g.pending.is_some() {
        let a = pend(&g).actor();
        answer(&mut g, a, "pass");
    }
    assert!(!skills::skip_discard_phase(&g, 0), "使用过【杀】后不再克己");
}

#[test]
fn qinxue_awakens_on_prepare_and_end() {
    let mut g = game(&[("lumeng", &["qinxue"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1", "c2", "c3", "c4", "c5"]); // 5 张 >= 3+2
    start_phase(&mut g, 0, Phase::Prepare);
    assert_eq!(pend_skill(&g).as_deref(), Some("qinxue"));
    answer(&mut g, 0, "opt2");
    assert_eq!(g.seats[0].max_hp, 2);
    assert!(g.seats[0].flags.contains("qinxue_awakened"));
}

#[test]
fn botu_grants_extra_turn_after_four_suits() {
    let mut g = game(&[("lumeng", &["botu"], 4), ("p1", &[], 4)]);
    g.counters.insert("turn.discard_suits".to_string(), 15);
    skills::botu_maybe(&mut g, 0);
    assert_eq!(g.extra_turn, Some(0));
}

// --------------------------------------------------------------- 苦肉 / 诈降
#[test]
fn kurou_discards_card_then_loses_hp() {
    let mut g = game(&[("huanggai", &["kurou"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:kurou").unwrap();
    answer(&mut g, 0, "pick:c1");
    assert_eq!(g.seats[0].hp, 3);
    assert!(g.discard_pile().contains(&"c1".to_string()));
}

#[test]
fn zhaxiang_draws_three_and_boosts_sha_limit_in_play_phase() {
    let mut g = game(&[("huanggai", &["zhaxiang"], 4), ("p1", &[], 4)]);
    play(&mut g, 0);
    let before = g.hand(0).len();
    flow::lose_hp(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(g.hand(0).len(), before + 3, "失去体力后摸 3");
    assert_eq!(skills::sha_limit_bonus(&g, 0), 1, "出牌阶段【杀】次数 +1");
}

// --------------------------------------------------------------- 英姿 / 反间
#[test]
fn yingzi_hand_limit_equals_max_hp_and_draws_one_more() {
    let mut g = game(&[("zhouyu", &["yingzi"], 3), ("p1", &[], 4)]);
    g.seats[0].max_hp = 5;
    assert_eq!(g.hand_limit(0), 5, "手牌上限 = 体力上限");
    let before = g.hand(0).len();
    g.draw_phase(0);
    assert_eq!(g.hand(0).len(), before + 3, "摸牌阶段 2+1");
}

#[test]
fn fanjian_lets_target_choose_lose_hp_or_discard_same_suit() {
    let mut g = game(&[("zhouyu", &["fanjian"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c2"]); // 红桃闪（交给目标）
    give(&mut g, 1, &["c3"]); // 方片桃（不同花色）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:fanjian").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:c2");
    assert_eq!(pend(&g).actor(), 1, "由目标二选一");
    answer(&mut g, 1, "opt2");
    assert_eq!(g.seats[1].hp, 3, "失去 1 点体力");
    g.check_conservation().expect("守恒");
}

// --------------------------------------------------------------- 流离 / 国色
#[test]
fn liuli_discards_and_transfers_sha() {
    let mut g = game(&[
        ("daqiao", &["liuli"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 1, &["c1"]); // p1 出杀
    give(&mut g, 0, &["c2", "c14"]); // 大乔受伤前有闪/可弃牌
    play(&mut g, 1);
    g.resolve_use(1, &"c1".to_string(), vec![0]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("liuli"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:c2");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "target:2"), "{opts:?}");
    answer(&mut g, 0, "target:2");
    // 转移后 p2 需要出【闪】
    if g.pending.is_some() {
        let a = pend(&g).actor();
        assert_eq!(a, 2, "新目标进入【闪】响应");
        answer(&mut g, 2, "pass");
    }
    assert_eq!(g.seats[0].hp, 4, "大乔不再是目标");
    assert_eq!(g.seats[2].hp, 3);
    g.check_conservation().expect("守恒");
}

#[test]
fn guose_diamond_as_indulgence_draws_one_and_opt2_needs_full_field() {
    let mut g = game(&[("daqiao", &["guose"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c3"]); // 方片桃
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:guose").unwrap();
    let opts = options(&g);
    assert!(opts.contains(&"opt1".to_string()));
    assert!(!opts.contains(&"opt2".to_string()), "场上无乐 → 第 2 项不可选");
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:c3");
    answer(&mut g, 0, "target:1");
    assert!(!g.judge_zone(1).is_empty(), "方片当【乐不思蜀】进了判定区");
    assert_eq!(g.hand(0).len(), 1, "选项 1 摸 1 张");
    g.check_conservation().expect("守恒");
}

// --------------------------------------------------------------- 谦逊 / 连营
#[test]
fn qianxun_moves_all_hand_out_for_single_target_trick() {
    let mut g = game(&[("p1", &[], 4), ("luxun", &["qianxun"], 4)]);
    give(&mut g, 0, &["c5"]); // 顺手牵羊
    give(&mut g, 1, &["c2", "c3"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c5".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qianxun"));
    answer(&mut g, 1, "opt1");
    assert!(g.hand(1).is_empty(), "所有手牌移出游戏");
    assert_eq!(g.void_return.len(), 2);
    g.check_conservation().expect("守恒");
}

#[test]
fn lianying_lets_up_to_x_characters_draw() {
    let mut g = game(&[("luxun", &["lianying"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c2", "c3"]);
    // 一次动作内失去全部手牌 → X = 2
    g.begin_action();
    g.to_discard(&"c2".to_string(), "test").unwrap();
    g.to_discard(&"c3".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("lianying"));
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "target:0"), "{opts:?}");
    let before = g.hand(0).len();
    answer(&mut g, 0, "target:0");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(0).len(), before + 1);
    assert_eq!(g.hand(1).len(), 1);
}

// --------------------------------------------------------------- 制衡 / 救援
#[test]
fn zhiheng_discards_hand_and_equip_then_extra_draw_when_hand_empty() {
    let mut g = game(&[("sunquan", &["zhiheng"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    equip(&mut g, 0, "c10", Slot::Weapon);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhiheng").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "choose:c10");
    answer(&mut g, 0, "confirm");
    // 弃 2 张（手牌清空）→ 摸 2 + 额外 1 = 3
    assert_eq!(g.hand(0).len(), 3, "弃光手牌额外摸 1");
    g.check_conservation().expect("守恒");
}

#[test]
fn jiuyuan_asks_wu_ally_to_redirect_heal_to_lord() {
    let mut g = game(&[("sunquan", &["jiuyuan"], 3), ("zhouyu", &[], 4)]);
    g.seats[0].identity = "lord".to_string();
    g.seats[1].kingdom = "wu".to_string();
    g.seats[1].hp = 4;
    g.seats[0].hp = 2;
    g.turn = 1; // 周瑜的回合内
    flow::heal(&mut g, 1, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jiuyuan"));
    answer(&mut g, 1, "opt1");
    assert_eq!(g.seats[0].hp, 3, "主公回复 1 点");
    assert_eq!(g.seats[1].hp, 4, "周瑜没有自行回复");
    g.check_conservation().expect("守恒");
}

// --------------------------------------------------------------- 枭姬 / 结姻
#[test]
fn xiaoji_draws_two_when_losing_equip() {
    let mut g = game(&[("sunshangxiang", &["xiaoji"], 3), ("p1", &[], 4)]);
    equip(&mut g, 0, "c10", Slot::Weapon);
    let before = g.hand(0).len();
    g.to_discard(&"c10".to_string(), "test").unwrap();
    assert_eq!(g.hand(0).len(), before + 2, "失去装备摸 2");
}

#[test]
fn jieyin_equal_hp_draws_and_heals_both() {
    let mut g = game(&[("sunshangxiang", &["jieyin"], 3), ("p1", &[], 4)]);
    g.seats[0].hp = 2;
    g.seats[1].hp = 2;
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jieyin").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:c1");
    assert_eq!(g.seats[0].hp, 3, "相等 → 双方各回复 1");
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.hand(0).len(), 1, "双方各摸 1");
    assert_eq!(g.hand(1).len(), 1);
    g.check_conservation().expect("守恒");
}

#[test]
fn jieyin_equip_from_own_zone_triggers_xiaoji() {
    let mut g = game(&[("sunshangxiang", &["jieyin", "xiaoji"], 3), ("p1", &[], 4)]);
    equip(&mut g, 0, "c10", Slot::Weapon);
    g.seats[0].hp = 1;
    play(&mut g, 0);
    let before = g.hand(0).len();
    skills::execute_active(&mut g, 0, "skill:jieyin").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt2");
    answer(&mut g, 0, "pick:c10");
    assert!(
        g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == 1)).len() == 1,
        "装备进了对方装备区"
    );
    assert!(g.hand(0).len() >= before + 2, "用自己装备区的装备会触发枭姬");
    g.check_conservation().expect("守恒");
}

#[test]
fn b10_none_of_the_skills_use_event_only_use_card() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/skills/b10.rs"
    ))
    .unwrap();
    assert!(!src.contains("use_card("), "b10 技能不得调用 use_card");
}
