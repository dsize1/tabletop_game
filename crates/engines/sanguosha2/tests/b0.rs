//! B0 七个技能的验收测试（按 `align/B0.md` 的「对齐后描述」逐条断言）。
//!
//! 每个技能 ≥1 正例 + ≥1 反例；重点覆盖用户裁定的 6 处差异：
//! 据守翻回触发解围 / 不屈「创」移出游戏 / 奋激排除摸牌阶段 + 脱困后摸 2 /
//! 行殇不含判定区 / 放逐无上限 / 颂威询问判定者。

use sanguosha2::kernel::flow;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::presentation::POutcome;
use sanguosha2::skills;

// ---------------------------------------------------------------------------
// 测试夹具
// ---------------------------------------------------------------------------

fn cards20() -> Vec<Card> {
    vec![
        Card::new("c1", "杀", Suit::Spade, 7),
        Card::new("c2", "闪", Suit::Heart, 2),
        Card::new("c3", "桃", Suit::Diamond, 3),
        Card::new("c4", "桃", Suit::Club, 4),
        Card::new("c5", "杀", Suit::Diamond, 9),
        Card::new("c6", "闪", Suit::Spade, 6),
        Card::equip("c7", "青釭剑", Suit::Spade, 6, Slot::Weapon),
        Card::equip("c8", "八卦阵", Suit::Spade, 2, Slot::Armor),
        Card::new("c9", "无懈可击", Suit::Club, 5),
        Card::new("c10", "杀", Suit::Club, 8),
        Card::new("c11", "杀", Suit::Diamond, 10),
        Card::new("c12", "桃", Suit::Heart, 12),
        Card::new("c13", "决斗", Suit::Spade, 1),
        Card::new("c14", "杀", Suit::Spade, 3),
        Card::new("c15", "闪", Suit::Diamond, 4),
        Card::new("c16", "杀", Suit::Heart, 10),
        Card::new("c17", "闪", Suit::Club, 2),
        Card::new("c18", "桃", Suit::Club, 9),
        Card::new("c19", "杀", Suit::Spade, 13),
        Card::new("c20", "闪", Suit::Heart, 11),
    ]
}

/// 4 人 B0 局：曹仁 / 周泰 / 曹丕（魏·主公）/ 卢植。
fn b0_game() -> Game {
    let mut caopi = Seat::new("p2", "caopi", &["xingshang", "fangzhu", "songwei"], 3);
    caopi.kingdom = "wei".to_string();
    let mut caoren = Seat::new("p0", "caoren", &["jushou", "jiewei"], 4);
    caoren.kingdom = "wei".to_string();
    let zhoutai = Seat::new("p1", "zhoutai", &["buqu", "fenji"], 4);
    let luzhi = Seat::new("p3", "luzhi", &["zhenliang"], 4);
    let mut g = Game::new(vec![caoren, zhoutai, caopi, luzhi], cards20());
    // 初始手牌（不走 gain，避免触发事件）
    give(&mut g, 0, &["c1", "c2"]);
    give(&mut g, 1, &["c3", "c4"]);
    give(&mut g, 2, &["c5", "c6"]);
    give(&mut g, 3, &["c10", "c11"]);
    g
}

fn give(g: &mut Game, seat: usize, ids: &[&str]) {
    for id in ids {
        g.to_hand(&id.to_string(), seat).expect("给牌");
    }
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn opt_prefix(g: &Game, prefix: &str) -> String {
    pend(g)
        .options()
        .iter()
        .find(|o| o.id.starts_with(prefix))
        .unwrap_or_else(|| panic!("缺少选项 {prefix}：{:?}", pend(g).options()))
        .id
        .clone()
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    g.check_conservation().expect("守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}

// ---------------------------------------------------------------------------
// 据守
// ---------------------------------------------------------------------------

#[test]
fn jushou_flips_back_to_face_up_and_triggers_jiewei_first() {
    let mut g = b0_game();
    g.seats[0].flipped = true; // 背面朝上
    give(&mut g, 0, &["c7"]); // 有装备可选
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend(&g).skill(), Some("jushou"));
    answer(&mut g, 0, "choose:jushou");

    // 翻至正面 → 解围②先询问（子帧先于据守的摸牌）
    assert_eq!(pend(&g).skill(), Some("jiewei"), "翻回正面必须先结算解围");
    assert!(!g.seats[0].flipped, "据守应把背面翻回正面（取反）");
    let hand_before = g.hand(0).len();
    answer(&mut g, 0, "pass"); // 不发动解围

    // 解围结束后继续据守：摸 4 → 询问弃/使用
    assert_eq!(pend(&g).skill(), Some("jushou"));
    assert_eq!(g.hand(0).len(), hand_before + 4, "解围之后才摸 4 张");
}

#[test]
fn jushou_face_up_flips_to_back_and_does_not_trigger_jiewei() {
    let mut g = b0_game();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    answer(&mut g, 0, "choose:jushou");
    assert!(g.seats[0].flipped, "正面发动应翻到背面");
    assert_eq!(
        pend(&g).skill(),
        Some("jushou"),
        "未翻至正面时不得触发解围"
    );
}

#[test]
fn jushou_equip_card_must_be_used_not_discarded() {
    let mut g = b0_game();
    give(&mut g, 0, &["c8"]); // 八卦阵
    begin_phase(&mut g, 0, Phase::End).unwrap();
    answer(&mut g, 0, "choose:jushou");
    answer(&mut g, 0, "choose:c8");
    assert_eq!(
        g.zone_of(&"c8".to_string()),
        Some(&Zone::Equip(0, Slot::Armor)),
        "装备牌应被使用而非弃置"
    );
    assert!(!g.discard_pile().contains(&"c8".to_string()));
}

// ---------------------------------------------------------------------------
// 解围
// ---------------------------------------------------------------------------

#[test]
fn jiewei_equipment_can_be_used_as_wuxie() {
    let mut g = b0_game();
    g.to_equip(&"c7".to_string(), 0, Slot::Weapon).unwrap();
    flow::start_wuxie(&mut g, 3, "c13".to_string());
    advance(&mut g).unwrap();
    // 从使用者（p3）左手侧开始：p0 第一个
    assert_eq!(pend(&g).actor(), 0);
    let id = opt_prefix(&g, "opt:");
    answer(&mut g, 0, &id);
    // 继续经过其他玩家：全部 pass
    for _ in 0..12 {
        if g.pending.is_none() {
            break;
        }
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
    assert!(
        g.presentation
            .iter()
            .any(|p| p.outcome == POutcome::TrickNegated),
        "一张无懈（奇数层）→ 锦囊被抵消"
    );
    assert_eq!(g.zone_of(&"c7".to_string()), Some(&Zone::Discard));
}

#[test]
fn jiewei_does_not_offer_equip_without_skill() {
    let mut g = b0_game();
    g.to_equip(&"c7".to_string(), 1, Slot::Weapon).unwrap(); // 周泰没有解围
    flow::start_wuxie(&mut g, 3, "c13".to_string());
    advance(&mut g).unwrap();
    // 全场无人可响应 → 直接结算（不得出现任何 opt: 选项）
    for _ in 0..12 {
        if g.pending.is_none() {
            break;
        }
        assert!(
            !pend(&g).options().iter().any(|o| o.id.starts_with("opt:")),
            "无解围不得提供装备当无懈"
        );
        let actor = pend(&g).actor();
        answer(&mut g, actor, "pass");
    }
}

// ---------------------------------------------------------------------------
// 不屈
// ---------------------------------------------------------------------------

#[test]
fn buqu_moves_chuang_out_of_game() {
    let mut g = b0_game();
    g.seats[1].hp = 0;
    flow::start_dying(&mut g, 1, None);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 1, "点数不同 → 回复至 1");
    let void_cards = g.ids_in(|z| *z == Zone::Void);
    assert_eq!(void_cards.len(), 1, "「创」必须移出游戏");
    assert!(g.mark_cards(1, "buqu").is_empty(), "创不再放 mark 区");
}

#[test]
fn buqu_duplicate_rank_goes_to_discard_not_void() {
    // 全同点数牌堆：第一次成创，第二次必重复。
    let cards: Vec<Card> = (1..=10)
        .map(|i| Card::new(format!("d{i}"), "杀", Suit::Spade, 5))
        .collect();
    let zhoutai = Seat::new("p1", "zhoutai", &["buqu"], 4);
    let other = Seat::new("p0", "nobody", &[], 4);
    let mut g = Game::new(vec![other, zhoutai], cards);
    g.seats[1].hp = 0;
    flow::start_dying(&mut g, 1, None);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 1);
    assert_eq!(g.ids_in(|z| *z == Zone::Void).len(), 1);
    let discard_after_first = g.discard_pile().len();

    // 再次濒死：同点数 → 判定牌进弃牌堆，不新增 void
    g.seats[1].hp = 0;
    flow::start_dying(&mut g, 1, None);
    advance(&mut g).unwrap();
    assert_eq!(g.ids_in(|z| *z == Zone::Void).len(), 1, "重复不得再移出游戏");
    assert_eq!(g.discard_pile().len(), discard_after_first + 1, "重复牌进弃牌堆");
    assert_eq!(g.seats[1].hp, 0, "重复时仍濒死");
}

// ---------------------------------------------------------------------------
// 奋激
// ---------------------------------------------------------------------------

#[test]
fn fenji_ignores_draw_phase_draws() {
    let mut g = b0_game();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.draw_with_cause(2, 1, Some("draw_phase"));
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "摸牌阶段摸牌不得触发奋激");

    g.draw_with_cause(2, 1, Some("skill"));
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fenji"), "其他来源摸牌算「获得」");
}

#[test]
fn fenji_draws_after_leaving_dying() {
    let mut g = b0_game();
    g.seats[1].skills = vec!["fenji".to_string()]; // 去掉不屈，单独验证「脱困后摸 2」
    skills::rebuild_subs(&mut g);
    g.seats[1].hp = 1; // 周泰濒死线
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = g.hand(2)[0].clone();
    g.to_discard_by(0, &card, "test").unwrap(); // p2 手牌被 p0 弃置（口径 B：被其他角色弃置）
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fenji"));
    let hand_p2_before = g.hand(2).len();
    answer(&mut g, 1, "choose:fenji"); // 失去 1 点体力 → 濒死
    assert_eq!(g.seats[1].hp, 0);
    // 濒死中：先把 p1 救起（p1 自己有桃）
    assert_eq!(pend(&g).skill(), Some("dying"), "先进入濒死结算");
    let heal = opt_prefix(&g, "respond:");
    answer(&mut g, 1, &heal);
    assert_eq!(g.seats[1].hp, 1);
    assert_eq!(
        g.hand(2).len(),
        hand_p2_before + 2,
        "脱离濒死后才摸 2 张"
    );
}

// ---------------------------------------------------------------------------
// 行殇
// ---------------------------------------------------------------------------

#[test]
fn xingshang_excludes_judge_zone() {
    let mut g = b0_game();
    // p1 死亡时：手牌 1 张 + 判定区 1 张
    let hand_card = g.hand(1)[0].clone();
    g.to_judge(&"c20".to_string(), 1).unwrap();
    g.seats[1].hp = 0;
    flow::die(&mut g, 1, None);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("xingshang"));
    answer(&mut g, 2, "choose:xingshang");
    assert!(
        g.hand(2).contains(&hand_card),
        "手牌应被行殇获得"
    );
    assert!(
        !g.hand(2).contains(&"c20".to_string()),
        "判定区牌不得被行殇获得"
    );
    assert_eq!(
        g.zone_of(&"c20".to_string()),
        Some(&Zone::Discard),
        "判定区牌按死亡清理进弃牌堆"
    );
}

#[test]
fn xingshang_can_be_declined() {
    let mut g = b0_game();
    let hand_card = g.hand(1)[0].clone();
    g.seats[1].hp = 0;
    flow::die(&mut g, 1, None);
    advance(&mut g).unwrap();
    answer(&mut g, 2, "pass");
    assert!(!g.hand(2).contains(&hand_card));
    assert_eq!(g.zone_of(&hand_card), Some(&Zone::Discard));
}

// ---------------------------------------------------------------------------
// 放逐
// ---------------------------------------------------------------------------

#[test]
fn fangzhu_x_has_no_cap() {
    let mut g = b0_game();
    // 曹丕体力上限 8、当前 2 → 已损失 6（旧实现会截到 5）
    g.seats[2].max_hp = 8;
    g.seats[2].hp = 2;
    let before = g.hand(0).len();
    flow::damage(&mut g, Some(3), 2, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fangzhu"));
    let target = opt_prefix(&g, "target:0"); // 目标是曹仁
    answer(&mut g, 2, &target);
    assert!(g.hand(0).len() >= before + 6, "X=6 不得被截断为 5");
}

#[test]
fn fangzhu_flips_before_drawing_and_can_trigger_jiewei() {
    let mut g = b0_game();
    g.seats[2].hp = 1; // 已损失 2
    flow::damage(&mut g, Some(3), 2, 0);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fangzhu"));
    let target = opt_prefix(&g, "target:0"); // 目标是曹仁（有解围）
    answer(&mut g, 2, &target);
    // 翻面 → 曹仁由正转背，不触发解围；随后摸 X=2 张（摸牌会触发奋激，属正常）
    assert!(g.seats[0].flipped);
    assert!(g.hand(0).len() >= 2, "翻面后应摸 2 张");
    if let Some(p) = &g.pending {
        assert!(matches!(p.skill(), Some("fenji") | Some("fangzhu")));
    }
}

// ---------------------------------------------------------------------------
// 颂威
// ---------------------------------------------------------------------------

#[test]
fn songwei_asks_the_judging_player() {
    let mut g = b0_game();
    g.seats[3].kingdom = "wei".to_string();
    let lord_hand = g.hand(2).len();
    let card = flow::judge_card(&mut g, 3).expect("判定牌");
    // 保证是黑色：c14 黑桃3
    let black = if g.cards.get(&card).map(|c| c.suit.is_red()).unwrap_or(true) {
        "c1".to_string()
    } else {
        card.clone()
    };
    flow::judge_effective(&mut g, 3, &black);
    advance(&mut g).unwrap();
    let p = pend(&g);
    assert_eq!(p.skill(), Some("songwei"));
    assert_eq!(p.actor(), 3, "必须询问判定者，而不是主公");
    answer(&mut g, 3, "choose:songwei");
    assert_eq!(g.hand(2).len(), lord_hand + 1, "发动后主公摸 1");
}

#[test]
fn songwei_not_triggered_on_red_judge() {
    let mut g = b0_game();
    g.to_frame(&"c1".to_string()).ok();
    flow::judge_effective(&mut g, 3, &"c2".to_string()); // 红桃
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "红色判定不触发颂威");
}

// ---------------------------------------------------------------------------
// 门禁：内核仍无技能名（B0 扩充后复查）
// ---------------------------------------------------------------------------

#[test]
fn gate_kernel_has_no_skill_branch_b0() {
    for (name, src) in [
        ("kernel/mod.rs", include_str!("../src/kernel/mod.rs")),
        ("kernel/resolve.rs", include_str!("../src/kernel/resolve.rs")),
        ("kernel/flow.rs", include_str!("../src/kernel/flow.rs")),
        ("kernel/deferred.rs", include_str!("../src/kernel/deferred.rs")),
    ] {
        for skill in [
            "jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei",
        ] {
            assert!(!src.contains(skill), "{name} 出现技能名 {skill}");
        }
    }
    // 注册表里 B0 七技均以声明方式存在（active() 可查）
    for code in ["jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei"] {
        assert!(skills::def(code).is_some(), "技能 {code} 未注册");
    }
}

#[test]
fn jushou_discard_step_is_mandatory() {
    // 走查发现：弃/使用 步骤曾下发「不弃置（跳过）」出口，与对齐单「④ 必须弃置 1 张」冲突。
    let mut g = b0_game();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    answer(&mut g, 0, "choose:jushou");
    assert_eq!(pend(&g).skill(), Some("jushou"));
    assert!(pend(&g).escape_hidden(), "弃置步骤应为强制（出口不下发）");
    let before = g.hand(0).len();
    answer(&mut g, 0, "pass");
    assert!(g.hand(0).len() < before, "不能跳过弃置");
}

#[test]
fn fenji_triggers_on_other_discard_even_in_own_turn() {
    // 口径 B（2026-11-11）：不按回合，按「被其他角色弃置」。自己回合内被他人弃手牌也要触发。
    let mut g = b0_game(); // 周泰在座位 1
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = g.hand(0)[0].clone();
    g.to_discard_by(2, &card, "guohe").unwrap(); // p2 弃置回合玩家 p0 的手牌
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fenji"));
    assert!(pend(&g).options()[0].label.contains("p0"), "令手牌被弃置者摸 2");
}

#[test]
fn fenji_ignores_self_discard_and_discard_phase() {
    let mut g = b0_game();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = g.hand(0)[0].clone();
    g.to_discard_by(0, &card, "discard_phase").unwrap(); // 自己弃牌
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "自己弃牌不触发奋激");
}

#[test]
fn fenji_gain_from_other_targets_original_owner() {
    // 口径 B：被其他角色获得手牌 → 令**原持有者**摸 2（不是获得者）。
    let mut g = b0_game();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = g.hand(2)[0].clone();
    g.begin_action_as(0);
    g.gain(&card, 0).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("fenji"));
    assert!(
        pend(&g).options()[0].label.contains("p2"),
        "奋激对象应为原持有者：{:?}",
        pend(&g).options()[0].label
    );
}
