//! B1 迁移门禁（风林火山·SP-1：18 技能）。
//!
//! 每个技能至少覆盖 B1.md「断言」的**正例 + 反例**（对齐单是权威口径），
//! 并覆盖 4 条机制级不变量：转换技 face 路由、单一状态源（伪帝借用）、
//! 数值钩子单出口（摸牌加成/方向/防伤/牌无效）、事件路由（窗口 + subject）。

use sanguosha3::kernel::event::{Event, EventKind};
use sanguosha3::kernel::resolve::{self, advance, Ask, Stage, Step};
use sanguosha3::kernel::skills::{self, b1};
use sanguosha3::kernel::{
    Card, DamageKind, DamageSpec, Game, Identity, Phase, Seat, SourceKind, Suit, Zone,
};

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn seat_with(id: usize, skills: &[&str], kingdom: &str) -> Seat {
    Seat {
        id,
        hp: 4,
        max_hp: 4,
        alive: true,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        kingdom: kingdom.to_string(),
        ..Default::default()
    }
}

/// 3 座对局：座位 0 带指定技能。
fn game(skills: &[&str]) -> Game {
    Game {
        seats: vec![
            seat_with(0, skills, "wei"),
            seat_with(1, &[], "shu"),
            seat_with(2, &[], "qun"),
        ],
        ..Default::default()
    }
}

fn add_card(g: &mut Game, id: &str, def: &str, zone: Zone) {
    g.cards.insert(id.to_string(), zone);
    g.card_defs.insert(
        id.to_string(),
        Card {
            id: id.to_string(),
            def: def.to_string(),
            suit: Suit::Spade,
            rank: 7,
        },
    );
}

/// 牌堆（末尾 = 顶）：`ids` 顺序 = 从底到顶。
fn add_deck(g: &mut Game, ids: &[&str]) {
    for id in ids {
        add_card(g, id, "sha", Zone::Draw);
        g.deck.push(id.to_string());
    }
}

fn run(g: &mut Game, skill: &str, actor: usize, data: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler 应存在");
    h(g, &b1::ask(skill, actor, data)).expect("handler 不应报错")
}


/// 以「真实待办」方式驱动技能帧：首帧拿 pending，再逐个回答；回答写回
/// `g.answer/answer_skill`（与 `resolve::submit` 同语义，但不依赖栈/回合帧）。
fn run_steps(
    g: &mut Game,
    skill: &str,
    actor: usize,
    data: serde_json::Value,
    answers: &[&str],
) -> Step {
    let mut data = data;
    let mut step = call(g, skill, actor, data.clone());
    for ans in answers {
        // 推进到待办：`Next` 继续跑；`NeedWith` 自带新 data（同一 pending 直接回答）。
        loop {
            match &step {
                Step::Need(_) | Step::NeedAt(_, _) => break,
                Step::NeedWith(_, d, _) => {
                    data = d.clone();
                    break;
                }
                Step::Next(_, d) => {
                    data = d.clone();
                    step = call(g, skill, actor, data.clone());
                }
                other => panic!("期望待办，实际 {other:?}"),
            }
        }
        let pending = match &step {
            Step::Need(p) | Step::NeedAt(_, p) => p.clone(),
            Step::NeedWith(_, _, p) => p.clone(),
            _ => unreachable!(),
        };
        let opts: Vec<String> = pending.options().iter().map(|o| o.id.clone()).collect();
        assert!(pending.has_option(ans), "选项 {ans} 不在 {opts:?}");
        g.answer = Some(ans.to_string());
        g.answer_skill = Some(skill.to_string());
        // `Need`/`NeedAt` 不带 data：沿用 pending.data；`NeedWith` 已在上面更新 data。
        if !matches!(step, Step::NeedWith(_, _, _)) {
            data = pending.data().clone();
        }
        step = call(g, skill, actor, data.clone());
    }
    // 收尾：只推进 `Next`（`Need*` = 还需要玩家输入，直接返回）。
    loop {
        match &step {
            Step::Next(_, d) => {
                data = d.clone();
                step = call(g, skill, actor, data.clone());
            }
            _ => break,
        }
    }
    step
}

fn call(g: &mut Game, skill: &str, actor: usize, data: serde_json::Value) -> Step {
    let h = skills::handler(skill).expect("handler 应存在");
    h(g, &b1::ask(skill, actor, data)).expect("handler 不应报错")
}

fn hand(g: &Game, seat: usize) -> Vec<String> {
    g.hand(seat)
}

fn hand_size(g: &Game, seat: usize) -> usize {
    g.hand_size(seat)
}

// ---------------------------------------------------------------------------
// 注册表 / 声明
// ---------------------------------------------------------------------------

#[test]
fn b1_all_handlers_registered() {
    for c in [
        "yongsi", "weidi", "qizhi", "jinzou", "juzhan", "feijun", "binglue", "huaiju", "yili",
        "zhenglun", "kuizhu", "chezheng", "lijun", "jianxiang", "shenshi", "chenglue", "shicai",
        "cunmu",
    ] {
        assert!(skills::handler(c).is_some(), "{c} 应有 handler");
        assert!(skills::def(c).is_some(), "{c} 应有声明");
    }
}

#[test]
fn transform_skills_split_faces_with_own_windows() {
    for c in ["juzhan", "shenshi", "chenglue"] {
        let d = skills::def(c).unwrap();
        assert!(d.transform, "{c} 是转换技");
        assert_eq!(d.faces.len(), 2, "{c} 必须拆两面");
    }
    // 非转换技不得误标（否则路由只读第一面）。
    for c in ["yongsi", "qizhi", "jinzou", "feijun", "huaiju", "shicai"] {
        assert!(!skills::def(c).unwrap().transform, "{c} 不是转换技");
    }
    assert!(skills::check_transform_faces().is_ok());
}

#[test]
fn weidi_sees_lord_skill_but_is_not_usable() {
    let mut g = Game {
        seats: vec![
            seat_with(0, &["songwei"], "wei"),
            seat_with(1, &["weidi"], "qun"),
            seat_with(2, &[], "wu"),
        ],
        ..Default::default()
    };
    g.seats[0].identity = Some(Identity::Lord);
    // 视为拥有当前主公的主公技（参与联动判定）
    assert!(skills::active(&g, 1, "songwei"), "伪帝视为拥有主公的主公技");
    // 但不可主动使用（legal 不出现）：借来的不算 owned
    assert!(!skills::can_active(&g, 1, "songwei"), "借来的主公技不可主动使用");
    // 只借主公技：非主公技不借
    assert!(!skills::active(&g, 1, "yongsi"));
    // 主公死亡后不再借
    g.seats[0].alive = false;
    assert!(!skills::active(&g, 1, "songwei"));
}

// ---------------------------------------------------------------------------
// 庸肆（锁定）：摸牌 +X / 弃牌阶段先弃 X / 计入本阶段弃牌数
// ---------------------------------------------------------------------------

#[test]
fn yongsi_draw_bonus_equals_kingdom_count() {
    let mut g = game(&["yongsi"]);
    add_deck(&mut g, &["c0", "c1", "c2", "c3", "c4", "c5"]);
    assert_eq!(g.kingdom_count(), 3, "魏/蜀/群 = 3");
    assert_eq!(b1::draw_bonus(&g, 0), 3);
    let drawn = sanguosha3::kernel::flow::draw_phase(&mut g, 0);
    assert_eq!(drawn.len(), 5, "2 + X(=3)");
}

#[test]
fn yongsi_discard_phase_discards_x_and_counts_phase_discard() {
    let mut g = game(&["yongsi"]);
    g.phase = Phase::Discard;
    for i in 0..5 {
        add_card(&mut g, &format!("h{i}"), "sha", Zone::Hand(0));
    }
    // 锁定技：必须弃 X=3 张，但**弃哪 3 张由持有者自选**（逐张询问）。
    run_steps(&mut g, "yongsi", 0, b1::empty(), &["card:h0", "card:h1", "card:h2"]);
    assert_eq!(hand_size(&g, 0), 2, "先弃 X=3 张");
    assert_eq!(g.counter("phase.discarded.Discard"), 3, "计入本阶段弃牌数（溃诛 X）");
    // 只做一次
    run(&mut g, "yongsi", 0, b1::empty());
    assert_eq!(hand_size(&g, 0), 2);
}

#[test]
fn yongsi_discard_discards_all_when_short() {
    let mut g = game(&["yongsi"]);
    g.phase = Phase::Discard;
    add_card(&mut g, "h0", "sha", Zone::Hand(0));
    run_steps(&mut g, "yongsi", 0, b1::empty(), &["card:h0"]);
    assert_eq!(hand_size(&g, 0), 0, "不足则弃光");
}

// ---------------------------------------------------------------------------
// 奇制 / 进趋
// ---------------------------------------------------------------------------

#[test]
fn qizhi_triggers_on_targeted_card_use() {
    let mut g = game(&["qizhi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_deck(&mut g, &["d0"]);
    add_card(&mut g, "t0", "sha", Zone::Hand(1));
    // 直接路由「指定目标」事件（Subject::By + OwnTurn）。
    let mut ev = Event::card(
        EventKind::TargetChosen,
        Some(1),
        "u0".to_string(),
        Some("目标".into()),
    );
    ev.by = Some(0);
    ev.turn = 0;
    ev.phase = Phase::Play;
    g.events.push(ev);
    advance(&mut g).unwrap();
    // 真实待办：先问「发动/放弃」，再强制选牌（不得代选）。
    assert_eq!(
        g.pending.as_ref().and_then(|p| p.skill()),
        Some("qizhi"),
        "奇制必须询问"
    );
    resolve::submit(&mut g, 0, "use").unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.kind().to_string()), Some("pick_other".into()));
    resolve::submit(&mut g, 0, "card:t0").unwrap();
    assert!(g.cards.get("t0") == Some(&Zone::Discard), "弃置目标一张牌");
    assert_eq!(hand_size(&g, 1), 1, "然后令其摸一张");
    assert_eq!(g.counter("turn.0.qizhi"), 1, "奇制次数 +1（进趋 X）");
}

#[test]
fn qizhi_does_not_trigger_without_target() {
    let mut g = game(&["qizhi"]);
    g.turn = 0;
    add_card(&mut g, "t0", "sha", Zone::Hand(1));
    // 无目标（无 TargetChosen 事件）
    run(&mut g, "qizhi", 0, b1::empty());
    assert_eq!(g.counter("turn.0.qizhi"), 0);
    assert!(g.cards.get("t0") == Some(&Zone::Hand(1)));
}

#[test]
fn jinzou_not_allowed_when_qizhi_count_is_zero() {
    let mut g = game(&["jinzou"]);
    g.phase = Phase::End;
    g.turn = 0;
    add_deck(&mut g, &["d0", "d1"]);
    // X=0：不提供主动入口（B1 Q6 裁定 B）
    assert!(skills::active_actions(&g, 0).is_empty());
    assert!(skills::use_active(&mut g, 0, "jinzou").is_err());
    assert_eq!(hand_size(&g, 0), 0, "X=0 不允许发动（不摸牌）");
}

#[test]
fn jinzou_draws_then_discards_down_to_x() {
    let mut g = game(&["jinzou"]);
    g.phase = Phase::End;
    g.turn = 0;
    g.set_counter("turn.0.qizhi", 2);
    for i in 0..3 {
        add_card(&mut g, &format!("h{i}"), "sha", Zone::Hand(0));
    }
    add_deck(&mut g, &["d0", "d1"]);
    assert!(skills::active_actions(&g, 0).contains(&"jinzou"));
    skills::use_active(&mut g, 0, "jinzou").unwrap();
    assert!(g.pending.is_some(), "可选技能须显式询问（I1）");
    resolve::submit(&mut g, 0, "use").unwrap();
    // 弃至 X 张：弃哪几张由王基自选（逐张询问）。
    while hand_size(&g, 0) > 2 {
        let p = g.pending.clone().expect("应询问弃哪张");
        let id = p.options().iter().find(|o| o.id.starts_with("card:")).unwrap().id.clone();
        resolve::submit(&mut g, 0, &id).unwrap();
    }
    assert_eq!(hand_size(&g, 0), 2, "摸 2 后弃至 X=2");
    assert_eq!(g.counter("phase.discarded.End"), 3, "弃 3 张（5→2）");
}

// ---------------------------------------------------------------------------
// 拒战（转换技）：阳 = 他人回合，阴 = 自己回合
// ---------------------------------------------------------------------------

#[test]
fn juzhan_yang_only_on_others_turn() {
    let mut g = game(&["juzhan"]);
    g.turn = 2; // 他人回合
    add_deck(&mut g, &["d0", "d1"]);
    let step = run_steps(
        &mut g,
        "juzhan",
        0,
        b1::data(&[("face", 0.into()), ("by", 1.into()), ("step", "ask".into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(hand_size(&g, 0), 1, "自己摸 1");
    assert_eq!(hand_size(&g, 1), 1, "来源摸 1");
    assert!(b1::deny_target(&g, 1, 0), "来源本回合不能再对你使用牌");
    assert_eq!(skills::face_of(&g, 0, "juzhan"), 1, "发动后转换面");
}

#[test]
fn juzhan_yin_takes_card_but_not_judge_zone() {
    let mut g = game(&["juzhan"]);
    g.turn = 0;
    add_card(&mut g, "e0", "sha", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add_card(&mut g, "j0", "lebu_sishu", Zone::Judge(1));
    skills::flip_face(&mut g, 0, "juzhan");
    let step = run_steps(
        &mut g,
        "juzhan",
        0,
        b1::data(&[("face", 1.into()), ("target", 1.into()), ("step", "ask".into())]),
        &["use", "card:e0"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("e0"), Some(&Zone::Hand(0)), "获得装备牌");
    assert_eq!(g.cards.get("j0"), Some(&Zone::Judge(1)), "不含判定区（Q8 裁定 B）");
    assert!(b1::deny_target(&g, 0, 1));
    assert_eq!(skills::face_of(&g, 0, "juzhan"), 0, "发动后转换面");
}

#[test]
fn juzhan_face_routing_rejects_wrong_window() {
    let mut g = game(&["juzhan"]);
    g.turn = 0; // 自己的回合
    add_deck(&mut g, &["d0", "d1"]);
    // 阳面 = 他人回合，自己的回合不触发。
    let mut ev = Event::card(EventKind::TargetChosen, Some(0), "c0".to_string(), None);
    ev.by = Some(1);
    ev.turn = 0;
    ev.phase = Phase::Play;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert_eq!(hand_size(&g, 0), 0, "阳面不在自己回合触发");
    assert_eq!(skills::face_of(&g, 0, "juzhan"), 0, "未发动不转面");

    // 阴面（By + OwnTurn）应触发，但要先询问发动。
    skills::flip_face(&mut g, 0, "juzhan");
    add_card(&mut g, "c1", "sha", Zone::Hand(1));
    let mut ev = Event::card(EventKind::TargetChosen, Some(1), "c1".to_string(), None);
    ev.by = Some(0);
    ev.turn = 0;
    ev.phase = Phase::Play;
    g.events.push(ev);
    advance(&mut g).unwrap();
    assert_eq!(g.pending.as_ref().and_then(|p| p.skill()), Some("juzhan"));
    resolve::submit(&mut g, 0, "use").unwrap();
    resolve::submit(&mut g, 0, "card:c1").unwrap();
    assert_eq!(g.cards.get("c1"), Some(&Zone::Hand(0)), "获得目标一张牌");
}

// ---------------------------------------------------------------------------
// 飞军 / 兵略
// ---------------------------------------------------------------------------

#[test]
fn feijun_takes_card_and_binglue_draws_two_once() {
    let mut g = game(&["feijun", "binglue"]);
    g.phase = Phase::Play;
    g.turn = 0;
    add_card(&mut g, "own", "sha", Zone::Hand(0));
    add_card(&mut g, "t0", "tao", Zone::Hand(1));
    add_card(&mut g, "t1", "shan", Zone::Hand(1));
    add_deck(&mut g, &["d0", "d1", "d2", "d3"]);
    skills::use_active(&mut g, 0, "feijun").unwrap();
    resolve::submit(&mut g, 0, "use:1").unwrap(); // 选项①
    resolve::submit(&mut g, 0, "card:own").unwrap(); // 代价：弃自己一张
    resolve::submit(&mut g, 0, "seat:1").unwrap(); // 目标
    // ①目标**自选**一张手牌 → pending 归属 = 目标（不是王平）
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    resolve::submit(&mut g, 1, "card:t1").unwrap();
    assert!(hand(&g, 0).contains(&"t1".to_string()), "目标自选一张手牌交给你");
    assert_eq!(hand_size(&g, 0), 3, "得 1 张 + 兵略首次摸 2");
    assert!(b1::binglue_used(&g, 0, 1));
    // 出牌阶段限一次
    assert!(skills::use_active(&mut g, 0, "feijun").is_err());
}

#[test]
fn feijun_opt2_discards_target_equip_chosen_by_wangping() {
    let mut g = game(&["feijun"]);
    g.phase = Phase::Play;
    g.turn = 0;
    add_card(&mut g, "own", "sha", Zone::Hand(0));
    add_card(&mut g, "w", "qinggang_jian", Zone::Equip(1, sanguosha3::kernel::Slot::Weapon));
    add_card(&mut g, "a", "bagua_zhen", Zone::Equip(1, sanguosha3::kernel::Slot::Armor));
    skills::use_active(&mut g, 0, "feijun").unwrap();
    resolve::submit(&mut g, 0, "use:2").unwrap();
    resolve::submit(&mut g, 0, "card:own").unwrap();
    resolve::submit(&mut g, 0, "seat:1").unwrap();
    // ②弃哪件装备**由王平选择**（Q10 裁定 A）→ pending 归属 = 王平
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    resolve::submit(&mut g, 0, "card:a").unwrap();
    assert_eq!(g.cards.get("a"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("w"), Some(&Zone::Equip(1, sanguosha3::kernel::Slot::Weapon)));
}

// ---------------------------------------------------------------------------
// 怀橘 / 遗礼 / 整论
// ---------------------------------------------------------------------------

#[test]
fn huaiju_game_start_three_ju_extra_draw_and_prevent_damage() {
    let mut g = Game {
        seats: vec![seat_with(0, &["huaiju"], "wu"), seat_with(1, &[], "wu")],
        ..Default::default()
    };
    add_deck(&mut g, &["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "c10", "c11", "c12", "c13", "c14", "c15"]);
    sanguosha3::kernel::turn::begin_game(&mut g, &[Identity::Rebel], 4);
    assert_eq!(b1::ju(&g, 0), 3, "开局 3 枚橘（计数标记）");
    // 摸牌阶段 +1
    let before = hand_size(&g, 0);
    let drawn = sanguosha3::kernel::flow::draw_phase(&mut g, 0);
    assert_eq!(drawn.len(), 3, "2 + 怀橘 1");
    assert_eq!(hand_size(&g, 0), before + 3);
    // 自动防伤（不询问）
    let spec = DamageSpec {
        from: Some(1),
        target: 0,
        amount: 2,
        kind: DamageKind::Normal,
        source: SourceKind::Slash,
    };
    let evs = sanguosha3::kernel::flow::damage(&mut g, spec, sanguosha3::kernel::ArmorKind::None, 0);
    assert!(evs.is_empty(), "有橘：防止该次伤害");
    assert_eq!(g.seats[0].hp, 4, "体力未变（伤害被防止）");
    assert_eq!(b1::ju(&g, 0), 2);
    // 橘用尽后不再防伤
    b1::remove_ju(&mut g, 0, 2);
    let spec2 = DamageSpec { amount: 1, ..spec };
    let evs = sanguosha3::kernel::flow::damage(&mut g, spec2, sanguosha3::kernel::ArmorKind::None, 0);
    assert_eq!(evs.len(), 1, "橘为 0 时正常结算伤害");
    assert_eq!(g.seats[0].hp, 3);
}

#[test]
fn yili_gives_ju_only_after_leaving_dying() {
    let mut g = game(&["yili"]);
    g.phase = Phase::Play;
    g.turn = 0;
    g.seats[0].hp = 1;
    add_card(&mut g, "tao1", "tao", Zone::Hand(1)); // 座位 1 有桃可救
    skills::use_active(&mut g, 0, "yili").unwrap();
    resolve::submit(&mut g, 0, "cost:hp").unwrap();
    resolve::submit(&mut g, 0, "seat:1").unwrap();
    assert_eq!(g.seats[0].hp, 0, "失去体力至濒死");
    assert_eq!(b1::ju(&g, 1), 0, "濒死未救回前不给橘");
    // 濒死救援链：座位 0 无桃 → 放弃；座位 1 出桃
    resolve::submit(&mut g, 0, "pass").unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(1));
    resolve::submit(&mut g, 1, "card:tao1").unwrap();
    assert_eq!(g.seats[0].hp, 1, "救回");
    // 延迟队列（SkillTrigger）在濒死流程之后派发 → 补发「橘」
    assert_eq!(b1::ju(&g, 1), 1, "救回后目标获得 1 枚橘");
}

#[test]
fn yili_can_pay_with_ju() {
    let mut g = game(&["yili"]);
    g.phase = Phase::Play;
    g.turn = 0;
    b1::add_ju(&mut g, 0, 1);
    skills::use_active(&mut g, 0, "yili").unwrap();
    resolve::submit(&mut g, 0, "cost:ju").unwrap();
    resolve::submit(&mut g, 0, "seat:1").unwrap();
    assert_eq!(b1::ju(&g, 0), 0, "代价移除 1 枚橘");
    assert_eq!(b1::ju(&g, 1), 1);
}

#[test]
fn zhenglun_skips_whole_draw_phase() {
    let mut g = game(&["zhenglun"]);
    g.phase = Phase::Draw;
    g.turn = 0;
    add_deck(&mut g, &["d0", "d1", "d2"]);
    assert!(skills::active_actions(&g, 0).contains(&"zhenglun"));
    skills::use_active(&mut g, 0, "zhenglun").unwrap();
    resolve::submit(&mut g, 0, "use").unwrap();
    assert_eq!(b1::ju(&g, 0), 1, "获得 1 枚橘");
    assert!(sanguosha3::kernel::flow::draw_phase(&mut g, 0).is_empty(), "整段跳过摸牌");
    // 反例：有橘时不提供入口
    let mut g2 = game(&["zhenglun"]);
    g2.phase = Phase::Draw;
    g2.turn = 0;
    b1::add_ju(&mut g2, 0, 1);
    assert!(!skills::active_actions(&g2, 0).contains(&"zhenglun"));
    add_deck(&mut g2, &["d0", "d1"]);
    assert_eq!(sanguosha3::kernel::flow::draw_phase(&mut g2, 0).len(), 2, "有橘时正常摸牌");
}

// ---------------------------------------------------------------------------
// 溃诛 / 掣政 / 立军
// ---------------------------------------------------------------------------

#[test]
fn kuizhu_draw_mode_uses_phase_discard_count() {
    let mut g = game(&["kuizhu"]);
    g.phase = Phase::Discard;
    g.set_counter("phase.discarded.Discard", 2);
    g.set_counter("phase.discarded.Play", 5); // 其他阶段弃牌不纳入（Q1/Q16）
    add_deck(&mut g, &["d0", "d1"]);
    // 模式①：至多 X=2 名，选 2 名。
    let step = run_steps(
        &mut g,
        "kuizhu",
        0,
        b1::data(&[("step", "mode".into())]),
        &["use:1", "t:1", "t:2", "ok"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(hand_size(&g, 1), 1);
    assert_eq!(hand_size(&g, 2), 1);
    // 反例：X=1 时选满 1 名后不再提供新的目标选项（UI 层封顶，而不是报错）。
    let mut g = game(&["kuizhu"]);
    g.phase = Phase::Discard;
    g.set_counter("phase.discarded.Discard", 1);
    let step = run_steps(
        &mut g,
        "kuizhu",
        0,
        b1::data(&[("step", "mode".into())]),
        &["use:1", "t:1"],
    );
    let opts = match &step {
        Step::Need(p) | Step::NeedAt(_, p) => {
            p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
        }
        other => panic!("期望继续待办，实际 {other:?}"),
    };
    assert!(!opts.iter().any(|o| o.starts_with("t:")), "已达 X 上限");
    assert!(opts.contains(&"ok".to_string()));
}

#[test]
fn kuizhu_damage_mode_requires_hp_sum_equal_x() {
    let mut g = game(&["kuizhu"]);
    g.phase = Phase::Discard;
    g.set_counter("phase.discarded.Discard", 6);
    g.seats[1].hp = 3;
    g.seats[2].hp = 3;
    // 组合由系统枚举：唯一满足 3+3=6 的是 {1,2}。
    let step = run_steps(
        &mut g,
        "kuizhu",
        0,
        b1::data(&[("step", "mode".into())]),
        &["use:2", "combo:0"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.seats[1].hp, 2, "造成 1 点伤害");
    assert_eq!(g.seats[2].hp, 2);
    // 反例：没有任何组合等于 X 时不提供模式②。
    let mut g = game(&["kuizhu"]);
    g.phase = Phase::Discard;
    g.set_counter("phase.discarded.Discard", 99);
    let step = call(&mut g, "kuizhu", 0, b1::data(&[("step", "mode".into())]));
    let opts = match &step {
        Step::Need(p) | Step::NeedAt(_, p) => {
            p.options().iter().map(|o| o.id.clone()).collect::<Vec<_>>()
        }
        other => panic!("期望待办，实际 {other:?}"),
    };
    assert!(!opts.contains(&"use:2".to_string()), "无组合不提供模式②");
}

#[test]
fn chezheng_makes_short_range_cards_ineffective() {
    // 4 人局：座位 2 攻击范围 1，不含座位 0 → 对超出范围者使用牌无效。
    let mut g = Game {
        seats: vec![
            seat_with(0, &["chezheng"], "wu"),
            seat_with(1, &[], "wu"),
            seat_with(2, &[], "wu"),
            seat_with(3, &[], "wu"),
        ],
        ..Default::default()
    };
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "c0", "sha", Zone::Hand(2));
    assert!(!b1::in_attack_range(&g, 2, 0));
    assert!(skills::card_ineffective(&g, 2, 0));
    // ① 越距使用无效：不产生伤害（但仍算使用过牌）。
    sanguosha3::kernel::card_play::use_on_targets(&mut g, 2, "c0", &[0]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4, "越距牌无效：无伤害");
    assert_eq!(g.counter("turn.2.cards_used"), 1, "仍然算使用过");
    // ② 出牌阶段结束：0 < 范围外角色数 → 询问发动 → 选目标 → 强制选牌。
    g.set_counter("turn.0.cards_used", 0);
    add_card(&mut g, "h2", "tao", Zone::Hand(2));
    let step = run_steps(
        &mut g,
        "chezheng",
        0,
        b1::data(&[("step", "ask".into())]),
        &["use", "seat:2", "card:h2"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("h2"), Some(&Zone::Discard), "弃范围外角色一张牌");
}

#[test]
fn chezheng_skips_when_enough_cards_used() {
    let mut g = Game {
        seats: vec![
            seat_with(0, &["chezheng"], "wu"),
            seat_with(1, &[], "wu"),
            seat_with(2, &[], "wu"),
            seat_with(3, &[], "wu"),
        ],
        ..Default::default()
    };
    g.turn = 0;
    g.phase = Phase::Play;
    g.set_counter("turn.0.cards_used", 9);
    add_card(&mut g, "h2", "tao", Zone::Hand(2));
    run(&mut g, "chezheng", 0, b1::data(&[("target", 2.into())]));
    assert_eq!(g.cards.get("h2"), Some(&Zone::Hand(2)), "使用牌数足够时不触发");
}

#[test]
fn lijun_other_wu_slash_gives_card_then_lord_may_bonus() {
    let mut g = Game {
        seats: vec![
            seat_with(0, &["lijun"], "wu"),
            seat_with(1, &[], "wu"),
            seat_with(2, &[], "shu"),
        ],
        ..Default::default()
    };
    g.seats[0].identity = Some(Identity::Lord);
    g.phase = Phase::Play;
    g.turn = 1;
    add_card(&mut g, "s0", "sha", Zone::Processing);
    add_deck(&mut g, &["d0"]);
    // 两个「可以」：吴角色先选择是否交牌，孙亮再选择是否给加成。
    let step = run_steps(
        &mut g,
        "lijun",
        0,
        b1::data(&[("step", "give".into()), ("by", 1.into()), ("card", "s0".into())]),
        &["use", "use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("s0"), Some(&Zone::Hand(0)), "杀交给孙亮（Q18 裁定 A）");
    assert_eq!(hand_size(&g, 1), 1, "其摸一张");
    assert_eq!(b1::sha_extra(&g, 1), 1, "本回合【杀】次数 +1");
    // 每回合限一次：第二次直接不询问。
    add_card(&mut g, "s1", "sha", Zone::Processing);
    let step = call(
        &mut g,
        "lijun",
        0,
        b1::data(&[("step", "give".into()), ("by", 1.into()), ("card", "s1".into())]),
    );
    assert!(matches!(step, Step::Done));
    assert_eq!(g.cards.get("s1"), Some(&Zone::Processing), "每回合限一次");
}

// ---------------------------------------------------------------------------
// 荐降 / 审时（转换技）
// ---------------------------------------------------------------------------

#[test]
fn jianxiang_targets_min_hand_and_owner_breaks_tie() {
    let mut g = game(&["jianxiang"]);
    g.turn = 2;
    add_card(&mut g, "x0", "sha", Zone::Hand(0));
    add_card(&mut g, "x1", "sha", Zone::Hand(0));
    add_card(&mut g, "a", "sha", Zone::Hand(1));
    add_card(&mut g, "b", "sha", Zone::Hand(2));
    add_deck(&mut g, &["d0"]);
    // 并列最少（座位 1、2 各 1 张）→ 由蒯越蒯良选择（Q19 裁定 B）。
    let step = run_steps(
        &mut g,
        "jianxiang",
        0,
        b1::data(&[("step", "ask".into()), ("by", 2.into())]),
        &["use", "seat:2"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(hand_size(&g, 2), 2, "并列时由技能拥有者选择");
    assert_eq!(hand_size(&g, 1), 1);
}

#[test]
fn jianxiang_not_triggered_by_own_card() {
    let mut g = game(&["jianxiang"]);
    add_deck(&mut g, &["d0"]);
    run(&mut g, "jianxiang", 0, b1::data(&[("by", 0.into())]));
    assert_eq!(hand_size(&g, 1), 0, "自己使用牌不触发");
}

#[test]
fn shenshi_yang_gives_card_then_damages_and_flips() {
    let mut g = game(&["shenshi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "mine", "sha", Zone::Hand(0));
    add_card(&mut g, "t1", "sha", Zone::Hand(1));
    skills::use_active(&mut g, 0, "shenshi").unwrap();
    resolve::submit(&mut g, 0, "use").unwrap();
    resolve::submit(&mut g, 0, "seat:1").unwrap();
    assert_eq!(g.pending.as_ref().map(|p| p.actor()), Some(0));
    resolve::submit(&mut g, 0, "card:mine").unwrap();
    assert_eq!(hand_size(&g, 0), 0, "交给其一张牌");
    assert_eq!(hand_size(&g, 1), 2);
    assert_eq!(g.seats[1].hp, 3, "造成 1 点伤害");
    assert_eq!(skills::face_of(&g, 0, "shenshi"), 1, "发动后转阴面");
    // 阴面不是主动技（且本阶段已限一次）
    assert!(!skills::active_actions(&g, 0).contains(&"shenshi"));
}

#[test]
fn shenshi_yin_records_card_and_end_check_fills_to_four() {
    let mut g = game(&["shenshi"]);
    g.turn = 1; // 回合外
    g.phase = Phase::Play;
    add_card(&mut g, "mine", "sha", Zone::Hand(0));
    add_deck(&mut g, &["d0", "d1", "d2", "d3"]);
    skills::flip_face(&mut g, 0, "shenshi"); // 阴面
    let step = run_steps(
        &mut g,
        "shenshi",
        0,
        b1::data(&[("step", "ask".into()), ("face", 1.into()), ("target", 1.into())]),
        &["use", "card:mine"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(g.cards.get("mine"), Some(&Zone::Hand(1)), "交给其一张牌");
    // 未到结束阶段：TurnEndCheck 保留在队列，不提前派发。
    advance(&mut g).unwrap();
    assert_eq!(hand_size(&g, 0), 0, "非结束阶段不结算");
    // 结束阶段 → 可派发 → 摸至 4 张。
    g.phase = Phase::End;
    advance(&mut g).unwrap();
    assert_eq!(hand_size(&g, 0), 4, "其未失去此牌 → 摸至 4 张");
}

#[test]
fn shenshi_end_check_skips_when_card_lost() {
    let mut g = game(&["shenshi"]);
    g.turn = 1;
    g.phase = Phase::Play;
    add_card(&mut g, "mine", "sha", Zone::Hand(0));
    add_deck(&mut g, &["d0", "d1", "d2", "d3"]);
    skills::flip_face(&mut g, 0, "shenshi");
    run(&mut g, "shenshi", 0, b1::data(&[("face", 1.into()), ("target", 1.into())]));
    // 其失去此牌
    g.cards.insert("mine".into(), Zone::Discard);
    g.phase = Phase::End;
    advance(&mut g).unwrap();
    assert_eq!(hand_size(&g, 0), 0, "未摸牌");
}

// ---------------------------------------------------------------------------
// 成略（转换技）/ 恃才 / 寸目
// ---------------------------------------------------------------------------

#[test]
fn chenglue_faces_draw_discard_and_free_suits() {
    let mut g = game(&["chenglue"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "a", "sha", Zone::Hand(0));
    add_card(&mut g, "b", "tao", Zone::Hand(0));
    add_deck(&mut g, &["d0"]);
    skills::use_active(&mut g, 0, "chenglue").unwrap();
    resolve::submit(&mut g, 0, "use").unwrap();
    resolve::submit(&mut g, 0, "card:a").unwrap();
    resolve::submit(&mut g, 0, "card:b").unwrap();
    assert_eq!(hand_size(&g, 0), 1, "摸 1 弃 2（原 2 +1 -2）");
    assert!(b1::chenglue_free(&g, 0, Suit::Spade), "弃置牌花色成为自由花色");
    assert_eq!(skills::face_of(&g, 0, "chenglue"), 1, "发动后转阴面");
    assert_eq!(g.counter("phase.0.chenglue"), 1, "每阶段限一次");
}

#[test]
fn shicai_puts_card_on_top_then_draws_it() {
    let mut g = game(&["shicai"]);
    g.turn = 0;
    add_card(&mut g, "p0", "sha", Zone::Processing);
    add_deck(&mut g, &["d0"]);
    let step = run_steps(
        &mut g,
        "shicai",
        0,
        b1::data(&[("step", "ask".into()), ("card", "p0".into())]),
        &["use"],
    );
    assert!(matches!(step, Step::Finish(_)));
    assert_eq!(hand(&g, 0), vec!["p0".to_string()], "置顶后摸回同一张");
    assert_eq!(g.counter("turn.0.shicai.basic"), 1);
    // 每回合每类限一次：第二张基本牌不再询问。
    add_card(&mut g, "p1", "tao", Zone::Processing);
    let step = call(
        &mut g,
        "shicai",
        0,
        b1::data(&[("step", "ask".into()), ("card", "p1".into())]),
    );
    assert!(matches!(step, Step::Done));
    assert_eq!(g.cards.get("p1"), Some(&Zone::Processing), "同一类型本回合已用过");
}

#[test]
fn shicai_is_optional_with_pass_exit() {
    let mut g = game(&["shicai"]);
    g.turn = 0;
    add_card(&mut g, "p0", "wuxie_keji", Zone::Processing);
    add_deck(&mut g, &["d0"]);
    let step = run_steps(
        &mut g,
        "shicai",
        0,
        b1::data(&[("step", "ask".into()), ("card", "p0".into())]),
        &["pass"],
    );
    assert!(matches!(step, Step::Done));
    assert_eq!(g.cards.get("p0"), Some(&Zone::Processing), "放弃时牌不置顶");
    assert!(hand(&g, 0).is_empty(), "放弃时不摸牌");
    assert_eq!(
        g.counter("turn.0.shicai.trick"),
        0,
        "放弃不消耗「每回合每类一次」"
    );
}

#[test]
fn cunmu_draws_from_bottom_of_deck() {
    let mut g = game(&["cunmu"]);
    add_deck(&mut g, &["bottom", "mid", "top"]);
    let drawn = sanguosha3::kernel::flow::draw(&mut g, 0, 1);
    assert_eq!(drawn, vec!["bottom".to_string()], "寸目：从牌堆底摸");
    // 未拥有寸目 → 从顶摸
    let mut g2 = game(&[]);
    add_deck(&mut g2, &["bottom", "mid", "top"]);
    assert_eq!(sanguosha3::kernel::flow::draw(&mut g2, 0, 1), vec!["top".to_string()]);
}

// ---------------------------------------------------------------------------
// 牌结算路径集成：使用事件 → 订阅（奇制端到端）
// ---------------------------------------------------------------------------

#[test]
fn card_use_path_emits_events_and_routes_qizhi() {
    let mut g = game(&["qizhi"]);
    g.turn = 0;
    g.phase = Phase::Play;
    add_card(&mut g, "s0", "sha", Zone::Hand(0));
    add_card(&mut g, "t0", "tao", Zone::Hand(1));
    add_deck(&mut g, &["d0"]);
    sanguosha3::kernel::card_play::begin_use(&mut g, 0, "s0", false).unwrap();
    advance(&mut g).unwrap();
    resolve::submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.counter("turn.0.cards_used"), 1, "使用计数在结算内");
    advance(&mut g).unwrap();
    // 真实路由：奇制先询问，再由玩家选牌。
    assert_eq!(g.pending.as_ref().and_then(|p| p.skill()), Some("qizhi"));
    resolve::submit(&mut g, 0, "use").unwrap();
    resolve::submit(&mut g, 0, "card:t0").unwrap();
    assert_eq!(g.counter("turn.0.qizhi"), 1, "指定目标事件路由到奇制");
    assert_eq!(g.seats[1].hp, 3, "杀已结算");
}

#[test]
fn ask_helper_builds_stage_choose() {
    let a: Ask = b1::ask("yongsi", 0, b1::empty());
    assert_eq!(a.stage, Stage::Choose);
}
