//! B1 第二批（13 技能）验收测试：聚焦用户裁定的关键差异。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Suit, Zone};
use sanguosha2::skills;

fn card(id: &str, name: &str, suit: Suit, rank: u8) -> Card {
    Card::new(id, name, suit, rank)
}

/// 造局：`(name, general, skills, kingdom)`；牌固定 20 张（含杀/桃/装备）。
fn game(specs: &[(&str, &str, &[&str], &str)]) -> Game {
    let seats: Vec<Seat> = specs
        .iter()
        .map(|(n, gen, sk, k)| {
            let mut s = Seat::new(n, gen, sk, 4);
            s.kingdom = k.to_string();
            s
        })
        .collect();
    let mut cards = Vec::new();
    let names = ["杀", "闪", "桃", "杀", "无懈可击", "杀", "闪", "桃", "杀", "决斗"];
    for i in 1..=20 {
        cards.push(card(
            &format!("c{i}"),
            names[(i - 1) % names.len()],
            if i % 2 == 0 { Suit::Heart } else { Suit::Spade },
            (i % 13) as u8,
        ));
    }
    let mut g = Game::new(seats, cards);
    for seat in 0..specs.len() {
        for j in 0..3 {
            let id = format!("c{}", seat * 3 + j + 1);
            g.to_hand(&id, seat).unwrap();
        }
    }
    g
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
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
    g.check_conservation().expect("守恒");
}

// ---------------------------------------------------------------------------
// 拒战：窗口
// ---------------------------------------------------------------------------

#[test]
fn juzhan_yang_only_on_others_turn() {
    let mut g = game(&[
        ("p0", "yanyan", &["juzhan"], "shu"),
        ("p1", "other", &[], "qun"),
    ]);
    // 他人回合（p1）用杀指定严颜 → 阳面可发动
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c4".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("juzhan"));
    assert_eq!(pend(&g).actor(), 0);
    answer(&mut g, 0, "choose:juzhan");
    assert!(!g.yang(0, "juzhan"), "发动后阳→阴");
}

#[test]
fn juzhan_yin_only_on_own_turn() {
    let mut g = game(&[
        ("p0", "yanyan", &["juzhan"], "shu"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.use_card(0, &"c1".to_string(), vec![1]);
    advance(&mut g).unwrap();
    let p = pend(&g);
    assert_eq!(p.skill(), Some("juzhan"));
    assert!(p.has_option("choose:juzhan"), "阴面应可发动：{:?}", p.options());
    assert_eq!(p.escape_id(), "pass");
}

// ---------------------------------------------------------------------------
// 遗礼：代价致濒死 → 救回后才给橘
// ---------------------------------------------------------------------------

#[test]
fn yili_gives_ju_only_after_leaving_dying() {
    let mut g = game(&[
        ("p0", "luji", &["yili"], "wu"),
        ("p1", "other", &[], "qun"),
    ]);
    skills::add_ju(&mut g, 0, 1); // 有橘可选，但我们选失血
    g.seats[0].hp = 1;
    // 给一张桃自救（手牌里 c3 是桃）
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend(&g).skill(), Some("yili"));
    answer(&mut g, 0, "choose:yili:hp");
    // 失去 1 点 → 濒死 → 先求桃
    assert!(g.seats[0].hp <= 0);
    assert_eq!(pend(&g).skill(), Some("dying"), "必须先完成濒死结算");
    let heal = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &heal);
    assert_eq!(g.seats[0].hp, 1);
    // 脱困后才询问给橘
    assert_eq!(pend(&g).skill(), Some("yili"));
    let target = pend(&g).options()[0].id.clone();
    let before = skills::ju(&g, 1);
    answer(&mut g, 0, &target);
    assert_eq!(skills::ju(&g, 1), before + 1);
}

// ---------------------------------------------------------------------------
// 立军：交杀后由孙亮二次选择
// ---------------------------------------------------------------------------

#[test]
fn lijun_requires_second_choice_by_lord() {
    let mut g = game(&[
        ("p0", "sunliang", &["lijun"], "wu"),
        ("p1", "wujiang", &[], "wu"),
    ]);
    g.seats[0].flags.insert("lord".to_string());
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let hand_before = g.hand(1).len();
    g.use_card(1, &"c4".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("lijun"));
    assert_eq!(pend(&g).actor(), 1, "先问使用者是否交出");
    // 没有「自动生效」：此时孙亮还没摸牌
    answer(&mut g, 1, "choose:lijun");
    assert!(g.hand(0).contains(&"c4".to_string()), "杀进入孙亮手牌");
    assert_eq!(pend(&g).actor(), 0, "第二段由孙亮决定");
    assert_eq!(g.hand(1).len(), hand_before - 1, "尚未摸牌");
    answer(&mut g, 0, "choose:lijun_bonus");
    assert_eq!(g.hand(1).len(), hand_before, "孙亮选择后才摸 1");
}

// ---------------------------------------------------------------------------
// 恃才：可选（原文含「可以」）
// ---------------------------------------------------------------------------

#[test]
fn shicai_is_optional_and_puts_card_on_top() {
    let mut g = game(&[
        ("p0", "xuyou", &["shicai"], "qun"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.use_card(0, &"c1".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("shicai"));
    assert_eq!(pend(&g).escape_id(), "pass", "必须有放弃出口");
    answer(&mut g, 0, "pass");
    assert!(g.hand(0).contains(&"c1".to_string()), "不发动时牌留在手里（未被置顶）");
}

// ---------------------------------------------------------------------------
// 成略：两面都在出牌阶段；弃牌花色成为自由花色
// ---------------------------------------------------------------------------

#[test]
fn chenglue_discard_suits_become_free() {
    let mut g = game(&[
        ("p0", "xuyou", &["chenglue"], "qun"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:chenglue").unwrap();
    answer(&mut g, 0, "choose:chenglue");
    // 阳：摸 1 弃 2
    let mut guard = 0;
    while g.pending.is_some() && guard < 8 {
        guard += 1;
        let id = pend(&g).options()[0].id.clone();
        answer(&mut g, 0, &id);
    }
    assert!(!g.yang(0, "chenglue"), "发动后转为阴");
    let free = ["phase.cl_spade", "phase.cl_heart", "phase.cl_club", "phase.cl_diamond"]
        .iter()
        .map(|k| g.counter(k))
        .sum::<u32>();
    assert!(free >= 2, "弃置的牌花色应记为自由花色");
}

// ---------------------------------------------------------------------------
// 整论：跳过摸牌阶段
// ---------------------------------------------------------------------------

#[test]
fn zhenglun_skips_draw_phase() {
    let mut g = game(&[
        ("p0", "luji", &["zhenglun"], "wu"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert_eq!(pend(&g).skill(), Some("zhenglun"));
    answer(&mut g, 0, "choose:zhenglun");
    assert_eq!(skills::ju(&g, 0), 1, "获得 1 枚橘");
    let before = g.hand(0).len();
    let drawn = g.draw_phase(0);
    assert!(drawn.is_empty() && g.hand(0).len() == before, "摸牌阶段被跳过");
}

// ---------------------------------------------------------------------------
// 溃诛：只算弃牌阶段
// ---------------------------------------------------------------------------

#[test]
fn kuizhu_only_on_discard_phase() {
    let mut g = game(&[
        ("p0", "sunliang", &["kuizhu"], "wu"),
        ("p1", "other", &[], "qun"),
    ]);
    // 出牌阶段结束：不触发
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.end_phase("Play").unwrap();
    assert!(g.pending.is_none(), "出牌阶段结束不触发溃诛");
    // 弃牌阶段弃 1 张 → 结束时触发
    begin_phase(&mut g, 0, Phase::Discard).unwrap();
    let id = g.hand(0)[0].clone();
    g.to_discard(&id, "discard_phase").unwrap();
    g.end_phase("Discard").unwrap();
    assert_eq!(pend(&g).skill(), Some("kuizhu"));
}

// ---------------------------------------------------------------------------
// 伪帝：视为拥有当前主公的主公技
// ---------------------------------------------------------------------------

#[test]
fn weidi_sees_lord_skill_but_is_not_usable() {
    let mut g = game(&[
        ("p0", "yuanshu", &["weidi"], "qun"),
        ("p1", "caopi", &["songwei"], "wei"),
    ]);
    g.seats[1].flags.insert("lord".to_string());
    assert!(skills::active(&g, 0, "songwei"), "伪帝视为拥有主公技");
    assert!(!skills::active(&g, 1, "weidi"));
    assert!(
        !skills::legal_actions(&g, 0).iter().any(|(id, _)| id.starts_with("skill:")),
        "借来的主公技不可主动使用"
    );
}

// ---------------------------------------------------------------------------
// 飞军 + 兵略
// ---------------------------------------------------------------------------

#[test]
fn feijun_takes_card_and_binglue_draws_two() {
    let mut g = game(&[
        ("p0", "wangping", &["feijun", "binglue"], "shu"),
        ("p1", "rich", &[], "qun"),
    ]);
    // p1 手牌多于 p0：多给 2 张
    g.to_hand(&"c10".to_string(), 1).unwrap();
    g.to_hand(&"c11".to_string(), 1).unwrap();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let legal: Vec<String> = skills::legal_actions(&g, 0)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(legal.contains(&"skill:feijun".to_string()));
    skills::execute_active(&mut g, 0, "skill:feijun").unwrap();
    answer(&mut g, 0, "choose:feijun:take");
    let target = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &target);
    let cost = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &cost);
    assert_eq!(g.counter("phase.feijun_used"), 1);
    // 弃 1 + 获得 1 + 兵略摸 2 → 净 +2
    assert!(g.hand(0).len() >= 5, "hand={}", g.hand(0).len());
}

// ---------------------------------------------------------------------------
// 审时（阳）：交牌 + 1 点伤害
// ---------------------------------------------------------------------------

#[test]
fn shenshi_yang_gives_card_then_damages() {
    let mut g = game(&[
        ("p0", "kuaiyue", &["shenshi"], "qun"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert!(skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:shenshi"));
    skills::execute_active(&mut g, 0, "skill:shenshi").unwrap();
    let target = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &target);
    let card = pend(&g).options()[0].id.clone();
    let hp = g.seats[1].hp;
    answer(&mut g, 0, &card);
    assert_eq!(g.seats[1].hp, hp - 1, "交牌后造成 1 点伤害");
    assert!(!g.yang(0, "shenshi"), "发动后转阴");
}

// ---------------------------------------------------------------------------
// 掣政：出牌阶段结束时按使用牌数判定
// ---------------------------------------------------------------------------

#[test]
fn chezheng_triggers_when_few_cards_used() {
    let mut g = game(&[
        ("p0", "sunliang", &["chezheng"], "wu"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 本阶段用了 0 张牌 < 攻击范围外角色数（1）→ 触发
    g.end_phase("Play").unwrap();
    assert_eq!(pend(&g).skill(), Some("chezheng"));
    let target = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &target);
    assert!(g.discard_pile().len() >= 1, "应弃置目标一张牌");
}

#[test]
fn chezheng_skips_when_enough_cards_used() {
    let mut g = game(&[
        ("p0", "sunliang", &["chezheng"], "wu"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.bump("turn.play_cards_used"); // 1 >= 1
    g.end_phase("Play").unwrap();
    assert!(g.pending.is_none(), "使用牌数足够时不触发");
}

// ---------------------------------------------------------------------------
// 荐降：成为目标后令手牌最少者摸 1（并列由本人选）
// ---------------------------------------------------------------------------

#[test]
fn jianxiang_triggers_only_when_becoming_target() {
    let mut g = game(&[
        ("p0", "kuaiyue", &["jianxiang"], "qun"),
        ("p1", "other", &[], "qun"),
    ]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    // 不是目标 → 不触发
    g.use_card(1, &"c4".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "不是目标时不触发荐降");
    // 成为目标 → 触发
    g.use_card(1, &"c5".to_string(), vec![0]);
    advance(&mut g).unwrap();
    let p = pend(&g);
    assert_eq!(p.skill(), Some("jianxiang"));
    assert_eq!(p.actor(), 0, "由蒯越蒯良选择并列者");
    let target = p.options()[0].id.clone();
    answer(&mut g, 0, &target);
    assert!(g.pending.is_none());
}
