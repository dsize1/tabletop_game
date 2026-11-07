//! 机制门禁：牌名硬编码、目标列表、手牌冻结。

use sanguosha2::kernel::{Card, Game, Phase, Seat, Suit};
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

// 门禁：技能/内核代码里不得用牌名比较（只能读 cards::def 属性）
#[test]
fn gate_no_card_name_comparison() {
    for (name, src) in [
        ("skills/b0.rs", include_str!("../src/skills/b0.rs")),
        ("skills/b1.rs", include_str!("../src/skills/b1.rs")),
        ("skills/b1b.rs", include_str!("../src/skills/b1b.rs")),
        ("kernel/flow.rs", include_str!("../src/kernel/flow.rs")),
    ] {
        for pat in ["name == \"", "name != \"", "matches!(name"] {
            assert!(
                !src.contains(pat),
                "{name} 出现牌名硬编码 `{pat}`（应读 cards::def 属性）"
            );
        }
    }
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
    assert!(g.to_judge(&card, 0).is_err(), "判定区废除后不能放入延时锦囊");
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
    assert_eq!(g.counter("turn.blocked_use"), 1, "冻结后使用应被拒绝") ;
    assert!(
        !g.events.iter().any(|e| matches!(e.kind, sanguosha2::kernel::event::EventKind::TargetChosen)),
        "被拒绝时不应产生目标指定事件"
    );
}

#[test]
fn wiring_qianjie_filters_targets() {
    let mut g = g4();
    g.cards.insert("le".to_string(), Card::new("le", "乐不思蜀", Suit::Heart, 6));
    g.use_card(0, &"le".to_string(), vec![1]);
    assert!(
        !g.events.iter().any(|e| matches!(e.kind, sanguosha2::kernel::event::EventKind::TargetChosen)),
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
    assert!(g.first_card_no_distance(0, &"c2".to_string()), "首张牌应无距离限制");
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
    g.to_equip(&"c2".to_string(), 0, Slot::HorseOffense).unwrap();
    assert_eq!(g.distance(0, 2), 1, "进攻马使距离 -1");
    assert!(g.can_reach(0, 2, &"c1".to_string()), "范围 5 应能打到距离 1");
}
