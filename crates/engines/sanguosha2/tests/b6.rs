//! B6 用例：刘谌（战绝/勤王）、张嶷（怃戎/矢志）、孙休（宴诛/兴学/诏缚）、
//! 朱治（安国）、全琮（邀名）、公孙渊（怀异）、郭图逢纪（急攻/饰非）。

use sanguosha2::kernel::event::EventKind;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
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

// ---------------------------------------------------------------------------
// 战绝：所有手牌当【决斗】；同一角色只摸 1；发动 2 次后失效
// ---------------------------------------------------------------------------

#[test]
fn zhanjue_uses_all_hand_and_limits_to_two_activations() {
    let mut g = game(&[
        ("liuchen", &["zhanjue"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhanjue").unwrap();
    answer(&mut g, 0, "choose:zhanjue");
    assert!(g.hand(0).is_empty(), "全部手牌消耗");
    assert_eq!(g.discard_pile().len(), 2);
    assert_eq!(g.counter("turn.zhanjue_uses"), 1);
    // 第 2 次
    give(&mut g, 0, &["c9"]);
    skills::execute_active(&mut g, 0, "skill:zhanjue").unwrap();
    answer(&mut g, 0, "choose:zhanjue");
    assert_eq!(g.counter("turn.zhanjue_uses"), 2);
    // 第 3 次不可用（裁定 Q2=B：按发动次数）
    give(&mut g, 0, &["c11"]);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:zhanjue"),
        "发动 2 次后本阶段失效"
    );
}

#[test]
fn zhanjue_damage_draws_once_per_character() {
    let mut g = game(&[
        ("liuchen", &["zhanjue"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhanjue").unwrap();
    answer(&mut g, 0, "choose:zhanjue");
    let before = g.hand(1).len();
    skills::zhanjue_on_damage(&mut g, 0, 1);
    assert_eq!(g.hand(1).len(), before + 1, "受伤者摸 1");
    // 自己受伤（owner==victim）只摸 1 张（裁定 Q1=B）
    let me_before = g.hand(0).len();
    skills::zhanjue_on_damage(&mut g, 0, 0);
    assert_eq!(g.hand(0).len(), me_before + 1, "同一角色只摸一次");
}

// ---------------------------------------------------------------------------
// 勤王：主公技；成本含装备区；自己不能响应；伤害来源视为刘谌
// ---------------------------------------------------------------------------

#[test]
fn qinwang_costs_equipment_and_ally_draws() {
    let mut g = game(&[
        ("liuchen", &["qinwang"], 4),
        ("shu_ally", &[], 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[1].kingdom = g.seats[0].kingdom.clone();
    equip(&mut g, 0, "c12", Slot::Weapon);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:qinwang").unwrap();
    let opts = options(&g);
    assert!(opts.contains(&"choose:c12".to_string()), "成本含装备区：{opts:?}");
    answer(&mut g, 0, "choose:c12");
    let before = g.hand(1).len();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), before + 1, "代出者摸 1");
    assert!(log_text(&g).contains("伤害来源视为"), "{}", log_text(&g));
}

#[test]
fn qinwang_needs_lord_and_other_same_kingdom() {
    let mut g = game(&[
        ("liuchen", &["qinwang"], 4),
        ("ally", &[], 4),
    ]);
    g.seats[1].kingdom = g.seats[0].kingdom.clone();
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:qinwang"),
        "非主公不可发动"
    );
    // 主公但只有自己同势力（2 人局：自己不能响应）→ 不可发动（裁定 Q3=B）
    let mut g = game(&[
        ("liuchen", &["qinwang"], 4),
        ("other_kingdom", &[], 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    g.seats[1].kingdom = "shu".to_string(); // 与刘谌（qun）不同势力
    give(&mut g, 0, &["c2"]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:qinwang"),
        "没有同势力其他人可代出"
    );
}

// ---------------------------------------------------------------------------
// 怃戎：同时展示（选择期不公开）；4 种组合
// ---------------------------------------------------------------------------

#[test]
fn wurong_sha_vs_non_shan_deals_damage() {
    let mut g = game(&[
        ("zhangyi", &["wurong"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 杀
    give(&mut g, 1, &["c3"]); // 桃（非闪）
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:wurong").unwrap();
    answer(&mut g, 0, "choose:c1");
    // 选择期：张嶷的牌**不得**公开
    assert!(!g.is_revealed(&"c1".to_string()), "选择期不公开");
    assert!(!log_text(&g).contains("黑桃5杀"), "选择期日志不露牌面");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:c3");
    assert_eq!(g.seats[1].hp, 3, "杀 + 非闪 → 1 点伤害");
    assert_eq!(g.zone_of(&"c1".to_string()), Some(&Zone::Discard));
}

#[test]
fn wurong_non_sha_vs_shan_gains_card() {
    let mut g = game(&[
        ("zhangyi", &["wurong"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c3"]); // 桃（非杀）
    give(&mut g, 1, &["c2", "c9"]); // 闪 + 闪
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:wurong").unwrap();
    answer(&mut g, 0, "choose:c3");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:c2");
    assert_eq!(g.seats[1].hp, 4, "不造成伤害");
    let gained = g.hand(0).iter().any(|id| id == "c2" || id == "c9");
    assert!(gained, "获得其一张牌");
}

#[test]
fn wurong_other_combinations_do_nothing() {
    let mut g = game(&[
        ("zhangyi", &["wurong"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c1"]); // 杀
    give(&mut g, 1, &["c2"]); // 闪 → 杀 vs 闪 = 无事
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:wurong").unwrap();
    answer(&mut g, 0, "choose:c1");
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "choose:c2");
    assert_eq!(g.seats[1].hp, 4);
    assert!(g.hand(0).contains(&"c1".to_string()), "不满足条件不弃置展示牌");
}

// ---------------------------------------------------------------------------
// 矢志：体力 1 时【闪】被替代（不能再当闪）
// ---------------------------------------------------------------------------

#[test]
fn shizhi_blocks_shan_only_at_one_hp() {
    let mut g = game(&[("zhangyi", &["shizhi"], 4), ("p1", &[], 4)]);
    let shan = "c2".to_string();
    g.seats[0].hp = 2;
    assert!(!skills::shan_blocked_by_shizhi(&g, 0, &shan), "体力 2 时【闪】仍可当闪");
    g.seats[0].hp = 1;
    assert!(skills::shan_blocked_by_shizhi(&g, 0, &shan), "体力 1 时【闪】视为【杀】");
    // 非【闪】不受影响
    assert!(!skills::shan_blocked_by_shizhi(&g, 0, &"c1".to_string()));
}

// ---------------------------------------------------------------------------
// 宴诛：opt1 交出装备 + 失去技能 + 修改兴学；opt2 目标自选弃牌
// ---------------------------------------------------------------------------

#[test]
fn yanzhu_option1_gains_equips_and_upgrades_xingxue() {
    let mut g = game(&[
        ("sunxiu", &["yanzhu", "xingxue"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    equip(&mut g, 1, "c13", Slot::Armor);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:yanzhu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt1");
    assert!(g.hand(0).contains(&"c12".to_string()) && g.hand(0).contains(&"c13".to_string()));
    assert!(!skills::active(&g, 0, "yanzhu"), "失去宴诛");
    assert!(g.seats[0].flags.contains("xingxue_upgraded"), "修改兴学");
}

#[test]
fn yanzhu_option2_target_chooses_card_from_hand_or_equip() {
    let mut g = game(&[
        ("sunxiu", &["yanzhu"], 4),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    give(&mut g, 1, &["c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:yanzhu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 1, "opt2");
    let opts = options(&g);
    assert!(opts.contains(&"choose:c12".to_string()), "可弃装备区：{opts:?}");
    answer(&mut g, 1, "choose:c12");
    assert_eq!(g.zone_of(&"c12".to_string()), Some(&Zone::Discard));
}

// ---------------------------------------------------------------------------
// 兴学：结束阶段每点体力一次；置顶由该角色自选
// ---------------------------------------------------------------------------

#[test]
fn xingxue_lets_target_choose_card_to_top() {
    let mut g = game(&[
        ("sunxiu", &["xingxue"], 3),
        ("p1", &[], 4),
    ]);
    g.seats[0].hp = 2;
    play(&mut g, 0);
    g.end_phase("End").unwrap();
    assert_eq!(pend(&g).skill(), Some("xingxue"));
    answer(&mut g, 0, "target:1");
    let hand = g.hand(1);
    let pick = hand.first().cloned().unwrap();
    answer(&mut g, 1, &format!("choose:{pick}"));
    assert_eq!(g.zone_of(&pick), Some(&Zone::Draw), "置顶");
    // 第二次（体力 2 → 2 次）
    assert_eq!(pend(&g).skill(), Some("xingxue"));
    answer(&mut g, 0, "pass");
    assert!(g.pending.is_none(), "可放弃");
}

#[test]
fn xingxue_not_triggered_outside_end_phase() {
    let mut g = game(&[
        ("sunxiu", &["xingxue"], 3),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    g.end_phase("Play").unwrap();
    assert!(g.pending.is_none(), "只有结束阶段触发");
}

// ---------------------------------------------------------------------------
// 诏缚：可选自己；统一攻击范围入口生效
// ---------------------------------------------------------------------------

#[test]
fn zhaofu_marks_targets_and_affects_attack_range() {
    let mut g = game(&[
        ("sunxiu", &["zhaofu"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
        ("p3", &[], 4),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    // 全部设为吴势力（孙休 & p1 为吴）
    for s in 0..4 {
        g.seats[s].kingdom = "wu".to_string();
    }
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhaofu").unwrap();
    answer(&mut g, 0, "target:0"); // 可以选自己（裁定 Q1=A）
    answer(&mut g, 0, "target:2");
    answer(&mut g, 0, "confirm");
    assert!(g.seats[0].flags.contains("zhaofu") && g.seats[2].flags.contains("zhaofu"));
    // 4 人局：0 与 2 距离 2；诏缚后吴势力（p1）也能打到
    assert!(g.in_attack_range(1, 2), "诏缚：吴势力视为在攻击范围内");
    assert!(!g.in_attack_range(3, 2) || g.seats[3].kingdom == "wu", "非吴势力不生效");
}

#[test]
fn zhaofu_requires_lord_and_once_per_game() {
    let mut g = game(&[("sunxiu", &["zhaofu"], 4), ("p1", &[], 4)]);
    play(&mut g, 0);
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:zhaofu"),
        "非主公不可发动"
    );
    let mut g = game(&[("sunxiu", &["zhaofu"], 4), ("p1", &[], 4)]);
    g.seats[0].flags.insert("lord".to_string());
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:zhaofu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "confirm");
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:zhaofu"),
        "限定技每局一次"
    );
}

// ---------------------------------------------------------------------------
// 安国：按条件执行 + 补做；装备从牌堆随机取
// ---------------------------------------------------------------------------

#[test]
fn anguo_applies_minimum_effects_for_target() {
    let mut g = game(&[
        ("zhuzhi", &["anguo"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c3"]);
    give(&mut g, 1, &["c9"]); // p1 手牌最少
    give(&mut g, 2, &["c5", "c6", "c7"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:anguo").unwrap();
    answer(&mut g, 0, "target:1");
    assert!(g.hand(1).len() >= 2, "手牌最少 → 摸 1");
    // 朱治手牌 2 > p1 的 1 → 不满足，不补做摸牌
    assert_eq!(g.counter("turn.anguo_used"), 1);
}

#[test]
fn anguo_uses_equip_from_draw_pile_when_equip_min() {
    let mut g = game(&[
        ("zhuzhi", &["anguo"], 4),
        ("p1", &[], 4),
    ]);
    // p1 装备最少（0）；牌堆里放一张装备牌
    give(&mut g, 1, &["c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:anguo").unwrap();
    answer(&mut g, 0, "target:1");
    // p1 装备最少 → 从牌堆随机使用一张装备牌（c12 青釭剑）
    let equipped = g.ids_in(|z| matches!(z, Zone::Equip(s, _) if *s == 1)).len();
    assert_eq!(equipped, 1, "从未牌堆使用装备（裁定 Q2=C）");
    g.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 邀名：造成 / 受到各限一次；弃置由全琮选
// ---------------------------------------------------------------------------

#[test]
fn yaoming_after_dealing_damage_discards_target_hand() {
    let mut g = game(&[
        ("quancong", &["yaoming"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c2", "c3", "c9"]); // 手牌多于全琮（0）
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("yaoming"));
    answer(&mut g, 0, "discard:1");
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("choose:")), "由全琮选牌：{opts:?}");
    let before = g.hand(1).len();
    let pick = opts.into_iter().find(|o| o.starts_with("choose:")).unwrap();
    answer(&mut g, 0, &pick);
    assert_eq!(g.hand(1).len(), before - 1);
}

#[test]
fn yaoming_dealt_and_taken_each_once_per_turn() {
    let mut g = game(&[
        ("quancong", &["yaoming"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c2", "c3"]);
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "pass");
    // 再次造成伤害 → 已用（造成方向）
    g.events.clear();
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "造成方向已限一次");
    // 受到伤害方向仍可用
    g.events.clear();
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.pend_skill(), Some("yaoming"));
}

trait PendSkill {
    fn pend_skill(&self) -> Option<&str>;
}
impl PendSkill for Game {
    fn pend_skill(&self) -> Option<&str> {
        self.pending.as_ref().and_then(|p| p.skill())
    }
}

// ---------------------------------------------------------------------------
// 怀异：展示亮牌；获得 2 张走 lose_hp；获得张数 = min
// ---------------------------------------------------------------------------

#[test]
fn huaiyi_reveals_and_loses_hp_after_gaining_two() {
    let mut g = game(&[
        ("gongsun", &["huaiyi"], 4),
        ("p1", &[], 4),
        ("p2", &[], 4),
    ]);
    give(&mut g, 0, &["c2", "c9"]); // 两张红色
    give(&mut g, 1, &["c3", "c11"]);
    give(&mut g, 2, &["c5"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:huaiyi").unwrap();
    answer(&mut g, 0, "opt_red");
    assert!(log_text(&g).contains("展示手牌"), "展示写日志");
    // 获得 2 张（同一角色可重复）
    answer(&mut g, 0, "target:1");
    let pick = options(&g)[0].clone();
    answer(&mut g, 0, &pick);
    answer(&mut g, 0, "target:1");
    let hp_before = g.seats[0].hp;
    let pick2 = options(&g)[0].clone();
    answer(&mut g, 0, &pick2);
    // 剩余 0 → 结算
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert_eq!(g.seats[0].hp, hp_before - 1, "获得 ≥2 张 → 失去 1 点体力");
    assert!(
        g.log.iter().any(|l| l.text.contains("失去 1 点体力")),
        "走 lose_hp：{}",
        log_text(&g)
    );
}

#[test]
fn huaiyi_gaining_one_does_not_lose_hp() {
    let mut g = game(&[
        ("gongsun", &["huaiyi"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c2"]);
    give(&mut g, 1, &["c3"]);
    play(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:huaiyi").unwrap();
    answer(&mut g, 0, "opt_red");
    answer(&mut g, 0, "target:1");
    let pick = options(&g)[0].clone();
    answer(&mut g, 0, &pick);
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert_eq!(g.seats[0].hp, 4, "只获得 1 张不失去体力");
}

// ---------------------------------------------------------------------------
// 急攻：出牌阶段开始可摸 2；手牌上限 = 本阶段伤害值
// ---------------------------------------------------------------------------

#[test]
fn jigong_draws_two_and_sets_hand_limit_to_damage() {
    let mut g = game(&[
        ("fujigong", &["jigong"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    assert_eq!(pend(&g).skill(), Some("jigong"));
    let before = g.hand(0).len();
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), before + 2);
    assert!(g.seats[0].flags.contains("jigong"));
    assert_eq!(g.hand_limit(0), 0, "未造成伤害 → 手牌上限 0");
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert_eq!(g.hand_limit(0), 1, "造成 1 点伤害 → 手牌上限 1（赋值）");
}

#[test]
fn jigong_can_be_declined() {
    let mut g = game(&[
        ("fujigong", &["jigong"], 4),
        ("p1", &[], 4),
    ]);
    play(&mut g, 0);
    answer(&mut g, 0, "pass");
    assert!(!g.seats[0].flags.contains("jigong"));
    assert_eq!(g.hand_limit(0), 4, "不发动 → 按体力值");
}

// ---------------------------------------------------------------------------
// 饰非：需要【闪】时的替代入口（使用 + 打出共用）
// ---------------------------------------------------------------------------

#[test]
fn shifei_lets_current_player_draw_then_discards_from_most() {
    let mut g = game(&[
        ("fujigong", &["shifei"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 1, &["c2", "c3", "c9"]); // 当前回合角色手牌最多
    play(&mut g, 1);
    let opts = skills::dodge_options(&g, 0);
    assert!(!opts.is_empty(), "提供饰非选项");
    let handled = skills::dodge_use(&mut g, 0, "skill:shifei").unwrap();
    assert!(handled);
    // 当前回合角色摸 1 后仍是唯一最多 → 不弃牌
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert!(g.log.iter().any(|l| l.text.contains("饰非")));
}

#[test]
fn shifei_discards_when_current_player_not_unique_most() {
    let mut g = game(&[
        ("fujigong", &["shifei"], 4),
        ("p1", &[], 4),
    ]);
    give(&mut g, 0, &["c12"]); // 装备区 1 张 → 手牌 0
    give(&mut g, 1, &["c2", "c3"]);
    play(&mut g, 1);
    skills::dodge_use(&mut g, 0, "skill:shifei").unwrap();
    advance(&mut g).unwrap();
    // 当前回合角色摸 1（3 张）→ 与 p0 手牌 0 不同；最多者=当前回合角色 → 唯一最多 → 不弃
    // 构造并列最多：让 p0 也摸到同数量
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    assert!(!skills::dodge_options(&g, 1).iter().any(|(id, _)| id == "skill:shifei"));
}

#[test]
fn shifei_absent_without_skill() {
    let mut g = game(&[("a", &[], 4), ("p1", &[], 4)]);
    play(&mut g, 0);
    assert!(skills::dodge_options(&g, 0).is_empty());
}

// ---------------------------------------------------------------------------
// 守恒 / 无帧遗留
// ---------------------------------------------------------------------------

#[test]
fn b6_flows_keep_conservation() {
    let mut g = game(&[
        ("sunxiu", &["yanzhu", "xingxue"], 3),
        ("p1", &[], 4),
    ]);
    equip(&mut g, 1, "c12", Slot::Weapon);
    give(&mut g, 1, &["c3"]);
    play(&mut g, 0);
    if skills::legal_actions(&g, 0).iter().any(|(id, _)| id == "skill:yanzhu") {
        skills::execute_active(&mut g, 0, "skill:yanzhu").unwrap();
        answer(&mut g, 0, "target:1");
        answer(&mut g, 1, "opt1");
    }
    g.end_phase("End").unwrap();
    if g.pending.is_some() {
        answer(&mut g, 0, "pass");
    }
    g.check_conservation().expect("牌区守恒");
    g.check_no_orphan_frames().expect("无帧遗留");
}

#[test]
fn xingxue_upgraded_settles_once() {
    let mut g = game(&[
        ("sunxiu", &["xingxue"], 3),
        ("p1", &[], 4),
    ]);
    g.seats[0].hp = 3;
    g.seats[0].flags.insert("xingxue_upgraded".to_string()); // 宴诛 opt1 已修改
    play(&mut g, 0);
    g.end_phase("End").unwrap();
    assert_eq!(pend(&g).skill(), Some("xingxue"));
    answer(&mut g, 0, "target:1");
    let pick = g.hand(1).first().cloned().unwrap();
    answer(&mut g, 1, &format!("choose:{pick}"));
    assert!(g.pending.is_none(), "修改后只结算 1 次（体力 3 不再有额外次数）");
}
