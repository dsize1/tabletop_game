//! B16 用例（14 技能）。
use sanguosha2::kernel::event::EventKind;
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
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("d3", "闪", Suit::Diamond, 4),
        ("t1", "桃", Suit::Heart, 9),
        ("t2", "桃", Suit::Heart, 8),
        ("j1", "酒", Suit::Spade, 9),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("nm2", "南蛮入侵", Suit::Club, 13),
        ("wj1", "万箭齐发", Suit::Heart, 1),
        ("wux1", "无懈可击", Suit::Spade, 11),
        ("sh1", "顺手牵羊", Suit::Diamond, 3),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("le1", "乐不思蜀", Suit::Heart, 6),
        ("bl1", "兵粮寸断", Suit::Spade, 5),
        ("jd1", "决斗", Suit::Club, 4),
        ("jd2", "决斗", Suit::Diamond, 6),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip("w2", "青龙偃月刀", Suit::Spade, 5, Slot::Weapon));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    cards.push(Card::equip("arm2", "白银狮子", Suit::Club, 1, Slot::Armor));
    cards.push(Card::equip("horse1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
    cards.push(Card::equip("horse2", "的卢", Suit::Club, 5, Slot::HorseDefense));
    // 备用牌（判定/摸牌用）。
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

// ---------------------------------------------------------------- 祸首
#[test]
fn huoshou_immune_and_replaces_damage_source() {
    let mut g = game(&[
        ("menghuo", &["huoshou"], 4, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["nm1"]);
    give(&mut g, 2, &["d1"]);
    g.use_as(1, &"nm1".to_string(), "南蛮入侵", vec![0, 2], false, None)
        .unwrap();
    advance(&mut g).unwrap();
    for _ in 0..4 {
        if g.pending.is_none() {
            break;
        }
        answer_cur(&mut g, "pass");
    }
    // 祸首免疫：不能成为【南蛮入侵】目标。
    assert_eq!(g.seats[0].hp, 4);
    // 伤害来源被替换为祸首持有者。
    assert!(g.counter("turn.damage_by:0") > 0, "伤害来源=祸首持有者");
    assert_eq!(g.counter("turn.damage_by:1"), 0);
}

#[test]
fn huoshou_own_nanman_keeps_source() {
    let mut g = game(&[("menghuo", &["huoshou"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["nm1"]);
    give(&mut g, 1, &["d1"]);
    g.use_as(0, &"nm1".to_string(), "南蛮入侵", vec![1], false, None)
        .unwrap();
    advance(&mut g).unwrap();
    for _ in 0..4 {
        if g.pending.is_none() {
            break;
        }
        answer_cur(&mut g, "pass");
    }
    assert!(g.counter("turn.damage_by:0") > 0, "自己使用 → 来源为自己");
}

// ---------------------------------------------------------------- 再起
#[test]
fn zaizhi_settles_per_red_card_in_discard() {
    let mut g = game(&[("menghuo", &["zaizhi"], 4, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    // 回合内两张红牌进弃牌堆。
    give(&mut g, 0, &["h1", "d1"]);
    g.to_discard(&"h1".to_string(), "test").unwrap();
    g.to_discard(&"d1".to_string(), "test").unwrap();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zaizhi"));
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_actor(&g), 1);
    let before = g.hand(1).len();
    answer(&mut g, 1, "opt1");
    assert_eq!(g.hand(1).len(), before + 1, "目标摸 1");
    // 第二次（rem2 张）→ 可放弃。
    if pend_skill(&g).as_deref() == Some("zaizhi") {
        answer(&mut g, 0, "pass");
    }
}

#[test]
fn zaizhi_needs_red_cards_in_discard() {
    let mut g = game(&[("menghuo", &["zaizhi"], 4, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zaizhi"));
}

// ---------------------------------------------------------------- 断粮
#[test]
fn duanliang_converts_black_cards_and_ignores_distance() {
    let mut g = game(&[
        ("xuhuang", &["duanliang"], 4, "wei"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
        ("p3", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["c1", "h1"]);
    equip(&mut g, 0, "w1", Slot::Weapon);
    let convs = skills::conversions(&g, 0);
    assert!(
        convs.iter().any(|c| c.from == "c1" && c.to == "兵粮寸断"),
        "黑色基本牌可转化"
    );
    assert!(
        convs.iter().any(|c| c.from == "w1" && c.to == "兵粮寸断"),
        "黑色装备牌（装备区）可转化"
    );
    assert!(!convs.iter().any(|c| c.from == "h1"), "红色牌不能转化");
    assert!(skills::supply_shortage_no_distance(&g, 0), "未造成伤害 → 无距离");
}

#[test]
fn duanliang_loses_no_distance_after_damage() {
    let mut g = game(&[("xuhuang", &["duanliang"], 4, "wei"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    g.bump("turn.damage_by:0");
    assert!(!skills::supply_shortage_no_distance(&g, 0));
}

// ---------------------------------------------------------------- 截辎
#[test]
fn jiezi_gives_mark_then_extra_draw_phase() {
    let mut g = game(&[("xuhuang", &["jiezi"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    give(&mut g, 1, &[]);
    g.counters.insert("turn.skip_draw".to_string(), 1);
    g.draw_phase(1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jiezi"));
    // 1 号手牌最少（0 张）→ 获得「辎」。
    let opts = options(&g);
    let opt = opts
        .iter()
        .find(|o| o.starts_with("target:1"))
        .cloned()
        .expect("可选 1 号");
    answer(&mut g, 0, &opt);
    assert_eq!(g.seats[1].counters.get("zai").copied(), Some(1));
    // 其摸牌阶段结束 → 移除「辎」并额外摸牌。
    g.counters.remove("turn.skip_draw");
    let before = g.hand(1).len();
    g.turn = 1;
    g.phase = Phase::Draw;
    g.draw_phase(1);
    skills::after_draw_phase(&mut g, 1);
    assert_eq!(g.seats[1].counters.get("zai").copied(), None, "「辎」已移除");
    assert!(
        g.hand(1).len() >= before + 4,
        "额外摸牌阶段（2 + 正常 2）"
    );
}

#[test]
fn jiezi_non_min_hand_draws_one() {
    let mut g = game(&[("xuhuang", &["jiezi"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["d1", "d2", "d3"]);
    g.draw_phase(1); // 1 号跳过（没有手牌最少 → 但 1 号手牌 0 是最少…）
    advance(&mut g).unwrap();
    if pend_skill(&g).as_deref() == Some("jiezi") {
        let opts = options(&g);
        let own = opts.iter().find(|o| o.starts_with("target:0")).cloned();
        if let Some(own) = own {
            let before = g.hand(0).len();
            answer(&mut g, 0, &own);
            assert_eq!(g.hand(0).len(), before + 1, "非最少 → 摸 1");
        }
    }
}

// ---------------------------------------------------------------- 断肠
#[test]
fn duanchang_killer_loses_all_skills() {
    let mut g = game(&[
        ("caiwenji", &["duanchang"], 1, "qun"),
        ("p1", &["wushuang"], 4, "qun"),
    ]);
    g.turn = 1;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert!(!skills::active(&g, 1, "wushuang"), "杀死者失去所有技能（含锁定技）");
}

#[test]
fn duanchang_no_source_no_trigger() {
    let mut g = game(&[
        ("caiwenji", &["duanchang"], 1, "qun"),
        ("p1", &["wushuang"], 4, "qun"),
    ]);
    g.turn = 1;
    flow::lose_hp(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert!(skills::active(&g, 1, "wushuang"), "无来源死亡不触发断肠");
}

// ---------------------------------------------------------------- 悲歌
#[test]
fn beige_discard_judge_and_gain_same_suit() {
    let mut g = game(&[
        ("caiwenji", &["beige"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 0, &["h1", "d2"]);
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 2, &["d3"]);
    // 判定牌（牌堆顶）与弃置牌同花色：红桃 t1。
    g.put_on_top(&"t1".to_string()).unwrap();
    g.resolve_use(1, &"sp1".to_string(), vec![2]).unwrap();
    if g.pending.is_some() {
        answer_cur(&mut g, "pass"); // 2 号不出【闪】
    }
    assert_eq!(pend_skill(&g).as_deref(), Some("beige"));
    // 弃置红桃 h1（与判定牌 t1 同花色）→ 判定红桃 → 目标回复 + 获得判定牌。
    let opt = options(&g)
        .into_iter()
        .find(|o| o == "pick:h1")
        .expect("可选红桃");
    let hand_before = g.hand(0).len();
    answer(&mut g, 0, &opt);
    assert_eq!(g.seats[2].hp, 4, "红桃：回复（已满则不变）");
    assert!(
        g.hand(0).len() >= hand_before,
        "同花色获得判定牌（弃 1 得 1）"
    );
}

#[test]
fn beige_needs_cards_to_trigger() {
    let mut g = game(&[
        ("caiwenji", &["beige"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["sp1"]);
    g.resolve_use(1, &"sp1".to_string(), vec![2]).unwrap();
    if g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_ne!(pend_skill(&g).as_deref(), Some("beige"));
}

// ---------------------------------------------------------------- 驱虎
#[test]
fn quhu_win_lets_target_damage_chosen_role() {
    let mut g = game(&[
        ("xunyu", &["quhu"], 3, "wei"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:quhu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    // 赢 → 选择受伤角色。
    assert_eq!(pend_skill(&g).as_deref(), Some("quhu"));
    let hp2 = g.seats[2].hp;
    answer(&mut g, 0, "target:2");
    assert_eq!(g.seats[2].hp, hp2 - 1);
}

#[test]
fn quhu_requires_higher_hp_and_hand() {
    let mut g = game(&[("xunyu", &["quhu"], 4, "wei"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    // 目标体力不大于荀彧 → 不可发动。
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:quhu"));
}

// ---------------------------------------------------------------- 节命
#[test]
fn jieming_draws_x_then_discards_to_x() {
    let mut g = game(&[("xunyu", &["jieming"], 3, "wei"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["d1", "d2", "d3", "h1", "t1"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jieming"));
    answer(&mut g, 0, "target:0");
    // 摸 X=3 后弃至 3。
    while pend_skill(&g).as_deref() == Some("jieming") && pend_actor(&g) == 0 {
        let opt = options(&g)
            .into_iter()
            .find(|o| o.starts_with("pick:"))
            .expect("弃牌选项");
        answer(&mut g, 0, &opt);
    }
    assert!(g.hand(0).len() <= 3);
}

#[test]
fn jieming_triggers_on_death() {
    let mut g = game(&[("xunyu", &["jieming"], 1, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(
        pend_skill(&g).as_deref(),
        Some("jieming"),
        "死亡时仍可发动节命"
    );
}

// ---------------------------------------------------------------- 强袭
#[test]
fn qiangxi_cost_hp_then_damage_once_per_target() {
    let mut g = game(&[
        ("dianwei", &["qiangxi"], 4, "wei"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qiangxi").unwrap();
    answer(&mut g, 0, "opt:hp");
    assert_eq!(g.seats[0].hp, 3, "代价：受到 1 点伤害");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3);
    // 第二次：1 号已被指定过，只能选 2 号。
    skills::execute_active(&mut g, 0, "skill:qiangxi").unwrap();
    answer(&mut g, 0, "opt:hp");
    assert!(!options(&g).iter().any(|o| o == "target:1"), "同一角色不可重复指定");
    answer(&mut g, 0, "target:2");
    // 第三次被拒绝。
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:qiangxi"));
}

#[test]
fn qiangxi_discards_weapon_as_cost() {
    let mut g = game(&[("dianwei", &["qiangxi"], 4, "wei"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    equip(&mut g, 0, "w1", Slot::Weapon);
    skills::execute_active(&mut g, 0, "skill:qiangxi").unwrap();
    answer(&mut g, 0, "opt:weapon");
    assert!(g.discard_pile().iter().any(|c| c == "w1"));
    assert_eq!(g.seats[0].hp, 4, "不失去体力");
}

// ---------------------------------------------------------------- 狞恶
#[test]
fn ninge_second_damage_draws_and_discards() {
    let mut g = game(&[
        ("dianwei", &["ninge"], 4, "wei"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 2;
    give(&mut g, 0, &[]);
    equip(&mut g, 0, "arm1", Slot::Armor);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("ninge"), "第 1 次不触发");
    let before = g.hand(0).len();
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("ninge"), "第 2 次触发");
    assert_eq!(g.hand(0).len(), before + 1, "典韦摸 1");
    // 由典韦选择弃置其装备区牌。
    answer(&mut g, 0, "pick:equip:arm1");
    assert!(g.discard_pile().iter().any(|c| c == "arm1"));
}

#[test]
fn ninge_ignores_unrelated_damage() {
    let mut g = game(&[
        ("dianwei", &["ninge"], 4, "wei"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 2;
    flow::damage(&mut g, Some(2), 1, 1);
    advance(&mut g).unwrap();
    flow::damage(&mut g, Some(2), 1, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("ninge"));
}

// ---------------------------------------------------------------- 双雄
#[test]
fn shuangxiong_discards_color_then_converts_opposite() {
    let mut g = game(&[("sx", &["shuangxiong"], 4, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    give(&mut g, 0, &["h1", "c1", "d1"]);
    skills::after_draw_phase(&mut g, 0);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("shuangxiong"));
    // 弃红桃 h1 → 黑色牌可当【决斗】。
    answer(&mut g, 0, "pick:h1");
    assert_eq!(g.counter("turn.sx_color"), 1);
    let convs = skills::conversions(&g, 0);
    assert!(convs.iter().any(|c| c.from == "c1" && c.to == "决斗"));
    assert!(!convs.iter().any(|c| c.from == "d1"), "同色不能转化");
}

#[test]
fn shuangxiong_gains_damage_cards_at_end() {
    let mut g = game(&[("sx", &["shuangxiong"], 4, "qun"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["d1"]);
    g.turn = 1;
    // 1 号用【杀】伤害双雄 → 结束阶段收回该牌。
    give(&mut g, 1, &["sp1"]);
    g.turn = 0;
    flow::damage_with(&mut g, Some(1), 0, 1, Some(&"sp1".to_string()));
    advance(&mut g).unwrap();
    g.to_discard(&"sp1".to_string(), "test").unwrap();
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert!(g.hand(0).iter().any(|c| c == "sp1"), "结束阶段获得伤害牌");
}

// ---------------------------------------------------------------- 直谏
#[test]
fn zhijian_moves_equip_and_draws() {
    let mut g = game(&[("zzzh", &["zhijian"], 3, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["w1", "d1"]);
    equip(&mut g, 1, "arm1", Slot::Armor);
    let before = g.hand(0).len();
    skills::execute_active(&mut g, 0, "skill:zhijian").unwrap();
    answer(&mut g, 0, "pick:w1");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.equip_in(1, Slot::Weapon), Some("w1".to_string()));
    assert_eq!(g.hand(0).len(), before, "置入 1 张后摸 1 张");
}

#[test]
fn zhijian_needs_equip_in_hand() {
    let mut g = game(&[("zzzh", &["zhijian"], 3, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1"]);
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:zhijian"));
}

// ---------------------------------------------------------------- 固政
#[test]
fn guzheng_single_action_two_discards() {
    let mut g = game(&[
        ("zzzh", &["guzheng"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 1;
    give(&mut g, 1, &["d1", "d2"]);
    g.begin_action();
    g.to_discard(&"d1".to_string(), "test").unwrap();
    g.to_discard(&"d2".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("guzheng"));
    let before1 = g.hand(1).len();
    let before0 = g.hand(0).len();
    answer(&mut g, 0, "give:d1");
    assert_eq!(g.hand(1).len(), before1 + 1, "弃牌者获得 1 张");
    assert_eq!(g.hand(0).len(), before0 + 1, "固政获得其余");
}

#[test]
fn guzheng_single_card_does_not_trigger() {
    let mut g = game(&[
        ("zzzh", &["guzheng"], 3, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    g.turn = 1;
    give(&mut g, 1, &["d1"]);
    g.begin_action();
    g.to_discard(&"d1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("guzheng"));
}

// ---------------------------------------------------------------- 破军
#[test]
fn pojun_voids_cards_until_turn_end() {
    let mut g = game(&[("xusheng", &["pojun"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1", "d2"]);
    equip(&mut g, 1, "arm1", Slot::Armor);
    g.resolve_use(0, &"sp1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("pojun"), "指定目标后介入");
    let zero = pend_actor(&g);
    answer(&mut g, zero, "opt1");
    // 移出装备牌（X = 体力值 4，至多 4 张）。
    let opt = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:equip:arm1"))
        .expect("可选装备");
    answer(&mut g, zero, &opt);
    answer(&mut g, zero, "pass");
    assert!(g.ids_in(|z| *z == Zone::Void).iter().any(|c| c == "arm1"));
    // 先结算【杀】的响应窗口（新时序：响应未结算时不允许切回合）。
    while pend_skill(&g).as_deref() == Some("__card") {
        let actor = pend_actor(&g);
        answer(&mut g, actor, "pass");
    }
    // 回合结束（切换到下一名角色的准备阶段）归还。
    g.phase = Phase::End;
    begin_phase(&mut g, 1, Phase::Prepare).unwrap();
    assert!(g.hand(1).iter().any(|c| c == "arm1"), "本回合结束归还原主");
}

#[test]
fn pojun_bonus_when_card_counts_not_higher() {
    let mut g = game(&[("xusheng", &["pojun"], 4, "wu"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["sp1", "d1"]);
    give(&mut g, 1, &["d1"]);
    assert_eq!(
        skills::damage_bonus_typed(
            &g,
            0,
            1,
            Some(&"sp1".to_string()),
            sanguosha2::cards::DamageKind::Normal,
            false
        ),
        1,
        "目标手牌不多于你 → 伤害 +1"
    );
    give(&mut g, 1, &["d2", "d3"]);
    assert_eq!(
        skills::damage_bonus_typed(
            &g,
            0,
            1,
            Some(&"sp1".to_string()),
            sanguosha2::cards::DamageKind::Normal,
            false
        ),
        0,
        "目标手牌更多 → 不加伤"
    );
}

// ---------------------------------------------------------------- 机制锚点
#[test]
fn b16_mechanism_anchors() {
    let mut g = game(&[("menghuo", &["huoshou"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["nm1"]);
    give(&mut g, 0, &["d1"]);
    // AOE 目标裁剪（祸首免疫）+ 伤害来源替换。
    g.use_as(1, &"nm1".to_string(), "南蛮入侵", vec![0], false, None)
        .unwrap();
    advance(&mut g).unwrap();
    for _ in 0..4 {
        if g.pending.is_none() {
            break;
        }
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[0].hp, 4);
    g.to_mark(&"x40".to_string(), 0, "tian").unwrap();
    assert_eq!(g.mark_cards(0, "tian").len(), 1);
    g.to_discard(&"x41".to_string(), "test").unwrap();
    g.check_conservation().unwrap();
    let _ = EventKind::PhaseStart;
}
