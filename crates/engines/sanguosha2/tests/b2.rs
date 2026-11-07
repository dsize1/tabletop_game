//! B2 首批：征荣（伤害锦囊属性 + 移出游戏）/ 从谏（目标列表 + 「然后」）。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{Card, Game, Phase, Seat, Suit, Zone};

fn game(specs: &[(&str, &str, &[&str])]) -> Game {
    let seats: Vec<Seat> = specs.iter().map(|(n, gen, sk)| Seat::new(n, gen, sk, 4)).collect();
    let names = ["杀", "火攻", "桃", "闪", "决斗", "无懈可击", "杀", "闪", "桃", "万箭齐发"];
    let cards: Vec<Card> = (1..=20)
        .map(|i| {
            Card::new(
                format!("c{i}"),
                names[(i - 1) % names.len()],
                if i % 2 == 0 { Suit::Heart } else { Suit::Spade },
                (i % 13) as u8,
            )
        })
        .collect();
    let mut g = Game::new(seats, cards);
    for s in 0..specs.len() {
        for j in 0..3 {
            g.to_hand(&format!("c{}", s * 3 + j + 1), s).unwrap();
        }
    }
    g
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn answer(g: &mut Game, actor: usize, action: &str) {
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}

#[test]
fn zhengrong_takes_card_out_of_game_when_using_damage_trick() {
    let mut g = game(&[
        ("p0", "mouqiu", &["zhengrong"]),
        ("p1", "other", &[]),
    ]);
    // p1 手牌多于 p0：给 p1 多 2 张
    g.to_hand(&"c10".to_string(), 1).unwrap();
    g.to_hand(&"c11".to_string(), 1).unwrap();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 火攻（伤害锦囊）指定 p1
    g.use_card(0, &"c2".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("zhengrong"), "伤害锦囊应触发征荣");
    let t = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &t);
    let card = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &card);
    assert_eq!(g.ids_in(|z| *z == Zone::Void).len(), 1, "「荣」应移出游戏");
    assert_eq!(g.counter("turn.rong"), 1);
}

#[test]
fn zhengrong_does_not_trigger_on_non_damage_trick() {
    let mut g = game(&[
        ("p0", "mouqiu", &["zhengrong"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 杀之外的非伤害锦囊（无中生有不在牌堆，用【闪】代替：不是伤害锦囊也不是杀）
    g.use_card(0, &"c4".to_string(), vec![1]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "非杀/非伤害锦囊不触发征荣");
}

#[test]
fn congjian_gives_own_card_then_draws() {
    let mut g = game(&[
        ("p0", "zhangxiu", &["congjian"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let hand_before = g.hand(0).len();
    g.use_card(1, &"c1".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("congjian"));
    let opt = pend(&g).options()[0].id.clone();
    let card = opt.trim_start_matches("choose:").to_string();
    answer(&mut g, 0, &opt);
    assert_eq!(g.hand(0).len(), hand_before, "交 1 摸 1 → 数量不变");
    assert!(g.hand(1).contains(&card), "交出的牌进入来源手牌");
}

#[test]
fn congjian_cannot_draw_without_giving() {
    let mut g = game(&[
        ("p0", "zhangxiu", &["congjian"]),
        ("p1", "other", &[]),
    ]);
    // 张绣没有自己的牌（手牌清空、无装备）
    while let Some(id) = g.hand(0).first().cloned() {
        g.to_discard(&id, "test").unwrap();
    }
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c1".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "无自己的牌 → 不能发动、不能摸");
}

// ---------------------------------------------------------------------------
// 往烈：手动令牌「不可响应」+ 冻结手牌到回合结束
// ---------------------------------------------------------------------------

#[test]
fn wanglie_marks_unresponsive_and_freezes_hand() {
    let mut g = game(&[
        ("p0", "chendao", &["wanglie"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = "c1".to_string(); // 杀（基本牌）
    g.use_card(0, &card, vec![1]);
    advance(&mut g).unwrap();
    assert_eq!(pend(&g).skill(), Some("wanglie"), "应向使用者询问是否发动");
    answer(&mut g, 0, "choose:wanglie");
    assert!(g.is_unresponsive(&card), "该牌应被标记为不能被响应");
    // 手牌冻结：所有手牌都不能再主动使用
    let other = g.hand(0).first().cloned().unwrap();
    assert!(!g.can_use_card(0, &other), "发动后本阶段不能再使用牌");
    // 切回合自动解冻
    sanguosha2::kernel::resolve::begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert!(g.can_use_card(0, &other), "回合结束应自动解冻");
}

#[test]
fn wanglie_declining_does_not_freeze() {
    let mut g = game(&[
        ("p0", "chendao", &["wanglie"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    let card = "c1".to_string();
    g.use_card(0, &card, vec![1]);
    advance(&mut g).unwrap();
    answer(&mut g, 0, "pass");
    assert!(!g.is_unresponsive(&card), "放弃时不得标记");
    let other = g.hand(0).first().cloned().unwrap();
    assert!(g.can_use_card(0, &other), "放弃时不冻结手牌");
}

// ---------------------------------------------------------------------------
// 父荫：每回合首次成为杀/决斗目标且对方手牌 ≥ 你 → 此牌对你无效
// ---------------------------------------------------------------------------

#[test]
fn fuyin_marks_unresponsive_when_qualifying() {
    let mut g = game(&[
        ("p0", "zhugzhan", &["fuyin"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let card = "c1".to_string(); // 杀
    g.use_card(1, &card, vec![0]);
    advance(&mut g).unwrap();
    assert!(g.is_unresponsive(&card), "父荫应令此牌对张春华无效");
    assert_eq!(g.counter("turn.fuyin_done"), 1);
}

#[test]
fn fuyin_only_once_per_turn() {
    let mut g = game(&[
        ("p0", "zhugzhan", &["fuyin"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.use_card(1, &"c1".to_string(), vec![0]);
    advance(&mut g).unwrap();
    g.use_card(1, &"c7".to_string(), vec![0]);
    advance(&mut g).unwrap();
    assert!(
        !g.is_unresponsive(&"c7".to_string()),
        "同一回合第二次不触发父荫"
    );
}

// ---------------------------------------------------------------------------
// 决堰：废除一个装备栏，按栏位给本回合效果
// ---------------------------------------------------------------------------

#[test]
fn jueyan_abolishes_slot_and_grants_effect() {
    use sanguosha2::kernel::Slot;
    let mut g = game(&[
        ("p0", "lukang", &["jueyan"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert!(sanguosha2::skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:jueyan"));
    sanguosha2::skills::execute_active(&mut g, 0, "skill:jueyan").unwrap();
    let opt = pend(&g).options()[0].id.clone(); // 第一个 = 武器
    answer(&mut g, 0, &opt);
    assert!(g.is_slot_abolished(0, Slot::Weapon), "武器栏应被废除");
    assert_eq!(g.counter("turn.jueyan_sha_bonus"), 3, "杀次数 +3");
    // 每阶段限一次
    assert!(!sanguosha2::skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:jueyan"));
}

#[test]
fn jueyan_excludes_abolished_slots() {
    use sanguosha2::kernel::Slot;
    let mut g = game(&[
        ("p0", "lukang", &["jueyan"]),
        ("p1", "other", &[]),
    ]);
    g.abolish_slot(0, Slot::Weapon).unwrap();
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    sanguosha2::skills::execute_active(&mut g, 0, "skill:jueyan").unwrap();
    let opts: Vec<String> = pend(&g).options().iter().map(|o| o.id.clone()).collect();
    assert!(
        !opts.iter().any(|o| o.contains("武器")),
        "已废除的栏位不可重复选择：{opts:?}"
    );
}

// ---------------------------------------------------------------------------
// 破势（觉醒，准备阶段）/ 雄乱（限定技，废装备区+判定区）
// ---------------------------------------------------------------------------

#[test]
fn poshi_awaken_abolishes_all_slots_at_prepare() {
    use sanguosha2::kernel::Slot;
    let mut g = game(&[
        ("p0", "lukang", &["poshi"]),
        ("p1", "other", &[]),
    ]);
    g.seats[0].hp = 1;
    let max_hp = g.seats[0].max_hp;
    // 注意：Game::new 的初始 phase 就是 Prepare，需先切到别的阶段再切回才会发 PhaseStart
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, max_hp - 1, "觉醒减 1 点体力上限");
    for slot in [
        Slot::Weapon,
        Slot::Armor,
        Slot::HorseOffense,
        Slot::HorseDefense,
        Slot::Treasure,
    ] {
        assert!(g.is_slot_abolished(0, slot), "{slot:?} 应被废除");
    }
}

#[test]
fn xiongluan_abolishes_equip_and_judge_zones() {
    let mut g = game(&[
        ("p0", "zhangxiu", &["xiongluan"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert!(sanguosha2::skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:xiongluan"));
    sanguosha2::skills::execute_active(&mut g, 0, "skill:xiongluan").unwrap();
    assert!(g.seats[0].judge_abolished, "判定区应被废除");
    assert!(g.is_slot_abolished(0, sanguosha2::kernel::Slot::Weapon));
    assert!(g.seats[0].flags.contains("xiongluan_active"));
    // 限定技：第二次不再提供
    assert!(!sanguosha2::skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:xiongluan"));
}

// ---------------------------------------------------------------------------
// 谦节：不能成为延时锦囊 / 铁索连环 / 拼点目标
// ---------------------------------------------------------------------------

#[test]
fn qianjie_blocks_delayed_tricks_and_chains() {
    let mut g = game(&[
        ("p0", "lukang", &["qianjie"]),
        ("p1", "other", &[]),
    ]);
    // 牌堆里有 乐不思蜀/铁索？用 use_card 传入的牌名判定
    g.cards.insert(
        "le".to_string(),
        Card::new("le", "乐不思蜀", Suit::Heart, 6),
    );
    g.cards.insert(
        "suo".to_string(),
        Card::new("suo", "铁索连环", Suit::Club, 10),
    );
    assert!(
        !sanguosha2::skills::can_target(&g, 1, "le", 0),
        "不能成为延时锦囊目标"
    );
    assert!(
        !sanguosha2::skills::can_target(&g, 1, "suo", 0),
        "不能成为铁索连环目标"
    );
    assert!(
        sanguosha2::skills::can_target(&g, 1, "c1", 0),
        "普通杀仍可指定"
    );
    assert!(sanguosha2::skills::blocks_pindian(&g, 0), "不能成为拼点目标");
    assert!(!sanguosha2::skills::blocks_pindian(&g, 1));
}

// ---------------------------------------------------------------------------
// 镇骨：两个时刻（立刻一次 + 目标回合结束再一次）
// ---------------------------------------------------------------------------

#[test]
fn zhengu_resolves_now_and_at_target_turn_end() {
    let mut g = game(&[
        ("p0", "haozhao", &["zhengu"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 让郝昭手牌数 = 2，目标 = 3
    while g.hand(0).len() > 2 {
        let id = g.hand(0)[0].clone();
        g.to_discard(&id, "setup").unwrap();
    }
    while g.hand(1).len() < 3 {
        let id = format!("c{}", 15 + g.hand(1).len());
        g.to_hand(&id, 1).unwrap();
    }
    assert_eq!(g.hand(0).len(), 2);
    assert_eq!(g.hand(1).len(), 3);
    sanguosha2::skills::execute_active(&mut g, 0, "skill:zhengu").unwrap();
    let t = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &t);
    assert_eq!(g.hand(1).len(), 2, "第一个时刻：手牌数向郝昭看齐");
    assert_eq!(g.scheduled.len(), 1, "记下第二个时刻");
    // 目标回合结束 → 第二次结算（此时郝昭手牌已变，目标再对齐一次）
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.end_phase("End").unwrap();
    assert!(g.scheduled.is_empty(), "第二时刻结算后应清除");
}

// ---------------------------------------------------------------------------
// 罪论：满足 1 项 → 选 1 张 → 剩余 2 张**两种排列**二选一
// ---------------------------------------------------------------------------

#[test]
fn zuilun_one_condition_then_two_card_order() {
    let mut g = game(&[
        ("p0", "zhugzhan", &["zuilun"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    // 让「仅满足 1 项」成立：
    //  ① 造成过伤害 → true
    //  ② 已弃置过牌 → false
    //  ③ 手牌最多（不是最少）→ false
    sanguosha2::kernel::flow::damage(&mut g, Some(0), 1, 1);
    g.to_hand(&"c19".to_string(), 0).unwrap();
    g.to_hand(&"c20".to_string(), 0).unwrap();
    let id = g.hand(0)[0].clone();
    g.to_discard(&id, "test").unwrap();
    g.end_phase("End").unwrap();
    // 选 1 张
    assert_eq!(pend(&g).skill(), Some("zuilun"));
    let pick = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &pick);
    // 剩余 2 张 → 恰好两个「排列」选项
    let opts: Vec<String> = pend(&g).options().iter().map(|o| o.id.clone()).collect();
    let orders: Vec<&String> = opts.iter().filter(|o| o.starts_with("order:")).collect();
    assert_eq!(orders.len(), 2, "应恰好给出两种排列：{opts:?}");
    let first_opt = orders[0].clone();
    let parts: Vec<&str> = first_opt.trim_start_matches("order:").split(',').collect();
    let (a, b) = (parts[0].to_string(), parts[1].to_string());
    answer(&mut g, 0, &first_opt);
    // 牌堆顶 = 选中的第一张，次顶 = 第二张
    let draw_order = g.ids_in(|z| *z == sanguosha2::kernel::Zone::Draw);
    assert_eq!(draw_order.first(), Some(&a), "第一张应成为新牌堆顶");
    assert_eq!(draw_order.get(1), Some(&b), "第二张应在次顶");
}
#[test]
fn zuilun_all_false_asks_victim() {
    let mut g = game(&[
        ("p0", "zhugzhan", &["zuilun"]),
        ("p1", "other", &[]),
    ]);
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.to_hand(&"c19".to_string(), 0).unwrap();
    g.to_hand(&"c20".to_string(), 0).unwrap();
    let id = g.hand(0)[0].clone();
    g.to_discard(&id, "test").unwrap();
    g.end_phase("End").unwrap();
    assert_eq!(pend(&g).skill(), Some("zuilun"), "均不满足应询问受害者");
    assert!(pend(&g).options().iter().any(|o| o.id.starts_with("target:")));
    let hp0 = g.seats[0].hp;
    let hp1 = g.seats[1].hp;
    let opt = pend(&g).options()[0].id.clone();
    answer(&mut g, 0, &opt);
    assert_eq!(g.seats[0].hp, hp0 - 1);
    assert_eq!(g.seats[1].hp, hp1 - 1);
}
// ---------------------------------------------------------------------------
// 鸿举：荣≥3 觉醒 + 等量选牌替换「荣」
// ---------------------------------------------------------------------------

#[test]
fn hongju_awaken_requires_equal_count_swap() {
    let mut g = game(&[
        ("p0", "mouqiu", &["hongju"]),
        ("p1", "other", &[]),
    ]);
    // 造 3 枚「荣」
    for _ in 0..3 {
        g.bump("turn.rong");
    }
    let max_hp = g.seats[0].max_hp;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.end_phase("Play").unwrap();
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(g.seats[0].max_hp, max_hp - 1, "觉醒减 1 点体力上限");
    assert_eq!(pend(&g).skill(), Some("hongju"), "应要求等量选牌替换「荣」");
    // 选满 3 张
    for _ in 0..3 {
        if g.pending.is_none() {
            break;
        }
        let opt = pend(&g).options()[0].id.clone();
        answer(&mut g, 0, &opt);
    }
    assert_eq!(
        g.ids_in(|z| *z == Zone::Void).len(),
        3,
        "选中的手牌应成为新的「荣」（移出游戏）"
    );
}
