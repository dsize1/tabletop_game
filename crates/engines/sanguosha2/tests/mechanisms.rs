//! 机制门禁：牌名硬编码、目标列表、手牌冻结。

use sanguosha2::kernel::{Card, Game, Phase, Seat, Slot, Suit};
use sanguosha2::skills;

fn g() -> Game {
    let mut g = Game::new(
        vec![
            Seat::new("p0", "a", &["shicai"], 4),
            Seat::new("p1", "b", &[], 4),
        ],
        vec![
            Card::new("c1", "杀", Suit::Spade, 7),
            Card::new("c2", "无中生有", Suit::Heart, 8),
            Card::new("c3", "火攻", Suit::Diamond, 3),
        ],
    );
    g.to_hand(&"c1".to_string(), 0).unwrap();
    g.to_hand(&"c2".to_string(), 0).unwrap();
    g.to_hand(&"c3".to_string(), 1).unwrap();
    g
}

// 门禁：技能/内核代码里不得用牌名比较（只能读 cards::def 属性）。
//
// B17 harness：文件清单扩到全部技能模块 + 内核流程；历史批次的已知例外显式登记
// （`ALLOW`），新增代码一律不得再加牌名比较——否则新批次会绕过旧清单。
#[test]
fn gate_no_card_name_comparison() {
    // 已知例外（历史批次，待属性化）：文件 → 允许的牌名比较处数。
    // - b9（3 处）：界武圣/转换的 `name == "杀"/"火杀"/"雷杀"` → 应读 def.tags.sha
    // - b15（2 处）：天义 `name == "杀"`、巨象 `name == "南蛮入侵"` → 应读属性
    // - b16（1 处）：祸首 `name == "南蛮入侵"` → 应读 def.effect
    const ALLOW: &[(&str, usize)] = &[
        ("skills/b9.rs", 3),
        ("skills/b15.rs", 2),
        ("skills/b16.rs", 1),
    ];
    let files: &[(&str, &str)] = &[
        ("skills/b0.rs", include_str!("../src/skills/b0.rs")),
        ("skills/b1.rs", include_str!("../src/skills/b1.rs")),
        ("skills/b1b.rs", include_str!("../src/skills/b1b.rs")),
        ("skills/b2.rs", include_str!("../src/skills/b2.rs")),
        ("skills/b3.rs", include_str!("../src/skills/b3.rs")),
        ("skills/b4.rs", include_str!("../src/skills/b4.rs")),
        ("skills/b5.rs", include_str!("../src/skills/b5.rs")),
        ("skills/b6.rs", include_str!("../src/skills/b6.rs")),
        ("skills/b7.rs", include_str!("../src/skills/b7.rs")),
        ("skills/b8.rs", include_str!("../src/skills/b8.rs")),
        ("skills/b9.rs", include_str!("../src/skills/b9.rs")),
        ("skills/b10.rs", include_str!("../src/skills/b10.rs")),
        ("skills/b11.rs", include_str!("../src/skills/b11.rs")),
        ("skills/b12.rs", include_str!("../src/skills/b12.rs")),
        ("skills/b13.rs", include_str!("../src/skills/b13.rs")),
        ("skills/b14.rs", include_str!("../src/skills/b14.rs")),
        ("skills/b15.rs", include_str!("../src/skills/b15.rs")),
        ("skills/b16.rs", include_str!("../src/skills/b16.rs")),
        ("skills/b17.rs", include_str!("../src/skills/b17.rs")),
        (
            "skills/mingren.rs",
            include_str!("../src/skills/mingren.rs"),
        ),
        (
            "skills/kongsheng.rs",
            include_str!("../src/skills/kongsheng.rs"),
        ),
        (
            "skills/liangyin.rs",
            include_str!("../src/skills/liangyin.rs"),
        ),
        (
            "skills/zhenliang.rs",
            include_str!("../src/skills/zhenliang.rs"),
        ),
        ("kernel/flow.rs", include_str!("../src/kernel/flow.rs")),
        (
            "kernel/card_play.rs",
            include_str!("../src/kernel/card_play.rs"),
        ),
        (
            "kernel/pindian.rs",
            include_str!("../src/kernel/pindian.rs"),
        ),
    ];
    for (name, src) in files {
        let allowed = ALLOW
            .iter()
            .find(|(file, _)| file == name)
            .map(|(_, n)| *n)
            .unwrap_or(0);
        let hits: usize = ["name == \"", "name != \"", "matches!(name"]
            .iter()
            .map(|pat| src.matches(pat).count())
            .sum();
        assert!(
            hits <= allowed,
            "{name} 出现牌名硬编码 {hits} 处（允许 {allowed} 处历史例外）；新增代码应读 cards::def 属性"
        );
    }
}

// 门禁：B17 引入的「单动作批次」必须在 `begin_action` 清空，防止跨动作泄漏。
#[test]
fn action_batches_are_cleared_between_actions() {
    let mut g = batch_game();
    g.to_equip(&"e1".to_string(), 0, Slot::Weapon).unwrap();
    g.to_discard(&"e1".to_string(), "test").unwrap();
    assert!(
        g.lost_batch
            .get(&0)
            .is_some_and(|v| v.contains(&"e1".to_string())),
        "失去装备区牌进入 lost_batch"
    );
    assert!(
        g.equip_lost_batch.iter().any(|(s, c)| *s == 0 && c == "e1"),
        "失去装备区牌进入 equip_lost_batch"
    );
    g.action_seen.insert("x".to_string());
    g.begin_action();
    assert!(g.lost_batch.is_empty(), "begin_action 清空 lost_batch");
    assert!(
        g.equip_lost_batch.is_empty(),
        "begin_action 清空 equip_lost_batch"
    );
    assert!(g.gain_batch.is_empty(), "begin_action 清空 gain_batch");
    assert!(g.action_seen.is_empty(), "begin_action 清空 action_seen");
}

// 门禁：`gain_batch` 只记录「从其他角色获得」，摸牌不算（恩怨）。
#[test]
fn gain_batch_tracks_cross_seat_gain_not_draw() {
    let mut g = batch_game();
    g.to_hand(&"c2".to_string(), 1).unwrap();
    g.gain(&"c2".to_string(), 0).unwrap();
    assert!(
        g.gain_batch
            .iter()
            .any(|(from, to, id)| *from == 1 && *to == 0 && id == "c2"),
        "跨座位获得进入 gain_batch"
    );
    g.begin_action();
    let drawn = g.draw(0, 1);
    assert!(!drawn.is_empty(), "牌堆有牌");
    assert!(g.gain_batch.is_empty(), "摸牌不算「获得」");
}

fn batch_game() -> Game {
    Game::new(
        vec![Seat::new("p0", "a", &[], 4), Seat::new("p1", "b", &[], 4)],
        vec![
            Card::new("c1", "杀", Suit::Spade, 7),
            Card::new("c2", "闪", Suit::Heart, 8),
            Card::new("f1", "闪", Suit::Heart, 9),
            Card::new("f2", "闪", Suit::Club, 10),
            Card::equip("e1", "青釭剑", Suit::Spade, 6, Slot::Weapon),
        ],
    )
}

// 目标列表：多目标必须能逐个拿到
#[test]
fn target_list_preserves_each_target() {
    let mut g = g();
    g.use_card(1, &"c3".to_string(), vec![0, 1]);
    let ev = g.events.last().expect("事件");
    let list: Vec<usize> = ev
        .cause
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    assert_eq!(list, vec![0, 1], "目标列表必须保留每个成员");
}

// 手牌冻结：冻结后不能使用，切回合自动解冻
#[test]
fn hand_freeze_blocks_use_and_unfreezes_on_turn_change() {
    let mut g = g();
    let card = g.hand(0)[0].clone();
    g.freeze_hand(0);
    assert!(!g.can_use_card(0, &card), "冻结后不能使用");
    // 切到下一个回合 → 自动解冻
    sanguosha2::kernel::resolve::begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert!(g.can_use_card(0, &card), "回合切换应自动解冻");
}

// 卡牌属性：伤害锦囊 / 基本牌 / 不可指定目标
#[test]
fn card_def_attributes() {
    assert!(sanguosha2::cards::is_damage_trick("火攻"));
    assert!(sanguosha2::cards::is_damage_trick("决斗"));
    assert!(!sanguosha2::cards::is_damage_trick("无懈可击"));
    assert!(sanguosha2::cards::is_basic("桃"));
    assert!(!sanguosha2::cards::needs_choice("无中生有"));
    assert_eq!(
        sanguosha2::cards::target_mode("五谷丰登"),
        sanguosha2::cards::TargetMode::AllAlive,
        "五谷丰登/桃园结义应为全场多目标"
    );
    let _ = skills::SKILLS.len();
}

// 消费点：已废除的栏位不能装备 / 判定区废除后不能放延时锦囊
#[test]
fn abolished_slot_and_judge_zone_reject_cards() {
    use sanguosha2::kernel::Slot;
    let mut g = g();
    g.abolish_slot(0, Slot::Weapon).unwrap();
    let card = "c1".to_string();
    assert!(
        g.to_equip(&card, 0, Slot::Weapon).is_err(),
        "已废除的装备栏不能装备"
    );
    g.seats[0].judge_abolished = true;
    assert!(
        g.to_judge(&card, 0).is_err(),
        "判定区废除后不能放入延时锦囊"
    );
}

// 消费点：雄乱生效期间，张绣使用的牌自动标记为不可响应
#[test]
fn xiongluan_marks_used_cards_unresponsive() {
    let mut g = g();
    g.seats[0].flags.insert("xiongluan_active".to_string());
    let card = "c1".to_string();
    g.use_card(0, &card, vec![1]);
    assert!(g.is_unresponsive(&card), "雄乱期间其牌不可被响应");
}

// ---------------------------------------------------------------------------
// 消费点接线（5 条 DoD）
// ---------------------------------------------------------------------------

fn g4() -> Game {
    Game::new(
        vec![
            Seat::new("p0", "wanglie", &["wanglie"], 4),
            Seat::new("p1", "qianjie", &["qianjie"], 4),
            Seat::new("p2", "plain", &[], 4),
            Seat::new("p3", "plain", &[], 4),
        ],
        (1..=12)
            .map(|i| Card::new(format!("c{i}"), "杀", Suit::Spade, (i % 13) as u8))
            .collect(),
    )
}

#[test]
fn wiring_frozen_blocks_use_card() {
    let mut g = g4();
    g.to_hand(&"c1".to_string(), 0).unwrap();
    g.freeze_hand(0);
    g.use_card(0, &"c1".to_string(), vec![2]);
    assert_eq!(g.counter("turn.blocked_use"), 1, "冻结后使用应被拒绝");
    assert!(
        !g.events
            .iter()
            .any(|e| matches!(e.kind, sanguosha2::kernel::event::EventKind::TargetChosen)),
        "被拒绝时不应产生目标指定事件"
    );
}

#[test]
fn wiring_qianjie_filters_targets() {
    let mut g = g4();
    g.cards.insert(
        "le".to_string(),
        Card::new("le", "乐不思蜀", Suit::Heart, 6),
    );
    g.use_card(0, &"le".to_string(), vec![1]);
    assert!(
        !g.events
            .iter()
            .any(|e| matches!(e.kind, sanguosha2::kernel::event::EventKind::TargetChosen)),
        "谦节角色不应被写入目标列表"
    );
}

#[test]
fn wiring_sha_limit_and_hand_limit_read_jueyan() {
    let mut g = g4();
    assert_eq!(g.sha_limit(0), 1, "基础 1 次");
    for _ in 0..3 {
        g.bump("turn.jueyan_sha_bonus");
    }
    assert_eq!(g.sha_limit(0), 4, "决堰武器栏 +3");
    g.bump("turn.lijun_sha_bonus");
    assert_eq!(g.sha_limit(0), 5, "立军 +1");
    g.seats[0].hp = 3;
    for _ in 0..3 {
        g.bump("turn.jueyan_hand_limit");
    }
    assert_eq!(g.hand_limit(0), 6, "手牌上限 = 体力 3 + 决堰防具栏 3");
}

#[test]
fn wiring_reach_uses_wanglie_first_card() {
    let mut g = g4();
    g.to_hand(&"c2".to_string(), 0).unwrap();
    // 无往烈的角色：4 人局 2→0 距离 2，基础范围 1 → 打不到
    assert!(!g.can_reach(2, 0, &"c2".to_string()), "距离 2 超出范围 1");
    // 往烈：出牌阶段首张牌无距离限制 → 0→2 可打
    assert!(
        g.first_card_no_distance(0, &"c2".to_string()),
        "首张牌应无距离限制"
    );
    assert!(g.can_reach(0, 2, &"c2".to_string()), "往烈首张牌应无视距离");
    // 距离 1 的目标本来就在范围内
    assert!(g.can_reach(0, 1, &"c2".to_string()));
}

// 攻击范围与坐骑（与旧引擎 range_value 一致）
#[test]
fn attack_range_reads_weapon_and_horses() {
    use sanguosha2::kernel::Slot;
    let mut g = g4();
    assert_eq!(g.attack_range(0), 1, "无武器时范围 1");
    // 装备麒麟弓（范围 5）
    g.cards.get_mut("c1").unwrap().name = "麒麟弓".to_string();
    g.to_equip(&"c1".to_string(), 0, Slot::Weapon).unwrap();
    assert_eq!(g.attack_range(0), 5, "武器范围应生效");
    // 距离：4 人局 0→2 基础距离 2；装备进攻马（赤兔）后 -1
    assert_eq!(g.distance(0, 2), 2);
    g.cards.get_mut("c2").unwrap().name = "赤兔".to_string();
    g.to_equip(&"c2".to_string(), 0, Slot::HorseOffense)
        .unwrap();
    assert_eq!(g.distance(0, 2), 1, "进攻马使距离 -1");
    assert!(
        g.can_reach(0, 2, &"c1".to_string()),
        "范围 5 应能打到距离 1"
    );
}

// 弃置契约（mechanics `discard-contract`，用户 2026-11-06 裁定）：
// 1) 判定区移出**不算弃置**（只发 JudgeZoneLeft，不发 CardDiscarded）；
// 2) 弃置 ≠ 使用（CardDiscarded / CardUsed 互斥）；
// 3) 进/出判定区都有统一事件。
#[test]
fn discard_contract_judge_zone_is_not_discard() {
    use sanguosha2::kernel::event::EventKind;
    let mut g = g();
    g.to_judge(&"c3".to_string(), 0).unwrap();
    assert!(
        g.events
            .iter()
            .any(|e| e.kind == EventKind::JudgeZoneEntered),
        "进入判定区要发 JudgeZoneEntered"
    );
    g.events.clear();
    g.to_discard(&"c3".to_string(), "yonglue").unwrap();
    assert!(
        g.events.iter().any(|e| e.kind == EventKind::JudgeZoneLeft),
        "离开判定区要发 JudgeZoneLeft"
    );
    assert!(
        !g.events.iter().any(|e| e.kind == EventKind::CardDiscarded),
        "判定区的牌不算弃置（不发 CardDiscarded）"
    );
    assert_eq!(g.phase_discarded, 0, "不计入本阶段弃牌数");
}

#[test]
fn discard_contract_discard_is_not_use() {
    use sanguosha2::kernel::event::EventKind;
    let mut g = g();
    let card = "c1".to_string();
    g.to_discard(&card, "hand_cap").unwrap();
    assert!(g.events.iter().any(|e| e.kind == EventKind::CardDiscarded));
    assert!(
        !g.events.iter().any(|e| e.kind == EventKind::CardUsed),
        "弃置不是使用"
    );
    g.events.clear();
    let card2 = "c2".to_string();
    g.use_card(0, &card2, vec![]);
    assert!(g.events.iter().any(|e| e.kind == EventKind::CardUsed));
    assert!(
        !g.events.iter().any(|e| e.kind == EventKind::CardDiscarded),
        "使用不是弃置"
    );
}

#[test]
fn draw_refills_from_discard_when_deck_empty() {
    // 机制（M14）：牌堆耗尽时必须把弃牌堆洗回牌堆；否则双方无牌可摸/可出 → 回合空转不终局。
    let mut g = g();
    // 夹具三张牌都在手牌 → 牌堆为空。先把一张手牌置入弃牌堆，再摸牌验证重洗。
    let c3 = "c3".to_string();
    g.to_discard(&c3, "test").unwrap();
    assert!(g.ids_in(|z| *z == sanguosha2::kernel::Zone::Draw).is_empty());
    let got = g.draw(1, 2);
    assert_eq!(got.len(), 1, "弃牌堆只有 1 张：洗回后只能摸 1 张");
    assert!(
        g.log.iter().any(|l| l.text.contains("洗回牌堆")),
        "应记录重洗事实：{:?}",
        g.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    );
    assert!(g.ids_in(|z| *z == sanguosha2::kernel::Zone::Draw).is_empty(), "牌堆与弃牌堆都空");
}

// ---------------------------------------------------------------------------
// 终局房规（2026-11-11 裁定）：round > 100 起【桃】/【桃园结义】回血无效
// ---------------------------------------------------------------------------

fn endgame_game() -> Game {
    let mut g = Game::new(
        vec![
            Seat::new("p0", "a", &[], 4),
            Seat::new("p1", "b", &[], 4),
        ],
        vec![
            Card::new("t1", "桃", Suit::Heart, 3),
            Card::new("t2", "桃园结义", Suit::Heart, 1),
            Card::new("t3", "桃", Suit::Heart, 4),
            Card::new("d1", "闪", Suit::Diamond, 2),
        ],
    );
    g.turn = 0;
    g.phase = Phase::Play;
    g.seats[0].hp = 2;
    g
}

#[test]
fn endgame_round_100_still_heals() {
    let mut g = endgame_game();
    g.round = 100;
    g.to_hand(&"t1".to_string(), 0).unwrap();
    g.resolve_use(0, &"t1".to_string(), vec![0]).unwrap();
    assert_eq!(g.seats[0].hp, 3, "第 100 轮仍可正常回血（阈值是 round > 100）");
}

#[test]
fn endgame_tao_heal_is_void_after_round_100() {
    let mut g = endgame_game();
    g.round = 101;
    g.to_hand(&"t1".to_string(), 0).unwrap();
    g.resolve_use(0, &"t1".to_string(), vec![0]).unwrap();
    assert_eq!(g.seats[0].hp, 2, "第 101 轮起【桃】回血无效");
    assert!(
        g.discard_pile().iter().any(|c| c == "t1"),
        "牌仍然正常使用并进弃牌堆（不是非法动作）"
    );
    assert!(
        g.log.iter().any(|l| l.text.contains("回血无效")),
        "必须给出明确的日志（端侧/复盘可读）"
    );
}

#[test]
fn endgame_peach_garden_heal_is_void_after_round_100() {
    let mut g = endgame_game();
    g.round = 101;
    g.seats[1].hp = 3;
    g.to_hand(&"t2".to_string(), 0).unwrap();
    g.resolve_use(0, &"t2".to_string(), vec![0, 1]).unwrap();
    assert_eq!(g.seats[0].hp, 2, "桃园结义：自己也不回血");
    assert_eq!(g.seats[1].hp, 3, "桃园结义：他人也不回血");
}

#[test]
fn endgame_dying_tao_still_saves_after_round_100() {
    use sanguosha2::kernel::flow;
    use sanguosha2::kernel::resolve::advance;
    let mut g = endgame_game();
    g.round = 150;
    g.to_hand(&"t3".to_string(), 0).unwrap();
    g.seats[0].hp = 0;
    flow::start_dying(&mut g, 0, None);
    advance(&mut g).unwrap();
    // 濒死救援不受房规限制：用【桃】自救。
    let heal = g
        .pending
        .as_ref()
        .and_then(|p| p.options().iter().find(|o| o.id.starts_with("respond:")).map(|o| o.id.clone()))
        .expect("濒死应有【桃】救援选项");
    let actor = g.pending.as_ref().map(|p| p.actor()).unwrap_or(0);
    sanguosha2::kernel::resolve::submit(&mut g, actor, &heal).unwrap();
    assert_eq!(g.seats[0].hp, 1, "濒死阶段【桃】仍然有效");
}
