//! B18 用例（17 技能，每条 ≥1 正例 + ≥1 反例）。
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit, Zone};
use sanguosha2::skills;

fn deck() -> Vec<Card> {
    let spec: &[(&str, &str, Suit, u8)] = &[
        ("sp1", "杀", Suit::Spade, 13),
        ("sp2", "杀", Suit::Spade, 10),
        ("sp3", "杀", Suit::Club, 12),
        ("h1", "杀", Suit::Heart, 9),
        ("d1", "闪", Suit::Diamond, 2),
        ("d2", "闪", Suit::Diamond, 3),
        ("d3", "闪", Suit::Diamond, 4),
        ("d4", "闪", Suit::Diamond, 5),
        ("t1", "桃", Suit::Heart, 9),
        ("t2", "桃", Suit::Heart, 8),
        ("j1", "酒", Suit::Spade, 9),
        ("jd1", "决斗", Suit::Club, 4),
        ("jd2", "决斗", Suit::Diamond, 6),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("sh1", "顺手牵羊", Suit::Diamond, 7),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("wj1", "万箭齐发", Suit::Heart, 1),
        ("wz1", "无中生有", Suit::Heart, 7),
        ("le1", "乐不思蜀", Suit::Heart, 6),
        ("bl1", "兵粮寸断", Suit::Spade, 5),
        ("hg1", "火攻", Suit::Heart, 2),
        ("tl1", "铁索连环", Suit::Club, 10),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip("w2", "朱雀羽扇", Suit::Diamond, 1, Slot::Weapon));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
    cards.push(Card::equip("arm2", "藤甲", Suit::Club, 2, Slot::Armor));
    cards.push(Card::equip("horse1", "赤兔", Suit::Diamond, 5, Slot::HorseOffense));
    cards.push(Card::equip("horse2", "的卢", Suit::Club, 5, Slot::HorseDefense));
    for i in 40..=99 {
        let suit = match i % 4 {
            0 => Suit::Heart,
            1 => Suit::Diamond,
            2 => Suit::Club,
            _ => Suit::Spade,
        };
        cards.push(Card::new(format!("x{i}"), "闪", suit, ((i % 13) + 1) as u8));
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
    g.pending
        .as_ref()
        .and_then(|p| p.skill().map(|s| s.to_string()))
}
fn pend_actor(g: &Game) -> usize {
    g.pending.as_ref().map(|p| p.actor()).unwrap_or(usize::MAX)
}
fn options(g: &Game) -> Vec<String> {
    g.pending
        .as_ref()
        .map(|p| p.options().iter().map(|o| o.id.clone()).collect())
        .unwrap_or_default()
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
/// 通用作答（测试推进用）：**优先选第一个实质选项**，只有「只剩出口」时才放弃。
///
/// 不能反过来（默认放弃）：会把「激将」「拆牌」这类真实决策静默拒掉，
/// 让用例看起来通过但实际没走目标分支。
fn answer_any(g: &mut Game) {
    drain_forced(g);
    let Some(p) = g.pending.clone() else { return };
    let action = p
        .options()
        .iter()
        .find(|o| o.id != p.escape_id())
        .map(|o| o.id.clone())
        .unwrap_or_else(|| p.escape_id().to_string());
    answer(g, p.actor(), &action);
}

fn answer_cur(g: &mut Game, action: &str) {
    // I1：先把「无牌可打」的礼貌询问排空，再取当前被询问者（排空会推进流程）。
    drain_forced(g);
    let actor = pend_actor(g);
    answer(g, actor, action);
}
fn play_phase(g: &mut Game, seat: usize) {
    g.turn = seat;
    begin_phase(g, seat, Phase::Play).unwrap();
}
fn kingdom_count(g: &Game) -> usize {
    let mut set = std::collections::BTreeSet::new();
    for s in &g.seats {
        if s.hp > 0 {
            set.insert(s.kingdom.clone());
        }
    }
    set.len()
}
fn pass_until(g: &mut Game, skill: &str) {
    for _ in 0..16 {
        // I1：先排空「无牌可打」的礼貌询问，再判断是否已到目标技能（排空会推进流程）。
        drain_forced(g);
        if pend_skill(g).as_deref() == Some(skill) {
            return;
        }
        if g.pending.is_none() {
            return;
        }
        answer_any(g);
    }
}

// ---------------------------------------------------------------- 贞烈
#[test]
fn zhenlie_cancels_sha_and_grants_miji() {
    let mut g = game(&[
        ("wangyi", &["zhenlie", "miji"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 1, &["sp1"]);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    pass_until(&mut g, "zhenlie");
    assert_eq!(pend_skill(&g).as_deref(), Some("zhenlie"));
    assert_eq!(pend_actor(&g), 0);
    answer(&mut g, 0, "opt1");
    assert_eq!(g.seats[0].hp, 2, "失去 1 点体力");
    assert_eq!(pend_skill(&g).as_deref(), Some("zhenlie"), "界贞烈二选一");
    answer(&mut g, 0, "miji");
    assert_eq!(g.counter("turn.miji_granted"), 1);
}

#[test]
fn zhenlie_cancels_normal_trick_but_not_delayed() {
    let mut g = game(&[
        ("wangyi", &["zhenlie"], 3, "wei"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 1);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["gh1", "le1"]);
    g.resolve_use(1, &"gh1".to_string(), vec![0]).unwrap();
    pass_until(&mut g, "zhenlie");
    assert_eq!(pend_skill(&g).as_deref(), Some("zhenlie"), "普通锦囊可贞烈");
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pass");
    // 延时锦囊不触发贞烈。
    g.resolve_use(1, &"le1".to_string(), vec![0]).unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zhenlie"));
}

// ---------------------------------------------------------------- 秘计
#[test]
fn miji_fires_once_in_own_end_phase_even_without_grant() {
    let mut g = game(&[("wangyi", &["zhenlie", "miji"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 0;
    g.seats[0].hp = 2;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("miji"), "自己结束阶段恒可发动");
    let before = g.hand(0).len();
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), before + 1, "摸 X=1 张");
    answer(&mut g, 0, "pass");
    // 模拟下一个自己的回合：即使也有贞烈②授权，结束阶段也只发动一次。
    g.counters.insert("turn.miji_offered:0".to_string(), 0);
    g.counters.insert("turn.miji_granted".to_string(), 1);
    g.phase = Phase::Play;
    g.seats[0].hp = 2;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("miji"));
    answer(&mut g, 0, "pass");
    assert_ne!(pend_skill(&g).as_deref(), Some("miji"), "只发动一次");
}

#[test]
fn miji_triggers_at_other_players_end_phase_when_granted() {
    let mut g = game(&[
        ("wangyi", &["zhenlie", "miji"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 1);
    g.seats[0].hp = 2; // 已损失 1 → 秘计 X=1
    give(&mut g, 1, &["sp1"]);
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    pass_until(&mut g, "zhenlie");
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "miji");
    // 该【杀】对王异无效，结算结束后轮到 p1 的结束阶段。
    if g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    g.phase = Phase::Play;
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("miji"), "他人回合结束阶段发动秘计");
    assert_eq!(pend_actor(&g), 0);
    let before = g.hand(0).len();
    answer(&mut g, 0, "opt1");
    // 贞烈已失去 1 点体力 → 已损失 2 → 摸 2 张。
    assert_eq!(g.hand(0).len(), before + 2);
    // 反例：他人回合、未获贞烈②授权 → 不发动。
    let mut g2 = game(&[("wangyi", &["zhenlie", "miji"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g2.turn = 1;
    g2.seats[0].hp = 2;
    g2.phase = Phase::Play;
    begin_phase(&mut g2, 1, Phase::End).unwrap();
    assert_ne!(pend_skill(&g2).as_deref(), Some("miji"));
}

// ---------------------------------------------------------------- 绝情
#[test]
fn jueqing_turns_damage_into_hp_loss() {
    let mut g = game(&[("zhangchunhua", &["jueqing"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 0;
    flow::damage(&mut g, Some(0), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 3, "伤害转为失去 1 点体力");
    assert_eq!(g.counter("turn.damage_taken:1"), 0, "不产生伤害事件（受伤计数为 0）");
    assert!(g.log.iter().any(|e| e.text.contains("绝情")));
    // 反例：无绝情来源正常伤害。
    let mut g2 = game(&[("p0", &[], 3, "wei"), ("p1", &[], 4, "qun")]);
    g2.turn = 0;
    flow::damage(&mut g2, Some(0), 1, 1);
    advance(&mut g2).unwrap();
    assert_eq!(g2.counter("turn.damage_taken:1"), 1);
}

// ---------------------------------------------------------------- 伤逝
#[test]
fn shangshi_draws_to_lost_hp_but_not_in_discard_phase() {
    let mut g = game(&[("zhangchunhua", &["shangshi"], 4, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 0;
    g.seats[0].hp = 2; // 已损失 2
    give(&mut g, 0, &["d1", "d2", "d3"]);
    g.phase = Phase::Play;
    g.to_discard(&"d1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.hand(0).len(), 2, "失去手牌后摸至已损失体力 2");
    // 弃牌阶段内不即时补：弃到 0 也不补。
    g.phase = Phase::Discard;
    let ids: Vec<String> = g.hand(0);
    for id in ids {
        g.to_discard(&id, "test").unwrap();
    }
    advance(&mut g).unwrap();
    assert_eq!(g.hand(0).len(), 0, "弃牌阶段内不补牌");
    // 弃牌阶段结束（进入结束阶段）时补到 2。
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(g.hand(0).len(), 2);
}

// ---------------------------------------------------------------- 翦灭
#[test]
fn jianmie_more_discards_triggers_duel() {
    let mut g = game(&[
        ("zhangchunhua", &["jianmie"], 3, "wei"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1", "sp2", "d1"]);
    give(&mut g, 1, &["d2"]);
    skills::execute_active(&mut g, 0, "skill:jianmie").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:black"); // 张春华弃 2 张黑
    answer(&mut g, 1, "choose:red"); // 目标弃 1 张红
    pass_until(&mut g, "__card");
    if g.pending.is_some() {
        answer_cur(&mut g, "pass");
    }
    assert_eq!(g.seats[1].hp, 3, "弃牌多者视为使用【决斗】并造成伤害");
}

#[test]
fn jianmie_tie_does_not_duel() {
    let mut g = game(&[
        ("zhangchunhua", &["jianmie"], 3, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 0);
    give(&mut g, 0, &["sp1"]);
    give(&mut g, 1, &["d1"]);
    skills::execute_active(&mut g, 0, "skill:jianmie").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:black");
    answer(&mut g, 1, "choose:red");
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 4, "平手不发生【决斗】");
}

// ---------------------------------------------------------------- 自守
#[test]
fn zishou_draw_bonus_and_end_phase_cost() {
    let mut g = game(&[("liubiao", &["zishou"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    g.phase = Phase::Judge;
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zishou"));
    answer(&mut g, 0, "opt1");
    begin_phase(&mut g, 0, Phase::Draw).unwrap();
    assert_eq!(g.counter("turn.zishou_used:0"), 1);
    assert_eq!(skills::draw_bonus(&g, 0), kingdom_count(&g));
    // 结束阶段：使用过伤害牌 → 弃 X 张。
    g.bump("turn.damage_card_used:0");
    g.phase = Phase::Play;
    give(&mut g, 0, &["d1", "d2", "d3"]);
    let before = g.hand(0).len();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zishou"));
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 0, "choose:d2");
    assert_eq!(g.hand(0).len(), before - 2, "结束阶段弃 X=2 张");
}

// ---------------------------------------------------------------- 宗室
#[test]
fn zongshi_guards_damage_once_per_kingdom_per_turn() {
    let mut g = game(&[
        ("liubiao", &["zongshi"], 3, "qun"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "qun"),
    ]);
    g.turn = 0;
    give(&mut g, 0, &["d1", "d2"]);
    g.seats[0].hp = 3;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 3, "宗室防止伤害");
    assert_eq!(pend_skill(&g).as_deref(), Some("zongshi"));
    answer(&mut g, 1, "pick:hand");
    assert_eq!(g.hand(1).len(), 1, "来源获得刘表一张牌");
    // 同势力同回合第二次：不再防止。
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 2);
    assert_eq!(skills::hand_limit_delta(&g, 0), 2, "手牌上限 +势力数");
}

// ---------------------------------------------------------------- 献州
#[test]
fn xianzhou_gives_equips_then_heals() {
    let mut g = game(&[
        ("caifuren", &["xianzhou"], 3, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    play_phase(&mut g, 0);
    equip(&mut g, 0, "w1", Slot::Weapon);
    equip(&mut g, 0, "arm1", Slot::Armor);
    g.seats[0].hp = 1;
    skills::execute_active(&mut g, 0, "skill:xianzhou").unwrap();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), 2, "两张装备牌交给接收者");
    assert_eq!(pend_actor(&g), 1);
    answer(&mut g, 1, "opt1");
    assert_eq!(g.seats[0].hp, 3, "回复 X=2 点体力");
    // 限定技：不能再次发动。
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:xianzhou"));
}

// ---------------------------------------------------------------- 窃听
#[test]
fn qieting_triggers_after_other_turn_end() {
    let mut g = game(&[
        ("caifuren", &["qieting"], 3, "qun"),
        ("p1", &[], 4, "wei"),
    ]);
    g.turn = 1;
    equip(&mut g, 1, "w1", Slot::Weapon);
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qieting"));
    assert_eq!(pend_actor(&g), 0);
    let before = g.hand(0).len();
    answer(&mut g, 0, "draw");
    assert_eq!(g.hand(0).len(), before + 1, "窃听摸一张");
    // 反例：该角色本回合造成过伤害且对其他角色使用过牌 → 不触发。
    let mut g2 = game(&[("caifuren", &["qieting"], 3, "qun"), ("p1", &[], 4, "wei")]);
    g2.turn = 1;
    g2.bump("turn.damage_by:1");
    g2.bump("turn.targeted_others:1");
    begin_phase(&mut g2, 0, Phase::Prepare).unwrap();
    assert_ne!(pend_skill(&g2).as_deref(), Some("qieting"));
}

// ---------------------------------------------------------------- 称象
#[test]
fn chengxiang_reveals_gains_and_stacks_bonus() {
    let mut g = game(&[("caochong", &["chengxiang"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 1;
    // 牌堆顶 4 张：d1(2) d2(3) t2(8) d3(4)。
    for id in ["d3", "t2", "d2", "d1"] {
        g.put_on_top(&id.to_string()).unwrap();
    }
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("chengxiang"));
    answer(&mut g, 0, "opt1");
    assert_eq!(pend_skill(&g).as_deref(), Some("chengxiang"));
    for id in ["d1", "d2", "t2"] {
        answer(&mut g, 0, &format!("choose:{id}"));
    }
    let before = g.hand(0).len();
    answer(&mut g, 0, "confirm");
    assert_eq!(g.hand(0).len(), before + 3, "获得 3 张（和 13）");
    assert_eq!(
        g.seats[0].counters.get("chengxiang_bonus").copied(),
        Some(1),
        "点数和恰为 13 → 下次多亮一张（可累积）"
    );
}

#[test]
fn chengxiang_triggers_per_damage_point() {
    let mut g = game(&[("caochong", &["chengxiang"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 1;
    flow::damage(&mut g, Some(1), 0, 2);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("chengxiang"));
    answer(&mut g, 0, "pass");
    assert_eq!(pend_skill(&g).as_deref(), Some("chengxiang"), "2 点伤害触发 2 次");
    answer(&mut g, 0, "pass");
}

// ---------------------------------------------------------------- 仁心
#[test]
fn renxin_flips_and_heals_dying_other() {
    let mut g = game(&[
        ("caochong", &["renxin"], 3, "wei"),
        ("p1", &[], 1, "qun"),
    ]);
    g.turn = 1;
    equip(&mut g, 0, "w1", Slot::Weapon);
    flow::damage(&mut g, Some(1), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("renxin"));
    assert_eq!(pend_actor(&g), 0);
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:w1");
    assert_eq!(g.seats[1].hp, 1, "回复至 1 点体力");
    assert!(g.seats[0].flipped, "翻面");
    assert!(g.cards.contains_key("w1"), "装备牌仍存在（已进弃牌堆）");
}

// ---------------------------------------------------------------- 落英
#[test]
fn luoying_gains_other_players_club_discard() {
    let mut g = game(&[("caozhi", &["luoying"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 1;
    give(&mut g, 1, &["tl1"]); // 梅花
    g.to_discard(&"tl1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("luoying"));
    answer(&mut g, 0, "gain:tl1");
    assert!(g.hand(0).contains(&"tl1".to_string()));
    assert!(g.luoying_cards.contains("tl1"));
    // 反例：自己的梅花牌不触发。
    g.luoying_cards.clear();
    g.to_hand(&"tl1".to_string(), 0).unwrap();
    g.to_discard(&"tl1".to_string(), "test").unwrap();
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("luoying"));
}

// ---------------------------------------------------------------- 酒诗
#[test]
fn jiushi_flips_for_jiu_and_back_after_damage() {
    let mut g = game(&[("caozhi", &["jiushi"], 3, "wei"), ("p1", &[], 4, "qun")]);
    play_phase(&mut g, 0);
    skills::execute_active(&mut g, 0, "skill:jiushi").unwrap();
    answer(&mut g, 0, "opt1");
    assert!(g.seats[0].flipped);
    assert!(g.counter("turn.jiu_used:0") > 0);
    // 受伤后询问翻回。
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("jiushi"));
    answer(&mut g, 0, "opt1");
    assert!(!g.seats[0].flipped);
    // 反例：已使用过【酒】→ 不能再发动酒诗。
    assert!(!skills::legal_actions(&g, 0)
        .iter()
        .any(|(id, _)| id == "skill:jiushi"));
}

#[test]
fn jiushi_luoying_card_has_no_distance_while_flipped() {
    let mut g = game(&[("caozhi", &["jiushi"], 3, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 0;
    give(&mut g, 0, &["tl1"]);
    g.luoying_cards.insert("tl1".to_string());
    g.seats[0].flipped = true;
    assert!(skills::card_no_distance(&g, 0, Some(&"tl1".to_string())));
    assert!(skills::sha_no_distance(&g, 0, &"tl1".to_string()));
    g.seats[0].flipped = false;
    assert!(!skills::card_no_distance(&g, 0, Some(&"tl1".to_string())));
}

// ---------------------------------------------------------------- 甘露
#[test]
fn ganlu_swaps_equips_and_pays_diff_when_needed() {
    let mut g = game(&[
        ("wuguotai", &["ganlu"], 3, "wu"),
        ("p1", &[], 4, "wei"),
        ("p2", &[], 4, "qun"),
    ]);
    play_phase(&mut g, 0);
    equip(&mut g, 1, "w1", Slot::Weapon);
    equip(&mut g, 1, "arm1", Slot::Armor);
    equip(&mut g, 1, "horse1", Slot::HorseOffense);
    equip(&mut g, 2, "w2", Slot::Weapon);
    give(&mut g, 0, &["d1", "d2"]);
    g.seats[0].hp = 3; // 已损失 1 < 差额 2 → 弃 2 张手牌
    skills::execute_active(&mut g, 0, "skill:ganlu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "target:2");
    assert_eq!(g.equip_in(1, Slot::Weapon), Some("w2".to_string()), "p1 获得 p2 的武器");
    assert_eq!(g.equip_in(2, Slot::Weapon), Some("w1".to_string()), "p2 获得 p1 的武器");
    assert_eq!(g.equip_in(2, Slot::Armor), Some("arm1".to_string()));
    assert_eq!(g.equip_in(1, Slot::Armor), None);
    assert_eq!(g.hand(0).len(), 0, "差额超过已损失体力：弃 2 张手牌");
}

// ---------------------------------------------------------------- 补益
#[test]
fn buyi_reveals_non_basic_and_heals_dying() {
    let mut g = game(&[
        ("wuguotai", &["buyi"], 3, "wu"),
        ("p1", &[], 1, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    g.turn = 1;
    give(&mut g, 1, &["gh1"]); // 非基本牌
    flow::damage(&mut g, Some(2), 1, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("buyi"));
    answer(&mut g, 0, "opt1");
    answer(&mut g, 0, "pick:hand:gh1");
    assert_eq!(g.seats[1].hp, 1, "弃非基本牌并回复 1 点体力");
    assert!(!g.hand(1).contains(&"gh1".to_string()));
}

#[test]
fn buyi_skips_when_victim_has_no_card() {
    let mut g = game(&[
        ("wuguotai", &["buyi"], 3, "wu"),
        ("p1", &[], 1, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    g.turn = 1;
    flow::damage(&mut g, Some(2), 1, 1);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("buyi"));
}

// ---------------------------------------------------------------- 当先
#[test]
fn dangxian_extra_play_phase_and_self_damage() {
    let mut g = game(&[("liaohua", &["dangxian"], 4, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 0;
    g.phase = Phase::Prepare;
    sanguosha2::turn::tick(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("dangxian"), "额外出牌阶段开始时获得【杀】");
    answer(&mut g, 0, "opt1");
    assert_eq!(g.phase, Phase::Play, "准备阶段后进入额外出牌阶段");
    assert_eq!(g.no_distance_cards.len(), 1, "获得的【杀】无距离限制");
    // 未造成伤害 → 阶段结束自伤 1，并回到判定阶段。
    let hp = g.seats[0].hp;
    let next = skills::on_end_play(&mut g, 0);
    assert_eq!(next, Phase::Judge);
    assert_eq!(g.seats[0].hp, hp - 1, "未造成伤害自伤 1 点");
}

// ---------------------------------------------------------------- 伏枥
#[test]
fn fuli_heals_draws_and_flips_when_x_gt_damage() {
    let mut g = game(&[("liaohua", &["fuli"], 4, "shu"), ("p1", &[], 4, "wei")]);
    g.turn = 1;
    g.seats[0].hp = 1;
    flow::damage(&mut g, Some(1), 0, 1);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("fuli"));
    answer(&mut g, 0, "opt1");
    let x = 2i32; // shu + wei（濒死者自己的势力也算）
    assert_eq!(g.seats[0].hp, x, "回复至 X 点");
    assert_eq!(g.hand(0).len(), x as usize, "手牌摸至 X 张");
    assert!(g.seats[0].flipped, "X > 本局造成伤害数 → 翻面");
    assert!(g.seats[0].flags.contains("fuli_used"));
    // 限定技：第二次濒死不提供。
    flow::damage(&mut g, Some(1), 0, 99);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("fuli"));
}
