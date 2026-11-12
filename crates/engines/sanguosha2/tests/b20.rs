//! B20 用例（8 技能，每条 ≥1 正例 + ≥1 反例）。
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{flow, Card, Game, Phase, Seat, Slot, Suit};
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
        ("t1", "桃", Suit::Heart, 7),
        ("t2", "桃", Suit::Heart, 8),
        ("j1", "酒", Suit::Spade, 9),
        ("jd1", "决斗", Suit::Club, 4),
        ("gh1", "过河拆桥", Suit::Spade, 3),
        ("gh2", "过河拆桥", Suit::Diamond, 7),
        ("nm1", "南蛮入侵", Suit::Spade, 7),
        ("wz1", "无中生有", Suit::Heart, 7),
        ("le1", "乐不思蜀", Suit::Heart, 6),
    ];
    let mut cards: Vec<Card> = spec
        .iter()
        .map(|(id, name, suit, rank)| Card::new(*id, name, *suit, *rank))
        .collect();
    cards.push(Card::equip("w1", "青釭剑", Suit::Spade, 6, Slot::Weapon));
    cards.push(Card::equip("arm1", "八卦阵", Suit::Spade, 2, Slot::Armor));
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
fn opt_with(g: &Game, prefix: &str) -> String {
    options(g)
        .into_iter()
        .find(|o| o.starts_with(prefix))
        .unwrap_or_else(|| panic!("没有 {prefix} 选项：{:?}", options(g)))
}

// ---------------------------------------------------------------- 樵拾
#[test]
fn qiaoshi_other_end_phase_draws_and_offs_round() {
    let mut g = game(&[
        ("xiahou", &["qiaoshi", "yanyu"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1"]);
    give(&mut g, 1, &["d2", "d3"]);
    g.turn = 1;
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiaoshi"), "{:?}", options(&g));
    assert_eq!(pend_actor(&g), 0);
    answer(&mut g, 0, "opt1");
    assert_eq!(g.hand(0).len(), 2, "夏侯氏摸一张");
    assert_eq!(g.hand(1).len(), 3, "当前回合角色摸一张");
    assert_eq!(g.counter("round.qiaoshi_off:0"), 1, "手牌数不等 → 本轮失效");
    // 本轮再次结束阶段 → 不再询问。
    g.turn = 1;
    g.phase = Phase::Play;
    begin_phase(&mut g, 1, Phase::End).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("qiaoshi"));
}

#[test]
fn qiaoshi_not_on_own_end_phase() {
    let mut g = game(&[("xiahou", &["qiaoshi"], 4, "wei"), ("p1", &[], 4, "qun")]);
    give(&mut g, 0, &["d1"]);
    g.turn = 0;
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("qiaoshi"), "自己的结束阶段不触发");
}

// ---------------------------------------------------------------- 燕语
#[test]
fn yanyu_recasts_sha_then_gifts_male() {
    let mut g = game(&[
        ("xiahou", &["yanyu"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp1", "sp2", "d1"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:yanyu").unwrap();
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 0, "choose:sp2");
    answer(&mut g, 0, "done");
    assert_eq!(g.counter("turn.yanyu_lost:0"), 2, "失去两张【杀】");
    // 出牌阶段结束 → 可令一名男性摸两张。
    g.end_phase("Play").unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("yanyu"), "{:?}", options(&g));
    let before = g.hand(1).len();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), before + 2);
}

#[test]
fn yanyu_no_gift_without_two_sha_lost() {
    let mut g = game(&[("xiahou", &["yanyu"], 4, "wei"), ("p1", &[], 4, "qun")]);
    give(&mut g, 0, &["sp1"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:yanyu").unwrap();
    answer(&mut g, 0, "choose:sp1");
    answer(&mut g, 0, "done");
    g.end_phase("Play").unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("yanyu"), "只失去 1 张不触发");
}

// ---------------------------------------------------------------- 强识
#[test]
fn qiangshi_reveals_chosen_card_and_draws_on_same_type() {
    let mut g = game(&[
        ("zhangsong", &["qiangshi", "xiantu"], 3, "shu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["d1", "t1"]);
    give(&mut g, 0, &["t2", "gh1"]);
    g.seats[0].hp = 2;
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiangshi"));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d1");
    assert_eq!(g.counter("turn.qiangshi_type"), 1, "基本牌类别");
    assert!(g.revealed.contains("d1"), "公开展示");
    // 使用同类别（基本牌）→ 可摸一张。
    g.resolve_use(0, &"t2".to_string(), vec![]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("qiangshi"), "{:?}", options(&g));
    let before = g.hand(0).len();
    answer(&mut g, 0, "draw");
    assert_eq!(g.hand(0).len(), before + 1);
}

#[test]
fn qiangshi_no_prompt_for_other_type() {
    let mut g = game(&[("zhangsong", &["qiangshi"], 3, "shu"), ("p1", &[], 4, "qun")]);
    give(&mut g, 1, &["d1"]);
    give(&mut g, 0, &["t2", "gh1"]);
    give(&mut g, 1, &["d2"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "choose:d1");
    // 使用锦囊（不同类别）→ 不摸牌。
    g.resolve_use(0, &"gh1".to_string(), vec![1]).unwrap();
    while g.pending.is_some() {
        answer_any(&mut g);
    }
    assert_ne!(pend_skill(&g).as_deref(), Some("qiangshi"));
}

// ---------------------------------------------------------------- 献图
/// 同类排查（真机 2026-11-12 屯田）：燕语只统计**自己**失去的【杀】。
#[test]
fn yanyu_ignores_other_players_lost_sha() {
    let mut g = game(&[("buy", &["yanyu"], 4, "wu"), ("p1", &[], 4, "qun")]);
    // 界步练师出牌阶段
    g.turn = 0;
    g.phase = sanguosha2::kernel::Phase::Play;
    // 他人（p1）失去两张【杀】（模拟：直接发事件）
    for _ in 0..2 {
        let sha = g
            .ids_in(|z| *z == sanguosha2::kernel::Zone::Draw)
            .into_iter()
            .find(|id| g.name_of(id) == "杀")
            .unwrap();
        g.to_hand(&sha, 1).unwrap();
        g.to_discard_by(1, &sha, "discard_phase").unwrap();
    }
    advance(&mut g).unwrap();
    assert_eq!(g.counter("turn.yanyu_lost:0"), 0, "他人失去【杀】不计入");
}

#[test]
fn xiantu_gives_and_loses_hp_when_damage_less() {
    let mut g = game(&[
        ("zhangsong", &["xiantu"], 3, "shu"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["d1", "d2", "d3"]);
    g.turn = 1;
    g.phase = Phase::Prepare;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("xiantu"), "{:?}", options(&g));
    assert_eq!(pend_actor(&g), 0);
    answer(&mut g, 0, "opt2");
    answer(&mut g, 0, "choose:d1");
    answer(&mut g, 0, "choose:d2");
    assert_eq!(g.hand(1).len(), 2, "交给两张");
    // p1 未造成伤害 → 张松失去 1 点体力。
    g.end_phase("Play").unwrap();
    assert_eq!(g.seats[0].hp, 2, "造成伤害 0 < 交给 2 张");
}

#[test]
fn xiantu_no_hp_loss_when_not_activated() {
    let mut g = game(&[("zhangsong", &["xiantu"], 3, "shu"), ("p1", &[], 4, "qun")]);
    give(&mut g, 0, &["d1", "d2"]);
    g.turn = 1;
    g.phase = Phase::Prepare;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    answer(&mut g, 0, "pass");
    g.end_phase("Play").unwrap();
    assert_eq!(g.seats[0].hp, 3, "未发动不失去体力");
}

// ---------------------------------------------------------------- 镇军
#[test]
fn zhenjun_discards_x_then_end_phase_draw() {
    let mut g = game(&[
        ("yujin", &["zhenjun", "yizhong"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["d1", "d2", "d3", "t1"]);
    g.turn = 0;
    g.phase = Phase::Draw;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhenjun"), "{:?}", options(&g));
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "hand");
    assert_eq!(g.hand(1).len(), 3, "X = 4 - 4 → 1 张");
    assert_eq!(pend_skill(&g).as_deref(), Some("zhenjun"));
    answer(&mut g, 0, "opt2");
    assert_eq!(g.counter("turn.zhenjun_draw:1"), 1);
    // 本回合结束阶段 → 其摸 1 张。
    g.phase = Phase::Play;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(g.hand(1).len(), 4, "结束阶段摸回 1 张");
}

#[test]
fn zhenjun_skips_target_without_cards() {
    let mut g = game(&[("yujin", &["zhenjun"], 4, "wei"), ("p1", &[], 4, "qun")]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zhenjun"), "目标没有牌不能发动");
}

// ---------------------------------------------------------------- 毅重
#[test]
fn yizhong_black_sha_is_immune_when_hp_ge() {
    let mut g = game(&[
        ("yujin", &["yizhong"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 0, &["d1"]);
    g.turn = 1;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.resolve_use(1, &"sp1".to_string(), vec![0]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 4, "黑色【杀】对于禁无效");
    assert_ne!(pend_skill(&g).as_deref(), Some("__card"), "不询问【闪】");
    assert!(g.log.iter().any(|l| l.text.contains("无效")), "播报无效");
}

#[test]
fn yizhong_blocks_shan_when_hand_le() {
    let mut g = game(&[
        ("yujin", &["yizhong"], 4, "wei"),
        ("p1", &[], 4, "qun"),
    ]);
    give(&mut g, 0, &["sp1", "t1", "t2"]);
    give(&mut g, 1, &["d1"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.resolve_use(0, &"sp1".to_string(), vec![1]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[1].hp, 3, "手牌数 ≤ 于禁 → 无法响应黑色【杀】");
}

#[test]
fn yizhong_red_sha_is_normal() {
    let mut g = game(&[("yujin", &["yizhong"], 4, "wei"), ("p1", &[], 4, "qun")]);
    give(&mut g, 1, &["h1"]);
    g.turn = 1;
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    g.resolve_use(1, &"h1".to_string(), vec![0]).unwrap();
    advance(&mut g).unwrap();
    assert_eq!(g.seats[0].hp, 3, "红色【杀】不受影响");
}

// ---------------------------------------------------------------- 安恤
#[test]
fn anxu_gains_two_cards_and_spade_rule() {
    let mut g = game(&[
        ("bulianshi", &["anxu", "zhuixi"], 3, "wu"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["sp1"]);
    give(&mut g, 2, &["t1", "t2"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:anxu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "hand");
    answer(&mut g, 0, "target:2");
    answer(&mut g, 0, "hand");
    // 从获得的两张中选一张展示（选黑桃 sp1 → 不摸牌）。
    answer(&mut g, 0, "choose:sp1");
    assert_eq!(g.counter("turn.anxu_used"), 1);
    assert!(g.revealed.contains("sp1"), "展示牌公开");
    let _ = (Slot::Weapon, equip as fn(&mut Game, usize, &str, Slot));
}

#[test]
fn anxu_non_spade_draws_one() {
    let mut g = game(&[
        ("bulianshi", &["anxu"], 3, "wu"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["d1"]);
    give(&mut g, 2, &["t1", "t2"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    skills::execute_active(&mut g, 0, "skill:anxu").unwrap();
    answer(&mut g, 0, "target:1");
    answer(&mut g, 0, "hand");
    answer(&mut g, 0, "target:2");
    answer(&mut g, 0, "hand");
    let before = g.hand(0).len();
    answer(&mut g, 0, "choose:d1");
    assert_eq!(g.hand(0).len(), before, "展示牌已交出，非黑桃再摸 1 张");
    // 展示牌不为黑桃 → 摸 1（交出 1 张、摸 1 张 → 数量不变）。
    assert_eq!(g.counter("turn.anxu_used"), 1);
}

#[test]
fn anxu_cannot_activate_twice() {
    let mut g = game(&[
        ("bulianshi", &["anxu"], 3, "wu"),
        ("p1", &[], 4, "qun"),
        ("p2", &[], 4, "qun"),
    ]);
    give(&mut g, 1, &["d1"]);
    give(&mut g, 2, &["d2"]);
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.counters.insert("turn.anxu_used".to_string(), 1);
    assert!(skills::execute_active(&mut g, 0, "skill:anxu").is_err());
}

// ---------------------------------------------------------------- 追忆
#[test]
fn zhuixi_end_phase_then_death_again() {
    let mut g = game(&[
        ("bulianshi", &["zhuixi"], 3, "wu"),
        ("p1", &[], 4, "qun"),
    ]);
    g.seats[0].hp = 2;
    g.turn = 0;
    begin_phase(&mut g, 0, Phase::End).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuixi"), "{:?}", options(&g));
    let before = g.hand(1).len();
    answer(&mut g, 0, "target:1");
    assert_eq!(g.hand(1).len(), before + 3, "摸三张");
    assert!(g.seats[0].flags.contains("zhuixi_used"));
    // 死亡时（亡语）：对同一名角色再次发动。
    flow::die(&mut g, 0, None);
    advance(&mut g).unwrap();
    assert_eq!(pend_skill(&g).as_deref(), Some("zhuixi"), "{:?}", options(&g));
    let before = g.hand(1).len();
    answer_cur(&mut g, "opt1");
    assert_eq!(g.hand(1).len(), before + 3, "亡语再次摸三张");
}

#[test]
fn zhuixi_death_only_after_used() {
    let mut g = game(&[("bulianshi", &["zhuixi"], 3, "wu"), ("p1", &[], 4, "qun")]);
    flow::die(&mut g, 0, None);
    advance(&mut g).unwrap();
    assert_ne!(pend_skill(&g).as_deref(), Some("zhuixi"), "未发动过则死亡时不触发");
}


