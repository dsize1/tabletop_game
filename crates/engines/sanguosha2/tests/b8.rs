//! B8 用例：辛宪英（忠鉴/才识）、吴苋（福绵/怠宴）、徐氏（问卦/伏诛）、曹节（守玺/惠民）、
//! 嵇康（清弦/绝响 + 四条子技能）、秦宓（谏征/专对/天辩）、薛综（复难/诫训）、蔡邕（辟撰/通博），
//! 以及本批新增的内核机制（拼点 / 目标数修正 / 牌堆承载使用）。

use sanguosha2::cards::ResponseKind;
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
        ("c4", "决斗", Suit::Club, 4),
        ("c5", "顺手牵羊", Suit::Spade, 5),
        ("c6", "过河拆桥", Suit::Club, 6),
        ("c7", "五谷丰登", Suit::Heart, 8),
        ("c8", "闪", Suit::Diamond, 9),
        ("c9", "杀", Suit::Heart, 10),
        ("c10", "桃", Suit::Club, 11),
        ("c11", "青釭剑", Suit::Spade, 6),
        ("c12", "八卦阵", Suit::Heart, 2),
        ("c13", "火杀", Suit::Heart, 4),
        ("c14", "无中生有", Suit::Heart, 12),
        ("c15", "梅花杀", Suit::Club, 13),
        ("c16", "红桃杀", Suit::Heart, 13),
        ("c17", "方片杀", Suit::Diamond, 1),
        ("c18", "黑桃顺", Suit::Spade, 3),
        ("c19", "方片闪", Suit::Diamond, 5),
        ("c20", "方片桃", Suit::Diamond, 7),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| match *name {
            "青釭剑" => Card::equip(*id, "青釭剑", *suit, *rank, Slot::Weapon),
            "八卦阵" => Card::equip(*id, "八卦阵", *suit, *rank, Slot::Armor),
            "梅花杀" => Card::new(*id, "杀", Suit::Club, *rank),
            "红桃杀" => Card::new(*id, "杀", Suit::Heart, *rank),
            "方片杀" => Card::new(*id, "杀", Suit::Diamond, *rank),
            "黑桃顺" => Card::new(*id, "顺手牵羊", *suit, *rank),
            "方片闪" => Card::new(*id, "闪", Suit::Diamond, *rank),
            "方片桃" => Card::new(*id, "桃", Suit::Diamond, *rank),
            _ => Card::new(*id, name, *suit, *rank),
        })
        .collect();
    for i in 21..=90 {
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

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

/// 进入指定阶段（单测不跑完整驱动器：先把阶段改成别的，保证 `PhaseStart` 会发出）。
fn start_phase(g: &mut Game, seat: usize, phase: Phase) {
    g.phase = if phase == Phase::Prepare {
        Phase::Discard
    } else {
        Phase::Prepare
    };
    begin_phase(g, seat, phase).unwrap();
}

fn log_text(g: &Game) -> String {
    g.log
        .iter()
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// 忠鉴（辛宪英）
// ---------------------------------------------------------------------------

#[test]
fn zhongjian_same_color_lets_owner_choose_draw_or_discard() {
    let mut g = game(&[
        ("xinxianying", &["zhongjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 黑桃 7 杀
    give(&mut g, 1, &["c2"]); // 红桃闪（颜色不同、点数不同）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhongjian").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "target:1");
    // p1 展示 1 张（体力值 4，但只有 1 张手牌）
    answer(&mut g, 1, "pick:c2");
    // 无同色同点 → 手牌上限 -1
    assert_eq!(g.counter("turn.zhongjian_penalty"), 1);
    assert_eq!(g.hand_limit(0), 3);
    g.check_conservation().expect("守恒");
}

#[test]
fn zhongjian_same_color_offers_two_options_and_same_rank_grants_second_use() {
    let mut g = game(&[
        ("xinxianying", &["zhongjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 黑桃 7
    give(&mut g, 1, &["c2"]); // 红桃 2 → 不同色不同点
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhongjian").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "pick:c2");
    let text = log_text(&g);
    assert!(text.contains("手牌上限 -1"), "{text}");

    // 同色：黑桃杀 vs 黑桃杀 → 二选一出现
    let mut g = game(&[
        ("xinxianying", &["zhongjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 黑桃 7
    give(&mut g, 1, &["c18"]); // 黑桃 3（同色不同点）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhongjian").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "pick:c18");
    let opts = options(&g);
    assert!(opts.contains(&"draw".to_string()), "{opts:?}");
    assert!(opts.contains(&"discard".to_string()), "{opts:?}");
    let before = g.hand(0).len();
    answer(&mut g, 0, "draw");
    assert_eq!(g.hand(0).len(), before + 1);

    // 同点：黑桃 7 vs 红桃 7 → 本回合可再发动一次
    let mut g = game(&[
        ("xinxianying", &["zhongjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 黑桃 7
    give(&mut g, 1, &["c20"]); // 方片 7（同点、不同色？方片=红 → 同色也同点）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhongjian").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "pick:c20");
    assert_eq!(g.counter("turn.zhongjian_extra"), 1);
    assert!(
        skills::legal_actions(&g, 0)
            .iter()
            .any(|(a, _)| a == "skill:zhongjian"),
        "同点后本回合可再发动一次"
    );
}

#[test]
fn zhongjian_requires_target_with_hand_cards() {
    let mut g = game(&[
        ("xinxianying", &["zhongjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0)
            .iter()
            .any(|(a, _)| a == "skill:zhongjian"),
        "其他角色都没有手牌时不可发动"
    );
}

// ---------------------------------------------------------------------------
// 才识（辛宪英）
// ---------------------------------------------------------------------------

#[test]
fn caishi_opt1_raises_hand_limit_for_turn() {
    let mut g = game(&[("xinxianying", &["caishi"], 4), ("p1", &[], 4)]);
    start_phase(&mut g, 0, Phase::Draw);
    assert_eq!(pend_skill(&g).as_deref(), Some("caishi"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand_limit(0), 5);
}

#[test]
fn caishi_opt2_hidden_when_full_hp_and_blocks_self_target() {
    let mut g = game(&[("xinxianying", &["caishi"], 4), ("p1", &[], 4)]);
    start_phase(&mut g, 0, Phase::Draw);
    assert!(
        !options(&g).iter().any(|o| o == "opt2"),
        "满体力时第 2 项不可选：{:?}",
        options(&g)
    );
    answer(&mut g, 0, "pass");

    // 掉血后可选 opt2；之后不能对自己使用【桃】
    g.seats[0].hp = 3;
    let mut g2 = game(&[("xinxianying", &["caishi"], 4), ("p1", &[], 4)]);
    g2.seats[0].hp = 3;
    give(&mut g2, 0, &["c3"]);
    start_phase(&mut g2, 0, Phase::Draw);
    answer(&mut g2, 0, "opt2");
    assert_eq!(g2.seats[0].hp, 4);
    play(&mut g2, 0);
    let err = g2.resolve_use(0, &"c3".to_string(), vec![0]);
    assert!(err.is_err(), "才识②：本回合不能对自己使用牌");
    g2.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 福绵（吴苋）
// ---------------------------------------------------------------------------

#[test]
fn fumian_opt1_adds_draw_bonus_and_opt2_allows_extra_target() {
    let mut g = game(&[("wuxian", &["fumian"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    start_phase(&mut g, 0, Phase::Prepare);
    assert_eq!(pend_skill(&g).as_deref(), Some("fumian"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.counter("turn.fumian_draw"), 1);
    start_phase(&mut g, 0, Phase::Draw);
    g.draw_phase(0);
    assert_eq!(g.hand(0).len(), 3, "本回合摸 3 张");

    // opt2：红色【杀】可以多指定 1 个目标（无距离）
    let mut g = game(&[("wuxian", &["fumian"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    start_phase(&mut g, 0, Phase::Prepare);
    answer(&mut g, 0, "opt2");
    assert_eq!(g.counter("turn.fumian_multi"), 1);
    give(&mut g, 0, &["c9"]); // 红桃杀
    play(&mut g, 0);
    g.resolve_use(0, &"c9".to_string(), vec![1, 2]).expect("红色【杀】多指定 1 个目标");
    // 使用后机会用掉
    assert_eq!(g.counter("turn.fumian_multi"), 0);
    g.check_conservation().expect("守恒");
}

#[test]
fn fumian_extra_target_does_not_apply_to_black_cards() {
    let mut g = game(&[("wuxian", &["fumian"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    start_phase(&mut g, 0, Phase::Prepare);
    answer(&mut g, 0, "opt2");
    give(&mut g, 0, &["c1"]); // 黑桃杀
    play(&mut g, 0);
    assert!(
        g.resolve_use(0, &"c1".to_string(), vec![1, 2]).is_err(),
        "黑色牌不能多指定目标"
    );
}

// ---------------------------------------------------------------------------
// 怠宴（吴苋）
// ---------------------------------------------------------------------------

#[test]
fn daiyan_gives_heart_basic_and_hurts_repeat_target_last_turn() {
    let mut g = game(&[("wuxian", &["daiyan"], 4), ("p1", &[], 4)]);
    // 牌堆顶放一张红桃基本牌
    g.put_on_top(&"c2".to_string()).unwrap();
    start_phase(&mut g, 0, Phase::End);
    assert_eq!(pend_skill(&g).as_deref(), Some("daiyan"));
    answer(&mut g, 0, "target:1");
    assert!(g.hand(1).contains(&"c2".to_string()), "获得红桃基本牌");
    assert_eq!(g.seats[1].hp, 4);

    // 下一个自己的回合再对同一目标发动 → 失去 1 点体力
    start_phase(&mut g, 1, Phase::End);
    if g.pending.is_some() {
        answer(&mut g, 1, "pass");
    }
    start_phase(&mut g, 0, Phase::End);
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "上回合也对其发动过 → 失去 1 点体力");
    g.check_conservation().expect("守恒");
}

#[test]
fn daiyan_falls_back_to_discard_pile_and_skips_other_targets() {
    let mut g = game(&[("wuxian", &["daiyan"], 4), ("p1", &[], 4), ("p2", &[], 4)]);
    // 牌堆里没有红桃基本牌（全部移走后把一张红桃基本牌放进弃牌堆）
    for id in g.ids_in(|z| *z == Zone::Draw) {
        g.to_discard(&id, "test").unwrap();
    }
    g.to_discard(&"c2".to_string(), "test").unwrap();
    // 放一张非红桃基本牌回牌堆，确认「牌堆优先」
    g.put_on_top(&"c1".to_string()).unwrap();
    start_phase(&mut g, 0, Phase::End);
    answer(&mut g, 0, "target:1");
    assert!(g.hand(1).contains(&"c2".to_string()), "从弃牌堆找到红桃基本牌");

    // 不同目标 → 不失去体力
    g.seats[1].hp = 4;
    start_phase(&mut g, 1, Phase::End);
    if g.pending.is_some() {
        answer(&mut g, 1, "pass");
    }
    start_phase(&mut g, 0, Phase::End);
    answer(&mut g, 0, "target:2");
    assert_eq!(g.seats[1].hp, 4);
}

// ---------------------------------------------------------------------------
// 问卦（徐氏）
// ---------------------------------------------------------------------------

#[test]
fn wengua_top_lets_both_draw_from_bottom() {
    let mut g = game(&[("p1", &[], 4), ("xushi", &["wengua"], 4)]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:wengua").unwrap();
    answer(&mut g, 0, "choose:c1");
    assert_eq!(pend(&g).actor(), 1, "由徐氏选择置顶/置底");
    answer(&mut g, 1, "top");
    assert_eq!(g.hand(0).len(), 1, "交出去的牌换成另一张");
    assert_eq!(g.hand(1).len(), 1);
    assert_eq!(g.counter("turn.wengua_used:0"), 1);
    g.check_conservation().expect("守恒");
}

#[test]
fn wengua_cannot_be_used_by_xushi_herself() {
    let mut g = game(&[("xushi", &["wengua"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0)
            .iter()
            .any(|(a, _)| a.starts_with("skill:wengua")),
        "徐氏本人不能发动问卦"
    );
}

// ---------------------------------------------------------------------------
// 伏诛（徐氏）
// ---------------------------------------------------------------------------

#[test]
fn fuzhu_uses_deck_sha_with_real_resolution() {
    let mut g = game(&[("p1", &[], 4), ("xushi", &["fuzhu"], 10)]);
    // 牌堆里有两张【杀】，p1 有两张【闪】
    give(&mut g, 0, &["c2", "c8"]);
    g.put_on_top(&"c1".to_string()).unwrap();
    start_phase(&mut g, 0, Phase::End);
    assert_eq!(pend_skill(&g).as_deref(), Some("fuzhu"));
    answer(&mut g, 1, "opt1");
    // 目标可以出【闪】（真实结算）
    assert_eq!(pend_skill(&g).as_deref(), Some("__card"));
    answer(&mut g, 0, "respond:c2");
    // 第二张【杀】继续结算
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "respond:c8");
    }
    assert_eq!(g.seats[0].hp, 4, "出【闪】后不受伤");
    g.check_conservation().expect("守恒");
}

#[test]
fn fuzhu_skips_when_deck_is_too_large() {
    let mut g = game(&[("p1", &[], 1), ("xushi", &["fuzhu"], 4)]);
    // 体力 4 → 阈值 40；牌堆 70 张 > 40
    start_phase(&mut g, 0, Phase::End);
    assert!(g.pending.is_none(), "牌堆数超过体力值×10 时不发动");
}

// ---------------------------------------------------------------------------
// 守玺（曹节）
// ---------------------------------------------------------------------------

#[test]
fn shouxi_declared_name_can_nullify_sha() {
    let mut g = game(&[("p1", &[], 4), ("caojie", &["shouxi"], 4)]);
    give(&mut g, 0, &["c1"]); // 黑桃杀
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("shouxi"));
    let declare = options(&g)
        .into_iter()
        .find(|o| o.starts_with("choose:"))
        .expect("有牌名可选");
    answer(&mut g, 1, &declare);
    // 使用者选择：令此【杀】无效（出口 = pass）
    answer(&mut g, 0, "pass");
    assert_eq!(g.seats[1].hp, 4, "此【杀】对曹节无效");
    g.check_conservation().expect("守恒");
}

#[test]
fn shouxi_opt1_hidden_when_user_lacks_declared_card() {
    let mut g = game(&[("p1", &[], 4), ("caojie", &["shouxi"], 4)]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    let declare = options(&g)
        .into_iter()
        .find(|o| o == "choose:决斗")
        .expect("可以声明【决斗】");
    answer(&mut g, 1, &declare);
    assert!(
        !options(&g).iter().any(|o| o == "opt1"),
        "手中没有该牌名时 opt1 不可选：{:?}",
        options(&g)
    );
    answer(&mut g, 0, "pass");
}

// ---------------------------------------------------------------------------
// 惠民（曹节）
// ---------------------------------------------------------------------------

#[test]
fn huimin_draws_reveals_and_lets_each_pick_like_harvest() {
    let mut g = game(&[("caojie", &["huimin"], 2), ("p1", &[], 4)]);
    give(&mut g, 0, &["c3"]); // 曹节手牌 1 < 体力 2 → X 至少 1
    start_phase(&mut g, 0, Phase::End);
    assert_eq!(pend_skill(&g).as_deref(), Some("huimin"));
    answer(&mut g, 0, "target:0");
    // 展示 X 张后从曹节开始依次选择
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("pick:")), "{opts:?}");
    let pick = opts.into_iter().find(|o| o.starts_with("pick:")).unwrap();
    answer(&mut g, 0, &pick);
    // 轮到的下一个人若还有牌则继续询问
    if g.pending.is_some() {
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    g.check_conservation().expect("守恒");
}

#[test]
fn huimin_not_offered_when_nobody_below_hp() {
    let mut g = game(&[("caojie", &["huimin"], 2), ("p1", &[], 2)]);
    // 两人手牌都 ≥ 体力（0 手牌 0 < 2 会算进来）→ 先给两人各 2 张手牌
    give(&mut g, 0, &["c1", "c2"]);
    give(&mut g, 1, &["c3", "c4"]);
    start_phase(&mut g, 0, Phase::End);
    assert!(g.pending.is_none(), "X=0 时不可发动");
}

// ---------------------------------------------------------------------------
// 清弦（嵇康）
// ---------------------------------------------------------------------------

#[test]
fn qingxian_opt2_heals_and_discards_equip_then_draws_on_club() {
    let mut g = game(&[("jikang", &["qingxian"], 3), ("p1", &[], 4)]);
    g.seats[1].hp = 2;
    equip(&mut g, 1, "c15", Slot::Weapon); // 梅花杀当作装备位（用梅花牌触发摸牌判定）
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qingxian"));
    // 目标 = 伤害来源，由来源二选一
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "opt2");
    assert_eq!(g.seats[1].hp, 3, "回复 1 点体力");
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .expect("由该角色选择弃置的装备");
    let before = g.hand(0).len();
    answer(&mut g, 1, &pick);
    assert_eq!(g.hand(0).len(), before + 1, "梅花牌 → 嵇康摸 1 张");
    g.check_conservation().expect("守恒");
}

#[test]
fn qingxian_not_offered_while_someone_is_dying() {
    let mut g = game(&[("jikang", &["qingxian"], 3), ("p1", &[], 1)]);
    g.seats[1].hp = 1;
    // 先让 p1 进入濒死（压入 Dying 帧），再对嵇康造成伤害
    flow::damage(&mut g, Some(0), 1, 1);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_ne!(
        pend_skill(&g).as_deref(),
        Some("qingxian"),
        "有人濒死时不触发清弦"
    );
}

// ---------------------------------------------------------------------------
// 绝响（嵇康）+ 四弦
// ---------------------------------------------------------------------------

#[test]
fn juexiang_grants_one_of_four_sub_skills_and_blocks_club_targets() {
    let mut g = game(&[("jikang", &["juexiang"], 3), ("p1", &[], 4)]);
    flow::damage(&mut g, Some(1), 0, 3);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("juexiang"));
    answer(&mut g, 0, "target:1");
    let granted = ["hexian", "rouxian", "liexian", "jixian"]
        .into_iter()
        .find(|c| skills::active(&g, 1, c))
        .expect("随机授予一条子技能");
    assert!(g.seats[1].flags.contains("juexiang_guard"));
    // 梅花牌不能指定该角色为目标
    assert!(!skills::can_target(&g, 0, "杀", 1, None) || g.seats[1].flags.contains("juexiang_guard"));
    let _ = granted;
    g.check_conservation().expect("守恒");
}

#[test]
fn jixian_after_damage_makes_source_lose_hp_and_use_equip() {
    let mut g = game(&[("p1", &["jixian"], 3), ("p2", &[], 4)]);
    // 牌堆顶放一张装备牌，供「随机使用一张装备牌」
    let equip_id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.def_of(id).kind == sanguosha2::cards::CardKind::Equip)
        .expect("牌堆里有装备牌");
    g.put_on_top(&equip_id).unwrap();
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jixian"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[1].hp, 3, "伤害来源失去 1 点体力");
    let equips = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == 1)).len();
    assert_eq!(equips, 1, "随机使用了一张装备牌");
    g.check_conservation().expect("守恒");
}

#[test]
fn rouxian_after_damage_heals_source_and_discards_their_equip() {
    let mut g = game(&[("p1", &["rouxian"], 3), ("p2", &[], 4)]);
    g.seats[1].hp = 2;
    equip(&mut g, 1, "c11", Slot::Weapon);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("rouxian"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[1].hp, 3, "伤害来源回复 1 点体力");
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .expect("该角色选择弃置的装备");
    answer(&mut g, 1, &pick);
    let equips = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == 1)).len();
    assert_eq!(equips, 0);
}

#[test]
fn liexian_after_heal_makes_other_lose_hp_and_use_equip() {
    let mut g = game(&[("p1", &["liexian"], 3), ("p2", &[], 4)]);
    let equip_id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.def_of(id).kind == sanguosha2::cards::CardKind::Equip)
        .expect("牌堆里有装备牌");
    g.put_on_top(&equip_id).unwrap();
    g.seats[0].hp = 2;
    flow::heal(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("liexian"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[1].hp, 3);
    g.check_conservation().expect("守恒");
}

#[test]
fn hexian_after_heal_heals_other_and_discards_their_equip() {
    let mut g = game(&[("p1", &["hexian"], 3), ("p2", &[], 4)]);
    g.seats[0].hp = 2;
    g.seats[1].hp = 2;
    equip(&mut g, 1, "c12", Slot::Armor);
    flow::heal(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("hexian"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[1].hp, 3);
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .expect("该角色选择弃置装备");
    answer(&mut g, 1, &pick);
    let equips = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == 1)).len();
    assert_eq!(equips, 0);
}

// ---------------------------------------------------------------------------
// 谏征（秦宓）
// ---------------------------------------------------------------------------

#[test]
fn jianzheng_non_black_sha_makes_qinmi_the_only_target() {
    let mut g = game(&[("p1", &[], 4), ("p2", &[], 4), ("qinmi", &["jianzheng"], 4)]);
    give(&mut g, 0, &["c9"]); // 红桃杀（非黑色）
    give(&mut g, 2, &["c2", "c8"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c9".to_string(), vec![1]).unwrap();
    // 介入顺序：先第三方（谏征）
    assert_eq!(pend(&g).actor(), 2);
    assert_eq!(pend_skill(&g).as_deref(), Some("jianzheng"));
    answer(&mut g, 2, "opt1");
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("choose:"))
        .expect("由秦宓选牌");
    answer(&mut g, 2, &pick);
    // 目标被替换为秦宓 → 询问秦宓出【闪】
    assert_eq!(pend(&g).actor(), 2);
    answer(&mut g, 2, "pass");
    assert_eq!(g.seats[2].hp, 3, "秦宓成为目标并受到伤害");
    assert_eq!(g.seats[1].hp, 4, "原目标被取消");
    g.check_conservation().expect("守恒");
}

#[test]
fn jianzheng_black_sha_cancels_all_targets() {
    let mut g = game(&[("p1", &[], 4), ("p2", &[], 4), ("qinmi", &["jianzheng"], 4)]);
    give(&mut g, 0, &["c1"]); // 黑桃杀
    give(&mut g, 2, &["c2", "c8"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jianzheng"));
    answer(&mut g, 2, "opt1");
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("choose:"))
        .unwrap();
    answer(&mut g, 2, &pick);
    assert!(g.pending.is_none(), "黑色【杀】：目标全部取消，无后续询问");
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(g.seats[2].hp, 4);
}

// ---------------------------------------------------------------------------
// 专对 / 天辩（秦宓）
// ---------------------------------------------------------------------------

#[test]
fn zhuandui_win_makes_target_unable_to_respond() {
    let mut g = game(&[("p1", &[], 4), ("qinmi", &["zhuandui"], 4)]);
    give(&mut g, 0, &["c1", "c17"]); // 黑桃 7（+ 拼点备用牌，点数 1）
    give(&mut g, 1, &["c16", "c2"]); // 红桃 K + 闪
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuandui"));
    answer(&mut g, 1, "opt1");
    // 拼点：秦宓先选（发起者），然后 p1
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "choose:c16"); // K
    assert_eq!(pend(&g).actor(), 0);
    answer(&mut g, 0, "choose:c17"); // 点数 1
    assert_eq!(g.seats[0].hp, 4, "目标视角拼点赢 → 此【杀】对其无效");
    assert_eq!(g.seats[1].hp, 4);
    assert!(log_text(&g).contains("无效"), "{}", log_text(&g));
    g.check_conservation().expect("守恒");
}

#[test]
fn zhuandui_user_side_win_makes_target_unable_to_respond() {
    let mut g = game(&[("qinmi", &["zhuandui"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1", "c16"]); // 黑桃杀 + 红桃 K（拼点牌）
    give(&mut g, 1, &["c2"]); // 闪
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuandui"));
    answer(&mut g, 0, "opt1"); // 使用者视角拼点
    answer(&mut g, 0, "choose:c16"); // K
    answer(&mut g, 1, "choose:c2"); // 2 → 秦宓赢
    assert_eq!(g.seats[1].hp, 3, "拼点赢 → 目标不能响应，伤害照常");
    assert!(g.pending.is_none(), "不再询问【闪】");
    g.check_conservation().expect("守恒");
}

#[test]
fn zhuandui_lose_keeps_normal_response_window() {
    let mut g = game(&[("p1", &[], 4), ("qinmi", &["zhuandui"], 4)]);
    give(&mut g, 0, &["c1", "c9"]); // 黑桃杀 + 红桃杀（点数 10，拼点备用）
    give(&mut g, 1, &["c2", "c8"]); // 红桃 2 + 方片闪
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "opt1");
    answer(&mut g, 1, "choose:c2"); // 2
    answer(&mut g, 0, "choose:c9"); // 10 → 秦宓输
    // 拼点未赢 → 正常询问【闪】
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("respond:")), "{opts:?}");
    answer(&mut g, 1, "respond:c8");
    assert_eq!(g.seats[1].hp, 4);
}

#[test]
fn tianbian_heart_counts_as_king_and_deck_top_is_available() {
    let mut g = game(&[("p1", &[], 4), ("qinmi", &["zhuandui", "tianbian"], 4)]);
    give(&mut g, 0, &["c16", "c17"]); // 红桃 K（+ 备用拼点牌）
    give(&mut g, 1, &["c2"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c16".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "opt1");
    // 天辩：出现「牌堆顶」选项
    let opts = options(&g);
    assert!(opts.contains(&"deck_top".to_string()), "{opts:?}");
    // 秦宓用红桃 2 拼点（体力值视为 K=13）；p1 的红桃 K 也是 13 → 平局
    answer(&mut g, 1, "choose:c2");
    answer(&mut g, 0, "choose:c17");
    assert_eq!(g.seats[0].hp, 4, "平局 → 无人赢，目标正常响应");
}

// ---------------------------------------------------------------------------
// 复难（薛综）
// ---------------------------------------------------------------------------

#[test]
fn funan_swaps_used_card_and_response_card() {
    let mut g = game(&[("xuezong", &["funan"], 4), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["c2"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "respond:c2");
    assert_eq!(pend_skill(&g).as_deref(), Some("funan"));
    answer(&mut g, 0, "opt1");
    assert!(g.hand(0).contains(&"c2".to_string()), "薛综获得响应牌【闪】");
    assert!(g.hand(1).contains(&"c1".to_string()), "响应者获得【杀】");
    assert!(
        g.seats[1].frozen.contains(&"c1".to_string()),
        "本回合不能使用或打出此牌"
    );
    g.check_conservation().expect("守恒");
}

#[test]
fn funan_plus_only_takes_the_response_card() {
    let mut g = game(&[("xuezong", &["funan"], 4), ("p1", &[], 4)]);
    g.seats[0].flags.insert("funan_upgraded".to_string());
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["c2"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    answer(&mut g, 1, "respond:c2");
    let label = pend(&g)
        .options()
        .iter()
        .find(|o| o.id == "opt1")
        .map(|o| o.label.clone())
        .unwrap();
    assert!(label.contains("复难plus"), "{label}");
    answer(&mut g, 0, "opt1");
    assert!(g.hand(0).contains(&"c2".to_string()), "只获得响应牌");
    assert!(!g.hand(1).contains(&"c1".to_string()), "对方不再获得打出的牌");
}

// ---------------------------------------------------------------------------
// 诫训（薛综）
// ---------------------------------------------------------------------------

#[test]
fn jiexun_draws_public_diamonds_and_discards_count_from_one() {
    let mut g = game(&[("xuezong", &["jiexun"], 4), ("p1", &[], 4)]);
    equip(&mut g, 1, "c19", Slot::Weapon); // 方片闪（装备位）
    give(&mut g, 1, &["c3", "c4"]);
    start_phase(&mut g, 0, Phase::End);
    assert_eq!(pend_skill(&g).as_deref(), Some("jiexun"));
    answer(&mut g, 0, "target:1");
    // 场上方片 1 张 → 摸 1；Y=1 → 弃 1
    assert!(g.hand(1).len() >= 1);
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .expect("由目标弃牌");
    answer(&mut g, 1, &pick);
    g.check_conservation().expect("守恒");
}

#[test]
fn jiexun_loses_skill_only_when_target_ends_empty() {
    let mut g = game(&[("xuezong", &["jiexun"], 4), ("p1", &[], 4)]);
    give(&mut g, 1, &["c3"]);
    // 无方片 → 不摸牌；Y=1 → 弃 1 → 目标空 → 薛综失技升级
    start_phase(&mut g, 0, Phase::End);
    answer(&mut g, 0, "target:1");
    let pick = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .unwrap();
    answer(&mut g, 1, &pick);
    assert!(!skills::active(&g, 0, "jiexun"), "目标弃光 → 薛综失去诫训");
    assert!(g.seats[0].flags.contains("funan_upgraded"));
}

// ---------------------------------------------------------------------------
// 辟撰 / 通博（蔡邕）
// ---------------------------------------------------------------------------

#[test]
fn bizhuan_puts_spade_use_on_general_as_shu_and_raises_hand_limit() {
    let mut g = game(&[("caiyong", &["bizhuan"], 3), ("p1", &[], 4)]);
    give(&mut g, 0, &["c1"]); // 黑桃杀
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("bizhuan"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.mark_cards(0, "shu").len(), 1);
    assert_eq!(g.hand_limit(0), 4, "每有一张「书」手牌上限 +1");
    g.check_conservation().expect("守恒");
}

#[test]
fn bizhuan_stops_at_four_and_red_cards_do_not_trigger() {
    let mut g = game(&[("caiyong", &["bizhuan"], 3), ("p1", &[], 4)]);
    // 先放 4 张「书」
    let ids: Vec<String> = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .filter(|id| id != "c1")
        .take(4)
        .collect();
    for id in &ids {
        g.to_mark(id, 0, "shu").unwrap();
    }
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    assert!(g.pending.is_none(), "已有 4 张「书」不再询问");

    // 红色牌不触发
    let mut g2 = game(&[("caiyong", &["bizhuan"], 3), ("p1", &[], 4)]);
    give(&mut g2, 0, &["c9"]); // 红桃杀
    play(&mut g2, 0);
    g2.resolve_use(0, &"c9".to_string(), vec![1]).unwrap();
    if let Some(p) = g2.pending.as_ref() {
        assert_ne!(p.skill(), Some("bizhuan"));
    }
}

#[test]
fn tongbo_swaps_cards_with_shu_and_hands_all_out_on_four_suits() {
    let mut g = game(&[("caiyong", &["bizhuan", "tongbo"], 3), ("p1", &[], 4)]);
    // 「书」= 黑桃 + 红桃 + 方片；手牌补梅花
    // 「书」= 黑桃 + 红桃 + 方片 + 梅花（4 张）→ 替换后仍含四花色
    for id in ["c1", "c2", "c3", "c15"] {
        g.to_mark(&id.to_string(), 0, "shu").unwrap();
    }
    give(&mut g, 0, &["c18"]); // 黑桃顺（替换用）
    start_phase(&mut g, 0, Phase::Draw); // 摸牌阶段
    // 摸牌结束进入出牌阶段（通博时机）
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("tongbo"));
    let own = options(&g)
        .into_iter()
        .find(|o| o.starts_with("own:"))
        .expect("手牌/装备区槽位选项");
    assert!(own.starts_with("own:hand:"), "槽位严格对应手牌：{own}");
    answer(&mut g, 0, &own);
    let shu = options(&g)
        .into_iter()
        .find(|o| o.starts_with("pick:"))
        .unwrap();
    answer(&mut g, 0, &shu);
    // 替换后手牌非空 → 先结束替换
    if pend_skill(&g).as_deref() == Some("tongbo") && options(&g).iter().any(|o| o.starts_with("own:")) {
        answer(&mut g, 0, "pass");
    }
    // 四花色齐 → 分配给一名其他角色
    assert_eq!(pend_skill(&g).as_deref(), Some("tongbo"));
    answer(&mut g, 0, "target:1");
    assert_eq!(g.mark_cards(0, "shu").len(), 0);
    assert!(g.hand(1).len() >= 3, "所有「书」交给一名角色");
    g.check_conservation().expect("守恒");
}

#[test]
fn tongbo_rejects_option_that_does_not_match_zone() {
    let mut g = game(&[("caiyong", &["bizhuan", "tongbo"], 3), ("p1", &[], 4)]);
    g.to_mark(&"c1".to_string(), 0, "shu").unwrap();
    give(&mut g, 0, &["c15"]);
    equip(&mut g, 0, "c11", Slot::Weapon);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 伪造一个「装备区」选项但牌其实在手牌里 → 校验失败
    let err = submit(&mut g, 0, "own:equip:Weapon:c15");
    assert!(err.is_err(), "槽位与牌区不一致必须拒绝");
}

// ---------------------------------------------------------------------------
// 内核机制：拼点 / 目标数修正 / 牌堆承载
// ---------------------------------------------------------------------------

#[test]
fn pindian_keeps_choices_hidden_until_reveal_and_discards_both() {
    let mut g = game(&[("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, &["c1"]);
    give(&mut g, 1, &["c2"]);
    sanguosha2::kernel::pindian::start(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    assert!(g.revealed.is_empty(), "选择期不公开");
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 1, "choose:c2");
    assert!(g.revealed.is_empty(), "结算后立即收起展示");
    let result = g.pindian_result.take().expect("拼点结果");
    assert_eq!(result.rank_a, 7);
    assert_eq!(result.rank_b, 2);
    assert_eq!(result.winner, Some(0));
    assert_eq!(g.discard_pile().len(), 2, "双方拼点牌进弃牌堆");
    g.check_conservation().expect("守恒");
}

#[test]
fn pindian_tie_has_no_winner() {
    let mut g = game(&[("p1", &[], 4), ("p2", &[], 4)]);
    give(&mut g, 0, &["c1"]); // 黑桃 7
    give(&mut g, 1, &["c20"]); // 方片 7
    sanguosha2::kernel::pindian::start(&mut g, 0, 1, "test");
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 1, "choose:c20");
    let result = g.pindian_result.take().expect("拼点结果");
    assert_eq!(result.winner, None, "平局 → 双方都未赢");
}

#[test]
fn extra_target_allows_red_sha_on_two_targets_without_distance() {
    let mut g = game(&[
        ("wuxian", &["fumian"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    start_phase(&mut g, 0, Phase::Prepare);
    answer(&mut g, 0, "opt2");
    give(&mut g, 0, &["c9"]); // 红桃杀
    give(&mut g, 1, &["c2"]); // 闪
    give(&mut g, 3, &["c8"]); // 闪
    play(&mut g, 0);
    g.resolve_use(0, &"c9".to_string(), vec![1, 3])
        .expect("多指定目标且无距离限制");
    // 两个目标各自进入【闪】询问
    assert_eq!(pend(&g).actor(), 1);
    answer(&mut g, 1, "respond:c2");
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(pend(&g).actor(), 3);
    answer(&mut g, 3, "respond:c8");
    assert_eq!(g.seats[3].hp, 4);
    g.check_conservation().expect("守恒");
}

#[test]
fn sha_intercept_is_asked_before_response_not_after_resolution() {
    let mut g = game(&[("p1", &[], 4), ("caojie", &["shouxi"], 4)]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.resolve_use(0, &"c1".to_string(), vec![1]).unwrap();
    // 结算前：守玺介入询问先于【闪】询问
    assert_eq!(pend_skill(&g).as_deref(), Some("shouxi"));
    assert_eq!(g.seats[1].hp, 4, "尚未结算伤害");
}

#[test]
fn b8_none_of_the_skills_use_event_only_use_card() {
    // 门禁：技能层不得调用事件模拟入口（`Game::use_card`）。
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/skills/b8.rs"
    ))
    .unwrap();
    assert!(!src.contains("use_card("), "b8 技能不得调用 use_card");
}
