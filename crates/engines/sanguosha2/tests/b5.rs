//! B5 用例：孙鲁班（谮毁/骄矜）、朱桓（奋励/平寇）、顾雍（慎行/秉壹）、
//! 曹叡（恢拓/明鉴/兴衰）、曹休（千驹/倾袭）、钟繇（活墨/佐定）。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::presentation::POutcome;
use sanguosha2::skills;

fn seat(name: &str, skills: &[&str], hp: i32) -> Seat {
    Seat::new(name, "gen", skills, hp)
}

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit)] = &[
        ("c1", "杀", Suit::Spade),
        ("c2", "闪", Suit::Heart),
        ("c3", "桃", Suit::Diamond),
        ("c4", "决斗", Suit::Club),
        ("c5", "顺手牵羊", Suit::Spade),
        ("c6", "过河拆桥", Suit::Club),
        ("c7", "无懈可击", Suit::Club),
        ("c8", "五谷丰登", Suit::Heart),
        ("c9", "闪", Suit::Diamond),
        ("c10", "杀", Suit::Heart),
        ("c11", "桃", Suit::Club),
        ("c12", "青釭剑", Suit::Spade),
        ("c13", "八卦阵", Suit::Heart),
        ("c14", "赤兔", Suit::Diamond),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit)| match *name {
            "青釭剑" => Card::equip(*id, name, *suit, 6, Slot::Weapon),
            "八卦阵" => Card::equip(*id, name, *suit, 2, Slot::Armor),
            "赤兔" => Card::equip(*id, name, *suit, 5, Slot::HorseOffense),
            _ => Card::new(*id, name, *suit, 5),
        })
        .collect();
    for i in 15..=80 {
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

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}

fn options(g: &Game) -> Vec<String> {
    pend(g).options().iter().map(|o| o.id.clone()).collect()
}

fn play(g: &mut Game, seat: usize) {
    begin_phase(g, seat, Phase::Play).unwrap();
}

fn log_text(g: &Game) -> String {
    g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

/// 让某个座位在某阶段「独占最多」：清空其他人的手牌/体力之外的状态。
fn set_male(g: &mut Game, seat: usize, male: bool) {
    g.seats[seat].male = male;
}

// ---------------------------------------------------------------------------
// 谮毁：唯一目标 + 黑色普通锦囊也接；多目标不触发
// ---------------------------------------------------------------------------

#[test]
fn zanhui_helper_becomes_extra_target() {
    let mut g = game(&[
        ("sunluban", &["zanhui"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.use_card(0, &"c1".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zanhui"));
    answer(&mut g, 0, "target:2");
    answer(&mut g, 2, "opt2");
    assert_eq!(g.counter("turn.zanhui_extra_target"), 1, "成为额外目标");
    assert_eq!(g.counter("turn.zanhui_used"), 1);
}

#[test]
fn zanhui_black_trick_triggers_but_multi_target_does_not() {
    let mut g = game(&[
        ("sunluban", &["zanhui"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c5"]); // 黑桃【顺手牵羊】（黑色普通锦囊）
    play(&mut g, 0);
    g.use_card(0, &"c5".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zanhui"), "黑色普通锦囊也触发");
    answer(&mut g, 0, "pass");

    // 多目标：目标列表长度 2 → 不触发
    let mut g = game(&[
        ("sunluban", &["zanhui"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]);
    play(&mut g, 0);
    g.use_card(0, &"c1".to_string(), vec![1, 2]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非唯一目标不触发");
}

#[test]
fn zanhui_helper_gives_card_and_becomes_user() {
    let mut g = game(&[
        ("sunluban", &["zanhui"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]);
    give(&mut g, 2, &["c3"]);
    play(&mut g, 0);
    g.use_card(0, &"c1".to_string(), vec![1]);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "target:2");
    answer(&mut g, 2, "opt1");
    answer(&mut g, 2, "choose:c3");
    assert!(g.hand(0).contains(&"c3".to_string()), "交出牌进入孙鲁班手牌");
    assert_eq!(g.counter("turn.zanhui_switch_user"), 3, "使用者改为协助者（seat+1）");
}

// ---------------------------------------------------------------------------
// 骄矜：男性来源 -1；女性来源不触发；减为 0 不产生伤害事件
// ---------------------------------------------------------------------------

#[test]
fn jiaojin_reduces_damage_by_one_with_equip_cost() {
    let mut g = game(&[
        ("sunluban", &["jiaojin"], 4),
        ("male", &[], 4),
    ]);
    set_male(&mut g, 1, true);
    equip(&mut g, 0, "c12", Slot::Weapon);
    flow::damage_with(&mut g, Some(1), 0, 2, None);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("jiaojin"));
    answer(&mut g, 0, "choose:c12");
    assert_eq!(g.seats[0].hp, 3, "伤害 2 - 1 = 1");
    assert_eq!(g.zone_of(&"c12".to_string()), Some(&Zone::Discard));
}

#[test]
fn jiaojin_not_triggered_by_female_source() {
    let mut g = game(&[
        ("sunluban", &["jiaojin"], 4),
        ("female", &[], 4),
    ]);
    set_male(&mut g, 1, false);
    equip(&mut g, 0, "c12", Slot::Weapon);
    flow::damage_with(&mut g, Some(1), 0, 1, None);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "女性角色造成的伤害不触发");
    assert_eq!(g.seats[0].hp, 3);
}

#[test]
fn jiaojin_zero_damage_produces_no_damage_event() {
    let mut g = game(&[
        ("sunluban", &["jiaojin"], 4),
        ("male", &[], 4),
    ]);
    set_male(&mut g, 1, true);
    equip(&mut g, 0, "c12", Slot::Weapon);
    flow::damage_with(&mut g, Some(1), 0, 1, None);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "choose:c12");
    assert_eq!(g.seats[0].hp, 4, "伤害减为 0，不扣血");
    assert_eq!(g.counter("phase.damage"), 0, "不计入「本阶段受到过伤害」");
}

// ---------------------------------------------------------------------------
// 奋励：严格最多才可跳过；并列不触发；可连跳
// ---------------------------------------------------------------------------

#[test]
fn fenli_skips_draw_phase_when_strictly_most_hand() {
    let mut g = game(&[
        ("zhuhuan", &["fenli"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert_eq!(pend(&g).skill(), Some("fenli"));
    answer(&mut g, 0, "opt1");
    assert_eq!(g.counter("turn.skip_draw"), 1);
    assert_eq!(g.counter("turn.fenli_skipped"), 1);
}

#[test]
fn fenli_not_offered_when_tied() {
    let mut g = game(&[
        ("zhuhuan", &["fenli"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    give(&mut g, 1, &["c3"]);
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert!(g.pending.is_none(), "并列不算「全场最多」（裁定 Q1=B）");
}

// ---------------------------------------------------------------------------
// 平寇：每跳一个阶段 1 点伤害；「不发动」只放弃本次
// ---------------------------------------------------------------------------

#[test]
fn pingkou_deals_damage_per_skipped_phase() {
    let mut g = game(&[
        ("zhuhuan", &["pingkou"], 4),
        ("p1", &[], 4),
    ]);
    g.counters.insert("turn.fenli_skipped".to_string(), 2);
    play(&mut g, 0);
    g.end_phase("End").unwrap();
    assert_eq!(pend(&g).skill(), Some("pingkou"));
    answer(&mut g, 0, "pass"); // 放弃本次，仍有 1 次
    assert_eq!(pend(&g).skill(), Some("pingkou"), "放弃本次后仍可继续");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3);
}

#[test]
fn pingkou_not_offered_without_skipped_phase() {
    let mut g = game(&[
        ("zhuhuan", &["pingkou"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    g.end_phase("End").unwrap();
    assert!(g.pending.is_none());
}

// ---------------------------------------------------------------------------
// 慎行：弃两张（含装备区）摸一张
// ---------------------------------------------------------------------------

#[test]
fn shenxing_discards_two_including_equipment_and_draws_one() {
    let mut g = game(&[
        ("guyong", &["shenxing"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    equip(&mut g, 0, "c13", Slot::Armor);
    play(&mut g, 0);
    assert!(skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:shenxing"));
    skills::execute_active(&mut g, 0, "skill:shenxing").unwrap();
    answer(&mut g, 0, "choose:c2");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c13".to_string()), "装备区牌可选：{opts:?}");
    answer(&mut g, 0, "choose:c13");
    answer(&mut g, 0, "confirm");
    assert_eq!(g.hand(0).len(), 1, "弃 2 摸 1");
    assert_eq!(g.zone_of(&"c13".to_string()), Some(&Zone::Discard));
}

#[test]
fn shenxing_needs_two_cards() {
    let mut g = game(&[
        ("guyong", &["shenxing"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    assert!(!skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:shenxing"));
}

// ---------------------------------------------------------------------------
// 秉壹：自己的牌被弃置 → 展示 → 同色则自己 + 至多 X 名各摸 1；判定区弃置不触发
// ---------------------------------------------------------------------------

#[test]
fn bingyi_reveals_and_lets_chosen_targets_draw() {
    let mut g = game(&[
        ("guyong", &["bingyi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]); // 两张红色
    play(&mut g, 0);
    g.to_discard(&"c2".to_string(), "shenxing").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("bingyi"));
    answer(&mut g, 0, "opt1");
    assert!(log_text(&g).contains("展示所有手牌"), "展示写日志摘要");
    // 至多 X 名（X=2）：选 1 名即可
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "confirm");
    assert_eq!(g.counter("phase.bingyi"), 1);
    assert!(g.hand(1).len() >= 1);
}

#[test]
fn bingyi_not_triggered_for_judge_zone_discard_or_twice_per_phase() {
    let mut g = game(&[
        ("guyong", &["bingyi"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    play(&mut g, 0);
    // 判定区牌被弃置：不算「你的牌因弃置」（裁定：不含判定区）
    g.to_judge(&"c5".to_string(), 0).unwrap();
    g.to_discard(&"c5".to_string(), "judge").unwrap();
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "判定区牌不计入秉壹");
    // 手牌弃置：触发一次后本阶段不再触发
    g.to_discard(&"c2".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("bingyi"));
    answer(&mut g, 0, "pass");
    g.to_discard(&"c3".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "每阶段限一次");
}

// ---------------------------------------------------------------------------
// 恢拓：受到伤害后判定，红色回血 / 黑色摸伤害值张
// ---------------------------------------------------------------------------

#[test]
fn huituo_red_judge_heals_target() {
    let mut g = game(&[
        ("caorui", &["huituo"], 4),
        ("p1", &[], 4),
    ]);
    g.seats[1].hp = 3;
    // v2 的牌堆顶 = Draw 区**第一张**：把它改成红色判定牌
    let top = g.ids_in(|z| *z == Zone::Draw).first().cloned().unwrap();
    g.cards.get_mut(&top).unwrap().suit = Suit::Heart;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("huituo"));
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 4, "红色 → 回复 1 点体力");
}

/// 回归（2026-11-11 真机）：一条恢拓不能看起来回两次血。
/// 口径：hp 只 +1，且 `Heal` 展示只有 1 条（技能侧不再重复 push），日志里「回复」只出现一次。
/// 回归（2026-11-11 同类排查）：濒死浮层读 `pending.data.victim/hp`。
#[test]
fn dying_pending_carries_victim_and_hp() {
    let mut g = game(&[("a", &[], 4), ("b", &[], 4)]);
    give(&mut g, 1, &["c3"]); // 让 p1 有【桃】→ 濒死询问一定产生 pending
    g.seats[0].hp = 0;
    flow::start_dying(&mut g, 0, None);
    advance(&mut g).unwrap();
    let p = pend(&g);
    assert_eq!(p.kind(), "dying");
    assert_eq!(p.data()["victim"].as_str(), Some("a"), "濒死浮层要显示濒死者");
    assert_eq!(p.data()["hp"].as_i64(), Some(0), "濒死浮层要显示体力");
}

#[test]
fn huituo_heals_exactly_once() {
    let mut g = game(&[("caorui", &["huituo"], 4), ("p1", &[], 4)]);
    g.seats[1].hp = 3;
    let top = g.ids_in(|z| *z == Zone::Draw).first().cloned().unwrap();
    g.cards.get_mut(&top).unwrap().suit = Suit::Heart;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 4, "红色判定只回 1 点体力");
    let heals = g
        .presentation
        .iter()
        .filter(|p| matches!(p.outcome, POutcome::Heal { .. }))
        .count();
    assert_eq!(heals, 1, "回复展示只允许 1 条（否则端侧看起来回了两次）");
    let heal_logs = log_text(&g).matches("回复").count();
    assert_eq!(heal_logs, 1, "日志里「回复」只允许出现 1 次：{}", log_text(&g));
}

#[test]
fn huituo_black_judge_draws_damage_amount() {
    let mut g = game(&[
        ("caorui", &["huituo"], 4),
        ("p1", &[], 4),
    ]);
    // 让牌堆顶（Draw 区第一张）为黑色
    let top = g.ids_in(|z| *z == Zone::Draw).first().cloned().unwrap();
    g.cards.get_mut(&top).unwrap().suit = Suit::Spade;
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), 2, "黑色 → 摸伤害值（2）张");
}

#[test]
fn huituo_full_hp_red_branch_skipped() {
    let mut g = game(&[
        ("caorui", &["huituo"], 4),
        ("p1", &[], 4),
    ]);
    // 满体力目标 + 红色判定（牌堆顶 = Draw 区第一张）
    let top = g.ids_in(|z| *z == Zone::Draw).first().cloned().unwrap();
    g.cards.get_mut(&top).unwrap().suit = Suit::Heart;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 4, "满体力不回复（裁定 Q2=B）");
    assert!(log_text(&g).contains("体力已满"));
}

// ---------------------------------------------------------------------------
// 明鉴：交出所有手牌（获得语义）；0 张不能发动；下回合 +1/+1
// ---------------------------------------------------------------------------

#[test]
fn mingjian_gives_all_hand_and_buffs_next_turn() {
    let mut g = game(&[
        ("caorui", &["mingjian"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:mingjian").unwrap();
    answer(&mut g, 0, "target:1");
    assert!(g.hand(0).is_empty() && g.hand(1).len() == 2, "全部手牌交出");
    assert_eq!(g.counter("mingjian:1"), 1, "待生效");
    // 其回合开始 → 生效（+1 手牌上限 / +1 杀次数）
    let base_hl = 4;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(g.counter("mingjian:1"), 2);
    assert_eq!(g.hand_limit(1), base_hl + 1);
    assert_eq!(g.sha_limit(1), 2);
}

#[test]
fn mingjian_not_available_without_hand() {
    let mut g = game(&[
        ("caorui", &["mingjian"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:mingjian"),
        "手牌为 0 不能发动（裁定 Q1=B）"
    );
}

// ---------------------------------------------------------------------------
// 兴衰：濒死时逐个询问魏势力；脱困后支援者受到 1 点伤害
// ---------------------------------------------------------------------------

#[test]
fn xingshuai_asks_wei_allies_then_hurts_them_after_dying() {
    let mut g = game(&[
        ("caorui", &["xingshuai"], 3),
        ("weiA", &[], 4),
        ("qun", &[], 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[1].kingdom = "wei".to_string();
    g.seats[2].kingdom = "qun".to_string();
    g.seats[0].hp = 1;
    flow::damage(&mut g, Some(2), 0, 1); // 1 → 0 濒死
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("dying"));
    let opts = options(&g);
    assert!(opts.contains(&"xingshuai".to_string()), "求到自己时可发动兴衰：{opts:?}");
    let wei_hp = g.seats[1].hp;
    answer(&mut g, 0, "xingshuai");
    assert_eq!(pend(&g).actor(), 1, "先问魏势力角色");
    answer(&mut g, 1, "opt1");
    assert_eq!(g.seats[0].hp, 1, "脱困（0 → 1）");
    // 脱困后支援者受到 1 点伤害
    assert_eq!(g.seats[1].hp, wei_hp - 1, "支援者受到 1 点伤害");
    assert!(log_text(&g).contains("受到 1 点伤害"));
}

#[test]
fn xingshuai_not_available_without_lord_or_wei_allies() {
    // 非主公（给自己一张【桃】以便濒死询问存在）
    let mut g = game(&[
        ("caorui", &["xingshuai"], 3),
        ("weiA", &[], 4),
    ]);
    g.seats[1].kingdom = "wei".to_string();
    g.seats[0].hp = 1;
    give(&mut g, 0, &["c3"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("dying"));
    assert!(!options(&g).contains(&"xingshuai".to_string()), "非主公不可发动");
    // 主公但没有魏势力
    let mut g = game(&[
        ("caorui", &["xingshuai"], 3),
        ("qun", &[], 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[0].hp = 1;
    give(&mut g, 0, &["c3"]);
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("dying"));
    assert!(!options(&g).contains(&"xingshuai".to_string()), "无魏势力不可发动");
}

// ---------------------------------------------------------------------------
// 千驹：每损失 1 点体力距离 -1（与进攻马叠加）
// ---------------------------------------------------------------------------

#[test]
fn qianju_reduces_distance_per_lost_hp() {
    let mut g = game(&[
        ("caoxiu", &["qianju"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
        ("p4", &[], 4),
    ]);
    // 5 人局：0 与 3 距离 2
    assert_eq!(g.distance(0, 3), 2);
    g.seats[0].hp = 2; // 损失 2 点 → 距离 -2 → 下限 1
    assert_eq!(g.distance(0, 3), 1);
}

#[test]
fn qianju_absent_keeps_distance() {
    let mut g = game(&[
        ("caoxiu", &[], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
        ("p4", &[], 4),
    ]);
    g.seats[0].hp = 2;
    assert_eq!(g.distance(0, 3), 2, "没有千驹不改距离");
}

// ---------------------------------------------------------------------------
// 倾袭：目标二选一（弃等量手牌+弃武器 / 伤害 +1）
// ---------------------------------------------------------------------------

#[test]
fn qingxi_lets_target_discard_cards_and_weapon() {
    let mut g = game(&[
        ("caoxiu", &["qingxi"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 0, "c12", Slot::Weapon); // 青釭剑 range 2
    give(&mut g, 1, &["c2", "c3", "c9"]);
    flow::damage_with(&mut g, Some(0), 1, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("qingxi"));
    assert_eq!(pend(&g).actor(), 1, "由目标选择");
    answer(&mut g, 1, "opt1");
    answer(&mut g, 1, "choose:c2");
    answer(&mut g, 1, "choose:c3");
    assert_eq!(g.zone_of(&"c12".to_string()), Some(&Zone::Discard), "弃置其武器");
    assert_eq!(g.seats[1].hp, 3, "按原伤害结算");
}

#[test]
fn qingxi_opt2_increases_damage_and_no_weapon_no_trigger() {
    let mut g = game(&[
        ("caoxiu", &["qingxi"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 0, "c12", Slot::Weapon);
    flow::damage_with(&mut g, Some(0), 1, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    answer(&mut g, 1, "opt2");
    assert_eq!(g.seats[1].hp, 2, "伤害 +1 = 2");

    // 没有武器 → 不询问
    let mut g = game(&[
        ("caoxiu", &["qingxi"], 4),
        ("p1", &[], 4),
    ]);
    flow::damage_with(&mut g, Some(0), 1, 1, Some(&"c1".to_string()));
    advance(&mut g).unwrap();
    assert!(g.pending.is_none());
}

// ---------------------------------------------------------------------------
// 活墨：黑色非基本牌置顶，视为使用牌名不同的基本牌
// ---------------------------------------------------------------------------

#[test]
fn huomo_places_black_trick_on_top_and_uses_missing_basic() {
    let mut g = game(&[
        ("zhongyao", &["huomo"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c4"]); // 黑色【决斗】
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:huomo").unwrap();
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.ends_with(":杀")), "可转化【杀】：{opts:?}");
    answer(&mut g, 0, "choose:c4:杀");
    assert_eq!(g.zone_of(&"c4".to_string()), Some(&Zone::Draw), "置于牌堆顶");
    assert_eq!(g.counter("turn.huomo_used"), 1);
    assert!(log_text(&g).contains("视为使用【杀】"));
    // M2：真实结算 → 选目标后 p1 无闪，受到 1 点伤害。
    assert_eq!(pend(&g).actor(), 0, "活墨【杀】需要选目标");
    answer(&mut g, 0, "target:1");
    assert_eq!(g.seats[1].hp, 3, "视为使用【杀】真实造成伤害");
}

#[test]
fn huomo_excludes_used_basic_names_and_red_cards() {
    let mut g = game(&[
        ("zhongyao", &["huomo"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c4"]); // 黑色【决斗】
    g.counters.insert("turn.huomo_used".to_string(), 1); // 本回合已用过【杀】
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:huomo").unwrap();
    let opts = options(&g);
    assert!(!opts.iter().any(|o| o.ends_with(":杀")), "用过的牌名不再提供：{opts:?}");
    assert!(opts.iter().any(|o| o.ends_with(":桃")) || opts.iter().any(|o| o.ends_with(":酒")));
    answer(&mut g, 0, "pass");

    // 红色非基本牌不可用
    let mut g = game(&[
        ("zhongyao", &["huomo"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c8"]); // 红桃【五谷丰登】
    play(&mut g, 0);
    assert!(!skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:huomo"));
}

// ---------------------------------------------------------------------------
// 佐定：其他角色出牌阶段黑桃牌指定目标后（本阶段无伤害）令其中一个目标摸 1
// ---------------------------------------------------------------------------

#[test]
fn zuoding_lets_one_target_draw_for_spade_card() {
    let mut g = game(&[
        ("zhongyao", &["zuoding"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    play(&mut g, 1);
    give(&mut g, 1, &["c1"]); // 黑桃【杀】
    g.use_card(1, &"c1".to_string(), vec![0, 2]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zuoding"));
    assert_eq!(pend(&g).actor(), 0, "由钟繇决定");
    let opts = options(&g);
    assert!(!opts.contains(&"target:0".to_string()), "不能令自己摸：{opts:?}");
    answer(&mut g, 0, "target:2");
    assert!(g.hand(2).len() >= 1);
    // 同一张牌只触发一次
    g.emit(sanguosha2::kernel::event::Event::card(
        sanguosha2::kernel::event::EventKind::TargetChosen,
        Some(1),
        "c1".to_string(),
        Some("2".to_string()),
    ));
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "一张牌只触发一次");
}

#[test]
fn zuoding_skipped_after_damage_or_for_heart() {
    // 本阶段已有伤害
    let mut g = game(&[
        ("zhongyao", &["zuoding"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    play(&mut g, 1);
    flow::damage(&mut g, Some(1), 2, 1);
    g.events.clear();
    give(&mut g, 1, &["c1"]);
    g.use_card(1, &"c1".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "本阶段有伤害则不触发");
    // 非黑桃
    let mut g = game(&[
        ("zhongyao", &["zuoding"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    play(&mut g, 1);
    give(&mut g, 1, &["c10"]); // 红桃【杀】
    g.use_card(1, &"c10".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非黑桃不触发");
}

// ---------------------------------------------------------------------------
// 守恒 / 无帧遗留
// ---------------------------------------------------------------------------

#[test]
fn b5_flows_keep_conservation() {
    let mut g = game(&[
        ("caorui", &["mingjian", "huituo"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:mingjian").unwrap();
    answer(&mut g, 0, "target:1");
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    if g.pending.as_ref().and_then(|p| p.skill()) == Some("huituo") {
        answer(&mut g, 0, "target:1");
    }
    g.check_conservation().expect("牌区守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}
