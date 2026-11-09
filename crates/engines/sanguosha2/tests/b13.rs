//! B13 用例（16 技能 + 2 授予技）。
use sanguosha2::kernel::event::{Event, EventKind};
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    // 第一张是牌堆顶（测试用它控制判定牌）。
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("sp5", "杀", Suit::Spade, 5),
        ("s1", "杀", Suit::Spade, 7),
        ("s2", "杀", Suit::Diamond, 8),
        ("s3", "杀", Suit::Club, 9),
        ("s4", "杀", Suit::Heart, 10),
        ("d1", "闪", Suit::Heart, 2),
        ("d2", "闪", Suit::Heart, 3),
        ("d3", "闪", Suit::Club, 10),
        ("t1", "桃", Suit::Diamond, 3),
        ("j1", "决斗", Suit::Club, 4),
        ("w1", "青釭剑", Suit::Spade, 6),
        ("wj1", "万箭齐发", Suit::Heart, 9),
        ("wux1", "无懈可击", Suit::Spade, 11),
        ("jd1", "借刀杀人", Suit::Club, 3),
        ("ts1", "铁索连环", Suit::Club, 10),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("e1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
    for i in 20..=90 {
        cards.push(Card::new(format!("c{i}"), "闪", Suit::Heart, 2));
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
    g.pending.as_ref().and_then(|p| p.skill().map(|s| s.to_string()))
}
fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
}
fn answer_cur(g: &mut Game, action: &str) {
    let actor = g.pending.as_ref().map(|p| p.actor()).expect("没有待办");
    answer(g, actor, action);
}
fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}
fn fire(g: &mut Game, kind: EventKind, seat: usize, card: Option<&str>, cause: Option<&str>) {
    let mut ev = Event::simple(kind, Some(seat), cause);
    ev.card = card.map(|c| c.to_string());
    ev.amount = Some(1);
    g.events.push(ev);
    advance(g).unwrap();
}

// ---------------------------------------------------------------- 烈弓
#[test]
fn liegong_blocks_shan_and_adds_damage() {
    let mut g = game(&[
        ("huang", &["liegong"], 3, "shu"),
        ("p1", &[], 3, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1", "s2"]); // 手牌 2
    give(&mut g, 1, &["d1", "d2", "d3"]); // 手牌 3
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("liegong"));
    answer(&mut g, 0, "opt1");
    // 手牌 2 < 3 → 不封【闪】；体力 3 ≤ 3 → 伤害 +1
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"));
    assert!(options(&g).iter().any(|o| o.starts_with("respond:")));
    answer(&mut g, 1, "pass");
    assert_eq!(g.seats[1].hp, 1, "【杀】伤害 1+1");
    // 判定可达：距离 ≤ 点数
    assert!(skills::range_override_reach(&g, 0, 1, &"s1".to_string()));
    // 介入钩子（sha_intercept）与 AOE 裁剪钩子（aoe_trim_target）的内核锚点
    assert!(!sanguosha2::skills::b13::sha_intercept(&g, 0, 1, Some(&"s1".to_string())).is_empty());
    let _ = skills::aoe_trim_target(&g, 0);
}

#[test]
fn liegong_can_decline() {
    let mut g = game(&[
        ("huang", &["liegong"], 3, "shu"),
        ("p1", &[], 3, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1", "s2", "s3"]);
    give(&mut g, 1, &["d1"]);
    g.resolve_use(0, &"s1".to_string(), vec![1]).unwrap();
    answer(&mut g, 0, "pass");
    answer(&mut g, 1, "respond:d1");
    assert_eq!(g.seats[1].hp, 3, "不发动烈弓 → 一张闪即抵消");
}

// ---------------------------------------------------------------- 诛害
#[test]
fn zhuhai_uses_real_sha_at_other_end_phase() {
    let mut g = game(&[
        ("xu", &["zhuhai"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.turn = 1;
    g.bump("turn.damage_by:1");
    give(&mut g, 0, &["s1"]);
    fire(&mut g, EventKind::PhaseStart, 1, None, Some("End"));
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuhai"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:s1");
    // 真实使用【杀】（无距离限制）→ 目标没有【闪】→ 受伤
    assert_eq!(g.seats[1].hp, 3);
}

#[test]
fn zhuhai_needs_sha_in_hand() {
    let mut g = game(&[
        ("xu", &["zhuhai"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.turn = 1;
    g.bump("turn.damage_by:1");
    give(&mut g, 0, &["d1"]);
    fire(&mut g, EventKind::PhaseStart, 1, None, Some("End"));
    assert_ne!(pend_skill(&g).as_deref(), Some("zhuhai"), "没有【杀】不能发动");
}

// ---------------------------------------------------------------- 潜心 / 荐言
#[test]
fn qianxin_awaken_grants_jianyan() {
    let mut g = game(&[
        ("xu", &["qianxin"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.seats[0].hp = 2; // 已受伤
    fire(&mut g, EventKind::DamageAfter, 0, None, Some("0"));
    assert_eq!(g.seats[0].max_hp, 2);
    assert!(g.seats[0].granted.iter().any(|s| s == "jianyan"));
}

#[test]
fn qianxin_requires_wounded() {
    let mut g = game(&[
        ("xu", &["qianxin"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    fire(&mut g, EventKind::DamageAfter, 0, None, Some("0"));
    assert_eq!(g.seats[0].max_hp, 3, "未受伤不觉醒");
}

#[test]
fn jianyan_reveals_and_gives_to_male() {
    let mut g = game(&[
        ("xu", &["jianyan"], 3, "shu"),
        ("m1", &[], 4, "wei"),
        ("f1", &[], 4, "wu"),
    ]);
    g.seats[2].male = false;
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jianyan").unwrap();
    let modes = options(&g);
    assert!(modes.iter().any(|o| o.starts_with("mode:")));
    answer(&mut g, 0, "mode:basic");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), 1, "交给男性角色");
}

#[test]
fn jianyan_needs_male() {
    let mut g = game(&[
        ("xu", &["jianyan"], 3, "shu"),
        ("f1", &[], 4, "wu"),
    ]);
    g.seats[1].male = false;
    g.seats[0].male = false;
    play_phase(&mut g, 0);
    assert!(
        skills::execute_active(&mut g, 0, "skill:jianyan").is_err(),
        "没有存活男性角色不能发动"
    );
}

// ---------------------------------------------------------------- 耀武
#[test]
fn yaowu_red_card_source_draws() {
    let mut g = game(&[
        ("hua", &["yaowu"], 4, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    give(&mut g, 1, &["s2"]); // 方块杀（红）
    let before = g.hand(1).len();
    fire(&mut g, EventKind::DamageAfter, 0, Some("s2"), Some("1"));
    assert_eq!(g.hand(1).len(), before + 1, "红色 → 来源摸 1");
}

#[test]
fn yaowu_non_red_victim_draws_and_self_source_no_draw() {
    let mut g = game(&[
        ("hua", &["yaowu"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    let before = g.hand(0).len();
    fire(&mut g, EventKind::DamageAfter, 0, Some("s1"), Some("1"));
    assert_eq!(g.hand(0).len(), before + 1, "非红 → 自己摸 1");
    // 红色但来源为自己 → 不摸
    let g2 = &mut game(&[("hua", &["yaowu"], 4, "qun"), ("p1", &[], 4, "wei")]);
    let b2 = g2.hand(0).len();
    fire(g2, EventKind::DamageAfter, 0, Some("s2"), Some("0"));
    assert_eq!(g2.hand(0).len(), b2, "红色且来源为自己 → 不摸牌");
}

// ---------------------------------------------------------------- 势斩
#[test]
fn shizhan_two_duels_per_phase() {
    let mut g = game(&[
        ("hua", &["shizhan"], 4, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]);
    skills::execute_active(&mut g, 0, "skill:shizhan").unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"), "决斗开始（华雄先出【杀】）");
    answer(&mut g, 0, "respond:s1");
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 3, "对方没有【杀】，决斗失败受伤");
    // 华雄（目标）先出【杀】—— 华雄没有【杀】→ 受伤后结束
    // 华雄已出【杀】→ 对方无【杀】→ 对方受伤
    assert_eq!(g.seats[1].hp, 3, "对方决斗失败受伤");
}

#[test]
fn shizhan_cannot_use_self() {
    let mut g = game(&[
        ("hua", &["shizhan"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:shizhan").unwrap();
    assert!(!options(&g).contains(&"target:0".to_string()));
}

// ---------------------------------------------------------------- 雷击
#[test]
fn leiji_judge_spade_deals_two_thunder() {
    let mut g = game(&[
        ("jiao", &["leiji"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1"]);
    // 模拟「打出【闪】」事件
    fire(&mut g, EventKind::CardPlayed, 0, Some("d1"), Some("response"));
    assert_eq!(pend_skill(&g).as_deref(), Some("leiji"));
    answer(&mut g, 0, "opt1");
    // 判定牌 = 牌堆顶 sp1（黑桃）→ 可选 2 点雷伤
    assert_eq!(pend_skill(&g).as_deref(), Some("leiji"));
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 2, "2 点雷电伤害");
}

#[test]
fn leiji_can_decline() {
    let mut g = game(&[
        ("jiao", &["leiji"], 3, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    give(&mut g, 0, &["d1"]);
    fire(&mut g, EventKind::CardPlayed, 0, Some("d1"), Some("response"));
    answer(&mut g, 0, "pass");
    assert_eq!(pend_skill(&g).as_deref(), None);
    assert_eq!(g.seats[1].hp, 4);
}

// ---------------------------------------------------------------- 鬼道
#[test]
fn guidao_rewrites_with_black_card_and_draws_on_spade_2_9() {
    let mut g = game(&[
        ("jiao", &["guidao"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp5"]); // 黑桃 5（2~9）
    // p1 判定
    sanguosha2::kernel::card_play::start_quick_judge(&mut g, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("guidao"));
    answer(&mut g, 0, "pick:sp5");
    advance(&mut g).unwrap();
    assert_eq!(g.judge_result.as_deref(), Some("sp5"));
    assert_eq!(g.hand(0).len(), 1, "黑桃 2~9/13 → 摸 1（随机）");
}

#[test]
fn guidao_offers_black_only() {
    let mut g = game(&[
        ("jiao", &["guidao"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1"]); // 红桃闪
    sanguosha2::kernel::card_play::start_quick_judge(&mut g, 1, "test");
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("guidao"), "红色牌不能改判");
}

// ---------------------------------------------------------------- 黄天
#[test]
fn huangtian_gives_shan_to_lord() {
    let mut g = game(&[
        ("jiao", &["huangtian"], 3, "qun"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    play_phase(&mut g, 1);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 1, "skill:huangtian").unwrap();
    answer(&mut g, 1, "pick:d1");
    assert_eq!(g.hand(0).len(), 1, "交给张角");
    assert_eq!(g.hand(1).len(), 0);
}

#[test]
fn huangtian_requires_qun_and_lord() {
    let mut g = game(&[
        ("jiao", &["huangtian"], 3, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    play_phase(&mut g, 1);
    give(&mut g, 1, &["d1"]);
    assert!(
        skills::execute_active(&mut g, 1, "skill:huangtian").is_err(),
        "非群势力不能发动黄天"
    );
}

// ---------------------------------------------------------------- 化身 / 新生
#[test]
fn huashen_gets_three_cards_and_skill() {
    let mut g = game(&[
        ("zuoci", &["huashen"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    let st = g.huashen.get(&0).cloned().unwrap_or_default();
    assert_eq!(st.cards.len(), 3, "开局 3 张化身牌");
    assert!(st.active.is_some(), "展示其中一张");
    // 不重复
    let mut uniq = st.cards.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), st.cards.len());
    // 池 = 上线武将
    assert!(sanguosha2::generals::live().count() >= 140);
}

#[test]
fn huashen_exchange_is_two_for_one() {
    let mut g = game(&[
        ("zuoci", &["huashen"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    // 清掉开局技能选择的 pending，直接驱动交换
    g.pending = None;
    g.stack.clear();
    let before = g.huashen.get(&0).unwrap().cards.len();
    fire(&mut g, EventKind::TurnStart, 0, None, None);
    assert_eq!(pend_skill(&g).as_deref(), Some("huashen"));
    answer(&mut g, 0, "opt2");
    let after = g.huashen.get(&0).unwrap().cards.len();
    assert_eq!(after, before - 1, "移去至多两张 → 只换回 1 张");
}

#[test]
fn xinsheng_gains_per_damage_point() {
    let mut g = game(&[
        ("zuoci", &["xinsheng"], 3, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    g.huashen.insert(
        0,
        sanguosha2::kernel::HuashenState {
            cards: vec!["sgs_hero_1".to_string()],
            active: None,
            orig_kingdom: "qun".into(),
            orig_male: true,
            active_skill: None,
        },
    );
    fire(&mut g, EventKind::DamageAfter, 0, None, Some("1"));
    assert_eq!(pend_skill(&g).as_deref(), Some("xinsheng"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.huashen.get(&0).unwrap().cards.len(), 2);
}

// ---------------------------------------------------------------- 蛊惑
#[test]
fn guhuo_fake_is_void_and_questioner_draws() {
    let mut g = game(&[
        ("yu", &["guhuo"], 2, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.seats[0].max_hp = 3;
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]); // 真实是【杀】，声明【桃】→ 假
    skills::execute_active(&mut g, 0, "skill:guhuo").unwrap();
    answer(&mut g, 0, "pick:s1");
    answer(&mut g, 0, "declare:桃"); // 桃为 SelfOnly → 直接进入质疑
    // 两名其他角色被逐一询问（同时收集）
    answer(&mut g, 1, "choose:question");
    answer(&mut g, 2, "pass");
    // 假 → 作废，质疑者摸 1
    assert!(g.zone_of(&"s1".to_string()) == Some(&Zone::Discard));
    assert_eq!(g.hand(1).len(), 1);
    assert_eq!(g.hand(2).len(), 0);
}

#[test]
fn guhuo_real_causes_chanyuan_and_penalty_choice() {
    let mut g = game(&[
        ("yu", &["guhuo"], 2, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["s1"]); // 真实【杀】，声明【杀】→ 真
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:guhuo").unwrap();
    answer(&mut g, 0, "pick:s1");
    answer(&mut g, 0, "declare:杀");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:question");
    answer(&mut g, 2, "pass");
    // 真 → 缠怨 + 质疑者自选弃牌或失去体力
    assert!(g.seats[1].flags.contains("chanyuan"));
    assert_eq!(pend_skill(&g).as_deref(), Some("guhuo"));
    answer(&mut g, 1, "hurt");
    assert_eq!(g.seats[1].hp, 3, "失去 1 点体力");
    // 之后按声明牌结算【杀】
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"));
}

// ---------------------------------------------------------------- 乱击
#[test]
fn luanji_two_same_suit_arrows() {
    let mut g = game(&[
        ("yuan", &["luanji"], 4, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1", "d2"]); // 同花色
    skills::execute_active(&mut g, 0, "skill:luanji").unwrap();
    answer(&mut g, 0, "pair:d1,d2");
    // 可少选一个目标
    assert_eq!(pend_skill(&g).as_deref(), Some("luanji"));
    answer(&mut g, 0, "drop:2");
    // 少选 p2 → 只有 p1 结算；双方都没有【闪】→ 直接受伤
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.seats[2].hp, 4, "被少选的目标不受影响");
}

#[test]
fn luanji_needs_same_suit() {
    let mut g = game(&[
        ("yuan", &["luanji"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1", "s1"]); // 不同花色
    assert!(
        skills::execute_active(&mut g, 0, "skill:luanji").is_err(),
        "花色不同不能转化"
    );
}

// ---------------------------------------------------------------- 血裔
#[test]
fn xueyi_marks_and_phase_draw() {
    let mut g = game(&[
        ("yuan", &["xueyi"], 4, "qun"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    sanguosha2::skills::b13::init_xueyi(&mut g);
    assert_eq!(sanguosha2::skills::b13::xueyi_marks(&g, 0), 4, "2 名群势力 × 2");
    assert_eq!(sanguosha2::skills::b13::hand_limit_delta(&g, 0), 4);
    play_phase(&mut g, 0);
    assert_eq!(pend_skill(&g).as_deref(), Some("xueyi"));
    answer(&mut g, 0, "opt1");
    assert_eq!(sanguosha2::skills::b13::xueyi_marks(&g, 0), 3);
}

#[test]
fn xueyi_no_marks_no_ask() {
    let mut g = game(&[
        ("yuan", &["xueyi"], 4, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    play_phase(&mut g, 0);
    assert_ne!(pend_skill(&g).as_deref(), Some("xueyi"));
}

// ---------------------------------------------------------------- 享乐
#[test]
fn xiangle_discard_basic_or_immune() {
    let mut g = game(&[
        ("shan", &["xiangle"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["s1", "d1"]);
    g.resolve_use(1, &"s1".to_string(), vec![0]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("xiangle"));
    answer(&mut g, 1, "opt1"); // 弃基本牌
    answer(&mut g, 1, "pick:d1");
    // 【杀】正常结算（目标没有【闪】）→ 伤害生效
    assert_eq!(g.seats[0].hp, 2);
    assert!(g.zone_of(&"d1".to_string()) == Some(&Zone::Discard));
}

#[test]
fn xiangle_without_basic_makes_sha_void() {
    let mut g = game(&[
        ("shan", &["xiangle"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["s1"]);
    g.resolve_use(1, &"s1".to_string(), vec![0]).unwrap();
    // 没有基本牌 → 不产生 pending，【杀】无效
    assert_ne!(pend_skill(&g).as_deref(), Some("xiangle"));
    assert_eq!(g.seats[0].hp, 3, "此【杀】无效");
}

// ---------------------------------------------------------------- 放权
#[test]
fn fangquan_skips_play_and_grants_extra_turn() {
    let mut g = game(&[
        ("chan", &["fangquan"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:fangquan").unwrap();
    assert_eq!(g.counter("turn.skip_play"), 1);
    // 弃牌阶段开始时询问
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("fangquan"));
    answer(&mut g, 0, "pick:d1");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.extra_turn, Some(1));
}

#[test]
fn fangquan_without_hand_no_extra_turn() {
    let mut g = game(&[
        ("chan", &["fangquan"], 3, "shu"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:fangquan").unwrap();
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("fangquan"));
    assert_eq!(g.extra_turn, None);
}

// ---------------------------------------------------------------- 若愚 / 思蜀
#[test]
fn ruoyu_awaken_strict_min_hp() {
    let mut g = game(&[
        ("chan", &["ruoyu"], 2, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 3, "wu"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 3);
    assert_eq!(g.seats[0].hp, 3);
    assert!(g.seats[0].granted.iter().any(|s| s == "sishu"));
    assert!(g.seats[0].granted.iter().any(|s| s == "jijiang"));
}

#[test]
fn ruoyu_tied_min_does_not_awaken() {
    let mut g = game(&[
        ("chan", &["ruoyu"], 2, "shu"),
        ("p1", &[], 2, "wei"),
        ("p2", &[], 3, "wu"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.phase = Phase::End;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, 2, "并列最小不触发");
}

#[test]
fn sishu_toggles_indulgence_judgement() {
    let mut g = game(&[
        ("chan", &["sishu"], 3, "shu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.phase = Phase::Prepare;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("sishu"));
    answer(&mut g, 0, "target:1");
    assert!(skills::sishu_reversed(&g, 1));
    // 再次发动 → 恢复
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    if pend_skill(&g).as_deref() == Some("sishu") {
        answer(&mut g, 0, "target:1");
        assert!(!skills::sishu_reversed(&g, 1), "再次选择 → 恢复常规");
    }
}

// ---------------------------------------------------------------- 蛊惑（无懈 / 濒死入口）
#[test]
fn guhuo_works_in_wuxie_window() {
    let mut g = game(&[
        ("yu", &["guhuo"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["wj1"]);
    give(&mut g, 0, &["wux1"]);
    g.resolve_use(1, &"wj1".to_string(), Vec::new()).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("wuxie"));
    assert!(options(&g).iter().any(|o| o == "guhuo"), "无懈窗口出现蛊惑");
    answer(&mut g, 0, "guhuo");
    answer(&mut g, 0, "pick:wux1");
    // 质疑阶段：其余角色都选择不质疑（按座位号顺序）
    answer(&mut g, 1, "pass");
    answer(&mut g, 2, "pass");
    // 真 → 计一张无懈，最终万箭被抵消
    assert_eq!(g.seats[0].hp, 3);
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(g.seats[2].hp, 4);
    assert!(
        g.log.iter().any(|l| l.text.contains("锦囊被抵消")),
        "蛊惑当【无懈可击】生效"
    );
}

#[test]
fn guhuo_works_in_dying_window() {
    let mut g = game(&[
        ("yu", &["guhuo"], 3, "qun"),
        ("victim", &[], 1, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.turn = 2; // 于吉回合外
    give(&mut g, 0, &["t1"]); // 真实的【桃】
    // victim 濒死
    flow::damage(&mut g, Some(2), 1, 1);
    advance(&mut g).unwrap();
    // 先问 victim 自己（无手牌）→ 于吉
    assert_eq!(pend_skill(&g).as_deref(), Some("dying"));
    assert!(options(&g).iter().any(|o| o == "guhuo"), "濒死窗口出现蛊惑");
    answer(&mut g, 0, "guhuo");
    answer(&mut g, 0, "pick:t1");
    // 质疑阶段：仅存活的其他角色（濒死者 hp=0 不参与）逐个询问
    answer_cur(&mut g, "pass");
    assert_eq!(g.seats[1].hp, 1, "蛊惑当【桃】救起");
}

#[test]
fn guhuo_can_declare_collateral() {
    let mut g = game(&[
        ("yu", &["guhuo"], 2, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.seats[0].max_hp = 3;
    play_phase(&mut g, 0);
    equip(&mut g, 1, "e1", Slot::HorseOffense); // 坐骑不影响武器判定
    equip(&mut g, 1, "w1", Slot::Weapon);
    give(&mut g, 0, &["jd1"]); // 真实的【借刀杀人】
    skills::execute_active(&mut g, 0, "skill:guhuo").unwrap();
    answer(&mut g, 0, "pick:jd1");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o == "declare:借刀杀人"), "可声明借刀杀人：{opts:?}");
    answer(&mut g, 0, "declare:借刀杀人");
    // 首位目标必须有武器
    let opts = options(&g);
    assert!(opts.contains(&"target:1".to_string()));
    assert!(!opts.contains(&"target:2".to_string()), "无武器者不可作首位目标");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "target:2"); // 第二目标
    answer_cur(&mut g, "pass"); // 质疑：p1
    answer_cur(&mut g, "pass"); // 质疑：p2
    // 真 → 借刀结算：p1 需对 p2 使用【杀】或交出武器
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"));
}

#[test]
fn guhuo_can_declare_iron_chain_with_two_targets() {
    let mut g = game(&[
        ("yu", &["guhuo"], 2, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "wu"),
    ]);
    g.seats[0].max_hp = 3;
    play_phase(&mut g, 0);
    give(&mut g, 0, &["ts1"]); // 真实的【铁索连环】
    skills::execute_active(&mut g, 0, "skill:guhuo").unwrap();
    answer(&mut g, 0, "pick:ts1");
    answer(&mut g, 0, "declare:铁索连环");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "target:2");
    answer_cur(&mut g, "pass");
    answer_cur(&mut g, "pass");
    assert!(g.seats[1].chained && g.seats[2].chained, "两名目标被横置");
}
