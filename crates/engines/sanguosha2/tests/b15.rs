//! B15 用例（19 技能）。
use sanguosha2::kernel::event::EventKind;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("h1", "杀", Suit::Heart, 12),
        ("c1", "杀", Suit::Club, 11),
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("d3", "闪", Suit::Diamond, 4),
        ("h2", "闪", Suit::Heart, 5),
        ("c2", "闪", Suit::Club, 6),
        ("t1", "桃", Suit::Heart, 9),
        ("t2", "桃", Suit::Heart, 10),
        ("j1", "酒", Suit::Spade, 9),
        ("j2", "酒", Suit::Club, 3),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("nm2", "南蛮入侵", Suit::Club, 13),
        ("wj1", "万箭齐发", Suit::Heart, 1),
        ("wux1", "无懈可击", Suit::Spade, 11),
        ("sh1", "顺手牵羊", Suit::Diamond, 3),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("le1", "乐不思蜀", Suit::Heart, 6),
        ("juedou", "决斗", Suit::Club, 4),
        ("wz1", "无中生有", Suit::Heart, 7),
        ("hgj", "火攻", Suit::Heart, 2),
        ("ts1", "铁索连环", Suit::Club, 10),
        ("sp2", "杀", Suit::Spade, 10),
        ("sp3", "杀", Suit::Spade, 9),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip("w2", "青龙偃月刀", Suit::Spade, 5, Slot::Weapon));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    cards.push(Card::equip("horse1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
    // 备用牌（判定/摸牌用）。
    for i in 30..=99 {
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

// ---------------------------------------------------------------- 天义
#[test]
fn tianyi_win_grants_bonus_and_extra_target() {
    let mut g = game(&[("tsc", &["tianyi"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:tianyi").unwrap();
    answer(&mut g, 0, "target:1");
    // 拼点：己方先选（13 点），对方 2 点。
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    assert_eq!(g.counter("turn.tianyi"), 1);
    assert_eq!(skills::sha_limit_bonus(&g, 0), 1);
    assert_eq!(skills::extra_targets(&g, 0, None, "杀"), 1);
    assert!(skills::sha_no_distance(&g, 0, &"sp1".to_string()));
}

#[test]
fn tianyi_lose_blocks_sha_use() {
    let mut g = game(&[("tsc", &["tianyi"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["sp1"]);
    skills::execute_active(&mut g, 0, "skill:tianyi").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 1, "choose:sp1");
    assert_eq!(g.counter("turn.tianyi_lose"), 1);
    assert_eq!(skills::sha_limit_override(&g, 0), Some(0));
    assert_eq!(g.sha_limit(0), 0);
}

// ---------------------------------------------------------------- 酣战
#[test]
fn hanzhan_forces_random_and_gains_biggest_sha() {
    let mut g = game(&[
        ("tsc", &["tianyi", "hanzhan"], 4, "wu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["h1", "d1", "d2"]);
    skills::execute_active(&mut g, 0, "skill:tianyi").unwrap();
    answer(&mut g, 0, "target:1");
    // 拼点前：酣战询问（持有者 = 界太史慈）。
    assert_eq!(pend_skill(&g).as_deref(), Some("hanzhan"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "choose:sp1");
    // 对方被强制随机：不应出现「choose:」询问。
    assert_ne!(pend_skill(&g).as_deref(), Some("pindian"));
    // 拼点后：酣战获得点数最大的【杀】（sp1 13 > h1 12）。
    assert_eq!(pend_skill(&g).as_deref(), Some("hanzhan"));
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("pick:sp1")), "可选最大点数【杀】");
    answer(&mut g, 0, "pick:sp1");
    assert!(g.hand(0).iter().any(|c| c == "sp1"));
}

#[test]
fn hanzhan_without_skill_lets_opponent_choose() {
    let mut g = game(&[("tsc", &["tianyi"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["h1"]);
    skills::execute_active(&mut g, 0, "skill:tianyi").unwrap();
    answer(&mut g, 0, "target:1");
    assert_ne!(pend_skill(&g).as_deref(), Some("hanzhan"));
    answer(&mut g, 0, "choose:sp1");
    assert_eq!(pend_actor(&g), 1, "对手正常自选拼点牌");
}

// ---------------------------------------------------------------- 肉林
#[test]
fn roulin_adds_flash_requirement_and_stacks_with_wushuang() {
    let mut g = game(&[
        ("dz", &["roulin"], 8, "qun"),
        ("f1", &[], 3, "wei"),
        ("lb", &["wushuang"], 4, "qun"),
    ]);
    g.seats[1].male = false;
    g.seats[0].male = true;
    use sanguosha2::cards::{Effect, ResponseKind};
    assert_eq!(
        skills::response_need(&g, 1, 0, ResponseKind::Shan, Effect::Sha),
        2,
        "肉林：需 2 张【闪】"
    );
    assert_eq!(
        skills::response_need(&g, 1, 2, ResponseKind::Shan, Effect::Sha),
        2,
        "无双：需 2 张【闪】"
    );
    // 无双 + 肉林（吕布对女性）：3 张（相加）。
    let mut g2 = game(&[
        ("dz2", &["roulin"], 8, "qun"),
        ("f2", &[], 3, "wei"),
        ("lb2", &["wushuang"], 4, "qun"),
    ]);
    g2.seats[1].male = false;
    // 让吕布同时拥有肉林（模拟两个来源）：直接验证相加。
    g2.seats[2].skills.push("roulin".to_string());
    skills::rebuild_subs(&mut g2);
    assert_eq!(
        skills::response_need(&g2, 1, 2, ResponseKind::Shan, Effect::Sha),
        3,
        "无双 + 肉林 = 3 张"
    );
    // 男性目标：不触发肉林。
    g.seats[2].male = true;
    assert_eq!(skills::response_need(&g, 2, 0, ResponseKind::Shan, Effect::Sha), 1);
}

// ---------------------------------------------------------------- 崩坏
#[test]
fn bengkuai_triggers_and_loses_hp() {
    let mut g = game(&[("dz", &["bengkuai"], 8, "qun"), ("p1", &[], 3, "wei")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("bengkuai"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[0].hp, 7);
}

#[test]
fn bengkuai_tie_triggers_unique_minimum_skips() {
    // 并列最小 → 触发（用户裁定 Q1=B）。
    let mut g = game(&[("dz", &["bengkuai"], 8, "qun"), ("p1", &[], 8, "wei")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("bengkuai"), "并列最小也触发");
    // 唯一最小 → 不触发。
    let mut g2 = game(&[("dz", &["bengkuai"], 8, "qun"), ("p1", &[], 9, "wei")]);
    g2.turn = 0;
    begin_phase(&mut g2, 0, Phase::End).unwrap();
    assert_ne!(pend_skill(&g2).as_deref(), Some("bengkuai"));
}

// ---------------------------------------------------------------- 酒池
#[test]
fn jiuchi_converts_and_stacks() {
    let mut g = game(&[("dz", &["jiuchi"], 8, "qun"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["j1", "sp2", "sp3", "d1"]);
    let convs = skills::conversions(&g, 0);
    assert_eq!(convs.len(), 3, "只有黑桃手牌可当【酒】");
    assert!(skills::jiu_unlimited(&g, 0));
    // 连喝两杯（strict 使用）。
    g.use_as(0, &"j1".to_string(), "酒", vec![0], true, None)
        .unwrap();
    advance(&mut g).unwrap();
    g.use_as(0, &"sp2".to_string(), "酒", vec![0], true, None)
        .unwrap();
    advance(&mut g).unwrap();
    assert!(g.counter("turn.jiu_bonus") >= 2, "多杯叠加");
    assert_eq!(skills::dying_self_jiu(&g, 0).len(), 1, "剩余黑桃手牌可濒死自救");
    assert!(skills::dying_self_jiu(&g, 0).iter().any(|c| c == "sp3"));
}

#[test]
fn jiuchi_does_not_convert_heart() {
    let mut g = game(&[("dz", &["jiuchi"], 8, "qun"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["h1"]);
    assert!(skills::conversions(&g, 0).is_empty());
}

// ---------------------------------------------------------------- 暴虐
#[test]
fn baonue_triggers_per_point_for_qun_damage() {
    let mut g = game(&[
        ("dz", &["baonue"], 8, "qun"),
        ("q1", &[], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.turn = 1;
    flow::damage(&mut g, Some(1), 2, 2);
    advance(&mut g).unwrap();
    // 逐点：2 点 → 先问第一次判定。
    assert_eq!(pend_skill(&g).as_deref(), Some("baonue"));
    answer(&mut g, 0, "opt1");
    // 判定（任意花色）后：还有 1 次询问。
    assert_eq!(pend_skill(&g).as_deref(), Some("baonue"));
    answer(&mut g, 0, "pass");
}

#[test]
fn baonue_ignores_non_qun_or_self() {
    let mut g = game(&[
        ("dz", &["baonue"], 8, "qun"),
        ("w1", &[], 4, "wei"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.turn = 1;
    flow::damage(&mut g, Some(1), 2, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("baonue"));
}

// ---------------------------------------------------------------- 屯田
#[test]
fn tuotian_judges_after_losing_card_outside_turn() {
    let mut g = game(&[("dengai", &["tuotian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["d1", "d2"]);
    g.turn = 1;
    g.put_on_top(&"c1".to_string()).unwrap(); // 判定：梅花（非红桃）
    g.to_discard(&"d1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tuotian"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.mark_cards(0, "tian").len(), 1, "非红桃得「田」");
    assert_eq!(skills::distance_delta(&g, 0), 1);
}

#[test]
fn tuotian_own_turn_only_counts_discarded_sha() {
    let mut g = game(&[("dengai", &["tuotian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["sp1", "d1"]);
    g.turn = 0;
    g.to_discard(&"d1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("tuotian"), "回合内弃非【杀】不触发");
    g.to_discard(&"sp1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tuotian"), "回合内弃【杀】触发");
}

// ---------------------------------------------------------------- 凿险
#[test]
fn zaoxian_awaken_and_extra_turn() {
    let mut g = game(&[("dengai", &["zaoxian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    for id in ["x30", "x31", "x32"] {
        g.to_mark(&id.to_string(), 0, "tian").unwrap();
    }
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 3);
    assert!(g.seats[0].granted.iter().any(|s| s == "jixi"));
    assert!(g.seats[0].flags.contains("zaoxian_extra"));
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(g.extra_turn, Some(0), "本回合后额外回合");
}

#[test]
fn zaoxian_needs_three_tian() {
    let mut g = game(&[("dengai", &["zaoxian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.to_mark(&"x30".to_string(), 0, "tian").unwrap();
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 4);
}

// ---------------------------------------------------------------- 巨象
#[test]
fn juxiang_immune_and_gains_after_other_nanman() {
    let mut g = game(&[
        ("zhurong", &["juxiang"], 4, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["nm1", "d1", "d2"]);
    give(&mut g, 2, &["d1", "d2"]);
    g.use_as(1, &"nm1".to_string(), "南蛮入侵", vec![0, 2], false, None)
        .unwrap();
    advance(&mut g).unwrap();
    // 巨象免疫：跳过 0；2 号无【杀】→ 直接受伤。
    assert_eq!(g.seats[0].hp, 4, "巨象对【南蛮入侵】免疫");
    assert_eq!(pend_skill(&g).as_deref(), Some("juxiang"), "结算结束后询问巨象");
    answer(&mut g, 0, "opt1");
    assert!(g.hand(0).iter().any(|c| c == "nm1"), "结算结束后获得该【南蛮入侵】");
}

#[test]
fn juxiang_does_not_gain_own_nanman() {
    let mut g = game(&[("zhurong", &["juxiang"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["nm1"]);
    give(&mut g, 1, &["d1"]);
    g.use_as(0, &"nm1".to_string(), "南蛮入侵", vec![1], false, None)
        .unwrap();
    advance(&mut g).unwrap();
    assert!(g.discard_pile().iter().any(|c| c == "nm1"), "自己使用的南蛮不获得");
}

// ---------------------------------------------------------------- 烈刃
#[test]
fn liereng_pindian_and_gain_card() {
    let mut g = game(&[("zhurong", &["liereng"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "c1"]);
    give(&mut g, 1, &["d1", "d2"]);
    g.resolve_use(0, &"sp1".to_string(), vec![1]).unwrap();
    // 先放弃【闪】响应 → 造成伤害后询问烈刃。
    if g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(pend_skill(&g).as_deref(), Some("liereng"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "choose:c1"); // 自己的拼点牌
    answer(&mut g, 1, "choose:d1");
    // 拼点后：选择获得其随机手牌/装备区牌。
    if pend_skill(&g).as_deref() == Some("liereng") {
        let opts = options(&g);
        let pick = opts
            .iter()
            .find(|o| o.starts_with("pick:"))
            .cloned()
            .expect("应有可获得的牌");
        answer(&mut g, 0, &pick);
    }
    assert_eq!(g.hand(1).len(), 1, "目标失去一张牌");
}

#[test]
fn liereng_ignores_non_sha_damage() {
    let mut g = game(&[("zhurong", &["liereng"], 4, "shu"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["d1"]);
    g.turn = 0;
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("liereng"));
}

// ---------------------------------------------------------------- 长标
#[test]
fn changbiao_uses_cards_as_no_distance_sha_and_draws_x() {
    let mut g = game(&[("zhurong", &["changbiao"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["c1", "d1", "d2"]);
    skills::execute_active(&mut g, 0, "skill:changbiao").unwrap();
    answer(&mut g, 0, "pick:c1");
    answer(&mut g, 0, "pick:d1");
    answer(&mut g, 0, "done");
    answer(&mut g, 0, "target:1");
    // 造成伤害（目标没有【闪】）。
    assert!(g.sha_used(0) >= 1, "计入【杀】使用次数");
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    // 阶段结束时摸 X=2 张（伤害命中）。
    assert!(g.hand(0).len() >= 2);
}

#[test]
fn changbiao_once_per_phase() {
    let mut g = game(&[("zhurong", &["changbiao"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["c1", "d1"]);
    g.bump("turn.changbiao_used");
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:changbiao"));
}

// ---------------------------------------------------------------- 挑衅
#[test]
fn tiaoxin_target_declines_discards_card() {
    let mut g = game(&[("jiangwei", &["tiaoxin"], 4, "shu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 1, &["d1", "d2"]);
    skills::execute_active(&mut g, 0, "skill:tiaoxin").unwrap();
    answer(&mut g, 0, "target:1");
    // 目标没有【杀】→ 不询问，直接进入姜维的弃牌选择。
    let opts = options(&g);
    let pick = opts
        .iter()
        .find(|o| o.starts_with("pick:"))
        .cloned()
        .expect("应有可取牌");
    answer(&mut g, 0, &pick);
    assert_eq!(g.hand(1).len(), 1);
    assert_eq!(g.counter("turn.tiaoxin_used"), 1);
}

#[test]
fn tiaoxin_requires_range() {
    let mut g = game(&[
        ("jiangwei", &["tiaoxin"], 4, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
        ("p3", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    // 3 号距离 1（4 人局 0↔3），2 号距离 2 → 不在攻击范围。
    give(&mut g, 3, &["d1"]);
    give(&mut g, 2, &["d2"]);
    skills::execute_active(&mut g, 0, "skill:tiaoxin").unwrap();
    assert!(!options(&g).iter().any(|o| o == "target:2"));
}

// ---------------------------------------------------------------- 志继
#[test]
fn zhiji_awaken_at_end_phase_without_hand() {
    let mut g = game(&[("jiangwei", &["zhiji"], 4, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhiji"));
    answer(&mut g, 0, "opt2");
    assert_eq!(g.seats[0].max_hp, 3);
    assert!(g.seats[0].granted.iter().any(|s| s == "guanxing"));
}

#[test]
fn zhiji_requires_empty_hand() {
    let mut g = game(&[("jiangwei", &["zhiji"], 4, "shu"), ("p1", &[], 4, "wei")]);
    give(&mut g, 0, &["d1"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zhiji"));
}

// ---------------------------------------------------------------- 帷幕
#[test]
fn weimu_blocks_black_trick_and_prevents_own_turn_damage() {
    let mut g = game(&[
        ("jia", &["weimu"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    // 黑色锦囊不能成为目标。
    assert!(!skills::can_target(&g, 1, "过河拆桥", 0, Some(&"gh1".to_string())));
    assert!(skills::can_target(&g, 1, "过河拆桥", 0, Some(&"hgj".to_string())));
    // 自己回合内伤害被防止并摸 2×点数。
    g.turn = 0;
    let before = g.hand(0).len();
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 3, "回合内伤害被防止");
    assert_eq!(g.hand(0).len(), before + 4, "摸 2×点数");
}

#[test]
fn weimu_does_not_prevent_other_turn_damage() {
    let mut g = game(&[("jia", &["weimu"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 2, "他人回合内不防止");
}

// ---------------------------------------------------------------- 乱武
#[test]
fn luanwu_each_other_chooses_then_final_sha() {
    let mut g = game(&[
        ("jiaxu", &["luanwu"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 2, &["d1", "d2"]);
    skills::execute_active(&mut g, 0, "skill:luanwu").unwrap();
    // 第一个其他角色选择：失去体力。
    answer_cur(&mut g, "lose");
    // 第二个：失去体力。
    if g.pending.is_some() {
        answer_cur(&mut g, "lose");
    }
    // 最终【杀】：询问是否发动（贾诩）。
    assert_eq!(pend_skill(&g).as_deref(), Some("luanwu"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "target:1");
    assert!(g.seats[1].hp < 4 || g.seats[2].hp < 4);
}

#[test]
fn luanwu_once_per_game() {
    let mut g = game(&[("jiaxu", &["luanwu"], 3, "qun"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    g.seats[0].flags.insert("luanwu_used".to_string());
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:luanwu"));
}

// ---------------------------------------------------------------- 完杀
#[test]
fn wansha_blocks_others_tao_and_nonlocked_skills() {
    let mut g = game(&[
        ("jiaxu", &["wansha"], 3, "qun"),
        ("p1", &[], 1, "wei"),
        ("p2", &["jijiu"], 4, "wu"),
    ]);
    g.turn = 0;
    give(&mut g, 1, &["j2"]); // 濒死者可用【酒】自救 → 会停在濒死询问
    give(&mut g, 2, &["t1"]); // 其他角色（被完杀封锁）
    give(&mut g, 0, &["t2"]); // 贾诩本人可以救
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_some(), "濒死询问");
    // 濒死中其他角色非锁定技失效。
    assert!(!skills::active(&g, 2, "jijiu"), "濒死中他人非锁定技失效");
    // 濒死者本人可用【酒】自救。
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("respond:j2")));
    // 放弃自救 → 其他角色被封锁 → 轮到贾诩（可用【桃】）。
    answer_cur(&mut g, "pass");
    assert_eq!(pend_actor(&g), 0, "完杀：其他角色被跳过，轮到贾诩");
    let opts = options(&g);
    assert!(
        opts.iter().any(|o| o.starts_with("respond:t2")),
        "贾诩本人可用【桃】救"
    );
    // 贾诩救起 → 濒死结束，非锁定技恢复。
    answer(&mut g, 0, "respond:t2");
    assert_eq!(g.seats[1].hp, 1);
    assert!(skills::active(&g, 2, "jijiu"), "濒死结束后恢复");
}

#[test]
fn wansha_not_active_outside_own_turn() {
    let mut g = game(&[("jiaxu", &["wansha"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    assert!(skills::active(&g, 0, "wansha"));
    assert!(!skills::dying_peach_blocked(&g, 1, 0));
}

// ---------------------------------------------------------------- 好施
#[test]
fn haoshi_draws_two_more_and_gives_half() {
    let mut g = game(&[
        ("lusu", &["haoshi"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    g.turn = 0;
    give(&mut g, 0, &["d1", "d2", "d3", "h2", "c2"]);
    give(&mut g, 1, &["x30"]);
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("haoshi"));
    answer(&mut g, 0, "opt1");
    sanguosha2::turn::tick(&mut g).unwrap();
    // 摸牌阶段结束：手牌 >5 → 交一半。
    if pend_skill(&g).as_deref() == Some("haoshi") {
        let n = g.hand(0).len() / 2;
        for _ in 0..n {
            let opt = options(&g)
                .into_iter()
                .find(|o| o.starts_with("pick:"))
                .expect("选牌");
            answer(&mut g, 0, &opt);
        }
        if pend_skill(&g).as_deref() == Some("haoshi") {
            let opt = options(&g)
                .into_iter()
                .find(|o| o.starts_with("target:"))
                .expect("选交牌目标");
            answer(&mut g, 0, &opt);
        }
    }
    assert!(g.counter("haoshi_back:0") > 0, "记录回礼者");
}

#[test]
fn haoshi_no_give_when_five_or_less() {
    let mut g = game(&[("lusu", &["haoshi"], 3, "wu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    if pend_skill(&g).as_deref() == Some("haoshi") {
        answer(&mut g, 0, "opt1");
    }
    sanguosha2::turn::tick(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("haoshi"), "手牌 ≤5 不交牌");
}

// ---------------------------------------------------------------- 缔盟
#[test]
fn dimeng_swaps_hands_and_pays_at_play_end() {
    let mut g = game(&[
        ("lusu", &["dimeng"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1", "d2", "d3"]);
    give(&mut g, 1, &["x30", "x31"]);
    give(&mut g, 2, &["x32"]);
    skills::execute_active(&mut g, 0, "skill:dimeng").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "target:2");
    assert_eq!(g.hand(1).len(), 1);
    assert_eq!(g.hand(2).len(), 2);
    assert_eq!(g.counter("turn.dimeng_discard"), 1, "差 1 张，阶段结束弃");
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("dimeng"));
    let opt = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .expect("弃牌选项");
    answer(&mut g, 0, &opt);
    assert_eq!(g.counter("turn.dimeng_discard"), 0);
}

#[test]
fn dimeng_excludes_unaffordable_second_target() {
    let mut g = game(&[
        ("lusu", &["dimeng"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 1, &["x30", "x31", "x32", "x33"]);
    give(&mut g, 2, &[]);
    skills::execute_active(&mut g, 0, "skill:dimeng").unwrap();
    answer(&mut g, 0, "target:1");
    assert!(
        !options(&g).iter().any(|o| o == "target:2"),
        "手牌差 4 > 自己手牌 0 → 不可选"
    );
}

// ---------------------------------------------------------------- 巧变
#[test]
fn qiaobian_starts_with_two_marks() {
    let g = game(&[("zhanghe", &["qiaobian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    assert_eq!(g.seats[0].counters.get("bian").copied(), Some(2));
}

#[test]
fn qiaobian_asks_at_phase_start_and_skips() {
    let mut g = game(&[("zhanghe", &["qiaobian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.phase = Phase::Draw;
    g.turn = 0;
    give(&mut g, 0, &["d1"]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiaobian"));
    answer(&mut g, 0, "opt1");
    // 支付：弃一张牌。
    let opt = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:d1"))
        .expect("支付选项");
    answer(&mut g, 0, &opt);
    assert_eq!(g.counter("turn.skip_play"), 1, "跳过出牌阶段");
}

#[test]
fn qiaobian_no_ask_without_payment() {
    let mut g = game(&[("zhanghe", &["qiaobian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.phase = Phase::Draw;
    g.turn = 0;
    g.seats[0].counters.remove("bian"); // 无牌、无装备、无「变」
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("qiaobian"), "无牌无「变」不询问");
}

#[test]
fn qiaobian_skip_judge_keeps_judge_zone() {
    let mut g = game(&[("zhanghe", &["qiaobian"], 4, "wei"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    g.to_judge(&"le1".to_string(), 0).unwrap();
    give(&mut g, 0, &["d1"]);
    begin_phase(&mut g, 0, Phase::Judge).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiaobian"));
    answer(&mut g, 0, "opt1");
    let opt = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:d1"))
        .expect("支付选项");
    answer(&mut g, 0, &opt);
    assert_eq!(g.counter("turn.skip_judge"), 1, "跳过判定阶段");
    sanguosha2::turn::tick(&mut g).unwrap();
    assert!(
        g.judge_zone(0).iter().any(|c| c == "le1"),
        "跳过判定：判定区的牌保留（不结算、不弃置）"
    );
    // 判定阶段被跳过 → 立刻进入摸牌阶段的询问（可再放弃跳摸牌）。
    assert_eq!(pend_skill(&g).as_deref(), Some("qiaobian"));
    answer(&mut g, 0, "pass");
    sanguosha2::turn::tick(&mut g).unwrap();
    assert_ne!(g.phase, Phase::Judge, "跳过判定后进入摸牌阶段");
    assert!(
        g.judge_zone(0).iter().any(|c| c == "le1"),
        "摸牌阶段时判定区的牌仍在"
    );
}

// ---------------------------------------------------------------- 机制锚点
#[test]
fn b15_mechanism_anchors() {
    // 拼点内核（酣战/天义/烈刃）、距离、次数、目标数、标记、虚拟牌、失去牌事件。
    let mut g = game(&[("tsc", &["tianyi"], 4, "wu"), ("p1", &[], 4, "wei")]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    sanguosha2::kernel::pindian::start(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 1, "choose:d1");
    assert!(g.pindian_result.is_some(), "拼点结果通道");
    assert!(g.distance(0, 1) >= 1, "距离入口");
    assert!(g.sha_limit(0) >= 1, "【杀】次数入口");
    assert_eq!(g.hand(0).len(), 0);
    g.to_mark(&"x30".to_string(), 0, "tian").unwrap();
    assert_eq!(g.mark_cards(0, "tian").len(), 1);
    g.to_discard(&"x31".to_string(), "test").unwrap();
    g.check_conservation().unwrap();
}

// ---------------------------------------------------------------- 急袭（凿险授予）
#[test]
fn zaoxian_grants_jixi_and_equipment_becomes_sha() {
    // 邓艾：屯田 3 张「田」→ 准备阶段觉醒，减 1 体力上限并获得「急袭」。
    let mut g = game(&[("dengai", &["zaoxian"], 4, "wei"), ("p1", &[], 4, "qun")]);
    for code in ["t1", "t2", "j1"] {
        g.to_mark(&code.to_string(), 0, "tian").unwrap();
    }
    g.phase = Phase::Prepare;
    g.turn = 0;
    g.events
        .push(sanguosha2::kernel::event::Event::simple(
            EventKind::PhaseStart,
            Some(0),
            Some("Prepare"),
        ));
    advance(&mut g).unwrap();
    assert!(
        g.seats[0].granted.iter().any(|c| c == "jixi"),
        "凿险应授予「急袭」"
    );
    // 装备区的武器可以当【杀】使用（legal 与 execute 同源）。
    g.to_equip(&"w1".to_string(), 0, Slot::Weapon).unwrap();
    let conv = skills::conversions(&g, 0);
    assert!(
        conv.iter().any(|c| c.skill == "jixi" && c.to == "杀" && c.from == "w1"),
        "急袭应把装备区的武器视为【杀】：{}", conv.iter().map(|c| format!("{}:{}->{}", c.skill, c.from, c.to)).collect::<Vec<_>>().join(",")
    );
    play_phase(&mut g, 0);
    let before = g.seats[1].hp;
    g.use_as(0, &"w1".to_string(), "杀", vec![1], false, None)
        .expect("急袭：装备当杀");
    advance(&mut g).unwrap();
    while g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[1].hp, before - 1, "急袭【杀】应造成伤害");
}

#[test]
fn jixi_is_registered_and_grant_only() {
    // 注册表必须包含 jixi（否则授予后 handler 缺失 → 静默失效）。
    assert!(skills::def("jixi").is_some(), "jixi 必须在 v2 注册表");
    // 未授予时不应提供转化。
    let g = game(&[("p0", &[], 4, "wei"), ("p1", &[], 4, "qun")]);
    assert!(skills::conversions(&g, 0).is_empty());
}
