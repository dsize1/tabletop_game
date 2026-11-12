//! P-C 内核 spike 的验收测试：3 个试点技能的正例/反例 + 6 类缺陷门禁。
//!
//! 「缺陷注入」不是真的改代码，而是**断言这些缺陷在新的结构下写不出来 / 一定被抓到**：
//! 1. 内核里出现 per-skill 分支 → `gate_kernel_has_no_skill_branch`；
//! 2. per-skill 队列重现 → `gate_single_deferred_queue`；
//! 3. 帧持牌丢失 → `gate_conservation_after_every_step` + `check_no_orphan_frames`；
//! 4. legal/execute 两处各写一半 → `gate_legal_execute_same_source`；
//! 5. 文案与语义相反 → `gate_presentation_texts_disjoint`；
//! 6. 手写白名单漏项 → `gate_view_derived_from_registry`。

use sanguosha2::kernel::event::EventKind;
use sanguosha2::kernel::pending::Pending;
use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{Game, Phase, Zone};
use sanguosha2::presentation::POutcome;
use sanguosha2::skills;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn game_in_play() -> Game {
    let mut g = sanguosha2::demo_game();
    // 每人先摸 3 张（保证有手牌可选）。
    for seat in 0..g.seats.len() {
        g.draw(seat, 3);
    }
    begin_phase(&mut g, 0, Phase::Play).expect("进入出牌阶段");
    g
}

fn pending_of<'a>(g: &'a Game) -> &'a Pending {
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
    g.check_no_orphan_frames().expect("无帧遗留");
}

/// 触发「移出游戏」事件（良姻的触发源）。
fn move_one_card_out_of_game(g: &mut Game) {
    let victim = 1usize;
    let card = g.hand(victim).first().cloned().expect("目标应有手牌");
    g.to_void(&card).expect("移出游戏");
    advance(g).expect("推进");
}

// ---------------------------------------------------------------------------
// 良姻：正例
// ---------------------------------------------------------------------------

#[test]
fn liangyin_asks_on_first_out_of_game_in_own_turn() {
    let mut g = game_in_play();
    move_one_card_out_of_game(&mut g);
    let p = pending_of(&g);
    assert_eq!(p.actor(), 0, "应由周妃（p0）决定");
    assert!(p.has_option("choose:draw"));
    assert!(p.has_option("choose:discard"));
    assert_eq!(p.escape_id(), "pass", "必须有放弃出口");
}

#[test]
fn liangyin_draw_then_heal_when_hand_count_matches_kongsheng() {
    let mut g = game_in_play();
    // 放 2 张「箜声」牌 → X = 2；她受伤以便成为回血候选。
    // （手牌 3 张，放 2 张后剩 1，良姻各摸 1 后为 2 == X）
    let mark_cards: Vec<String> = g.hand(0).into_iter().take(2).collect();
    for id in &mark_cards {
        g.to_mark(id, 0, "kongsheng").unwrap();
    }
    g.seats[0].hp = 2;
    move_one_card_out_of_game(&mut g);
    answer(&mut g, 0, "choose:draw");
    // 选目标
    let target = pending_of(&g)
        .options()
        .iter()
        .find(|o| o.id.starts_with("target:"))
        .expect("有目标候选")
        .id
        .clone();
    answer(&mut g, 0, &target);
    // 回血候选：周妃手牌数 == 1（箜声 1 张）且未满血
    let p = pending_of(&g);
    assert!(p.has_option("choose:heal:0"), "周妃应可回血：{:?}", p.options());
}

#[test]
fn liangyin_heal_silently_skipped_when_no_candidate() {
    let mut g = game_in_play();
    // 周妃满血 → 不在候选；X = 0（无箜声）→ 目标手牌数不为 0 也不在候选。
    move_one_card_out_of_game(&mut g);
    answer(&mut g, 0, "choose:draw");
    let target = pending_of(&g)
        .options()
        .iter()
        .find(|o| o.id.starts_with("target:"))
        .unwrap()
        .id
        .clone();
    answer(&mut g, 0, &target);
    assert!(g.pending.is_none(), "候选为空时不得询问（复验 25）");
}

// ---------------------------------------------------------------------------
// 良姻：反例（窗口 / 每回合首次）
// ---------------------------------------------------------------------------

#[test]
fn liangyin_does_not_trigger_on_other_players_turn() {
    let mut g = game_in_play();
    // 切到 p1 的回合：周妃（p0）的 OwnTurn 窗口不成立。
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    assert!(g.pending.is_none(), "切回合不应残留 pending");
    let card = g.hand(2).first().cloned().unwrap();
    g.to_void(&card).unwrap();
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "他人回合内不得询问良姻（用户裁定）");
}

#[test]
fn liangyin_only_first_time_per_turn_per_direction() {
    let mut g = game_in_play();
    move_one_card_out_of_game(&mut g);
    answer(&mut g, 0, "pass");
    assert!(g.pending.is_none());
    // 同一回合第二次「移出游戏」→ 不再询问
    move_one_card_out_of_game(&mut g);
    assert!(g.pending.is_none(), "每回合首次：第二次不得再问");
}

// ---------------------------------------------------------------------------
// 箜声：准备置牌 → 结束取回非装备 → 使用剩余装备
// ---------------------------------------------------------------------------

#[test]
fn kongsheng_places_on_prepare_and_uses_equip_on_end() {
    let mut g = game_in_play();
    // 多摸 3 张，保证手牌里同时有非装备与装备。
    g.draw(0, 3);
    // 准备阶段：触发箜声（PhaseStart Prepare）。
    begin_phase(&mut g, 0, Phase::Prepare).unwrap();
    let p = pending_of(&g);
    assert_eq!(p.skill(), Some("kongsheng"));
    let exit = p.escape_id().to_string();
    // 放 1 张非装备（挑一张 name 非装备）
    let non_equip = g
        .hand(0)
        .into_iter()
        .find(|id| !g.is_equip(id))
        .expect("有非装备手牌");
    answer(&mut g, 0, &format!("choose:{non_equip}"));
    // 放 1 张装备
    let equip = g
        .hand(0)
        .into_iter()
        .find(|id| g.is_equip(id))
        .expect("有装备手牌");
    answer(&mut g, 0, &format!("choose:{equip}"));
    answer(&mut g, 0, &exit);
    assert_eq!(g.mark_cards(0, "kongsheng").len(), 2);

    // 结束阶段：非装备回手，装备进延迟队列 → 询问「令谁使用」
    begin_phase(&mut g, 0, Phase::End).unwrap();
    let p = pending_of(&g);
    assert_eq!(p.skill(), Some("kongsheng"));
    assert!(p.has_option("target:0"));
}

#[test]
fn kongsheng_all_equip_marks_keep_equip_and_does_not_return() {
    let mut g = game_in_play();
    g.draw(0, 3); // 保证手牌里有装备
    let equip = g.hand(0).into_iter().find(|id| g.is_equip(id)).unwrap();
    g.to_mark(&equip, 0, "kongsheng").unwrap();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    // 全装备 → 不回手，直接询问使用
    assert!(g.hand(0).iter().all(|id| *id != equip));
    assert_eq!(pending_of(&g).skill(), Some("kongsheng"));
}

// ---------------------------------------------------------------------------
// 贞良：阳（主动，legal/execute 同源）+ 阴（回合外）
// ---------------------------------------------------------------------------

#[test]
fn zhenliang_yang_is_legal_and_executable() {
    let mut g = game_in_play();
    // 给卢植（p1）放一张「任」= 红色，并构造他的出牌阶段。
    let red = g
        .hand(0)
        .into_iter()
        .find(|id| g.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false))
        .expect("有红牌");
    g.to_mark(&red, 1, "ren").unwrap();
    g.draw(1, 1);
    begin_phase(&mut g, 1, Phase::Play).unwrap();

    let legal = skills::legal_actions(&g, 1);
    let ids: Vec<&str> = legal.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"skill:zhenliang"), "legal 必须下发（同源）");

    let msg = skills::execute_active(&mut g, 1, "skill:zhenliang").unwrap();
    assert!(g.pending.is_some(), "应进入选目标：{}", msg);
    let target = g.pending.as_ref().unwrap().options()[0].id.clone();
    answer(&mut g, 1, &target);
    let card = pending_of(&g).options()[0].id.clone();
    answer(&mut g, 1, &card);
    assert!(!g.yang(1, "zhenliang"), "阳面发动后应转为阴");
    assert!(g.presentation.iter().any(|p| p.skill == "zhenliang"));
}

#[test]
fn zhenliang_yin_triggers_only_out_of_turn_and_same_color() {
    let mut g = game_in_play();
    let red = g
        .hand(0)
        .into_iter()
        .find(|id| g.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false))
        .unwrap();
    g.to_mark(&red, 1, "ren").unwrap();
    g.transform.insert("1:zhenliang".to_string(), false); // 阴面

    // 同色牌在**自己回合**打出 → 不触发（OtherTurn 窗口）
    begin_phase(&mut g, 1, Phase::Play).unwrap();
    let same = g
        .hand(2)
        .into_iter()
        .find(|id| g.cards.get(id).map(|c| c.suit.is_red()).unwrap_or(false))
        .unwrap();
    g.emit(sanguosha2::kernel::event::Event::card(
        EventKind::CardPlayed,
        Some(2),
        same.clone(),
        None,
    ));
    advance(&mut g).unwrap();
    assert!(g.pending.is_none(), "自己回合内阴面不触发");

    // 回到周妃回合，卢植（p1）回合外打出同色 → 触发
    begin_phase(&mut g, 0, Phase::Play).unwrap();
    g.emit(sanguosha2::kernel::event::Event::card(
        EventKind::CardPlayed,
        Some(2),
        same,
        None,
    ));
    advance(&mut g).unwrap();
    let p = pending_of(&g);
    assert_eq!(p.skill(), Some("zhenliang"));
    assert_eq!(p.actor(), 1);
}

// ---------------------------------------------------------------------------
// 门禁 1：内核不得出现 per-skill 分支
// ---------------------------------------------------------------------------

#[test]
fn gate_kernel_has_no_skill_branch() {
    let sources = [
        include_str!("../src/kernel/mod.rs"),
        include_str!("../src/kernel/resolve.rs"),
        include_str!("../src/kernel/event.rs"),
    ];
    for (name, src) in ["kernel/mod.rs", "kernel/resolve.rs", "kernel/event.rs"]
        .iter()
        .zip(sources)
    {
        for skill in ["liangyin", "kongsheng", "zhenliang"] {
            assert!(
                !src.contains(skill),
                "{name} 出现了技能名 `{skill}` —— 核心流程禁止 per-skill 分支"
            );
        }
        assert!(!src.contains("== \"skill:"), "{name} 出现了 action 字符串分支");
    }
}

// ---------------------------------------------------------------------------
// 门禁 2：只有一条延迟队列
// ---------------------------------------------------------------------------

#[test]
fn gate_single_deferred_queue() {
    let src = include_str!("../src/kernel/mod.rs");
    let queue_fields = src
        .lines()
        .filter(|l| l.trim_start().starts_with("pub ") && l.contains("_queue"))
        .count();
    assert_eq!(queue_fields, 0, "不得新增 per-skill 队列字段");
    assert!(src.contains("pub deferred: Vec<Deferred>"), "必须只有单一 deferred");
    let order = sanguosha2::kernel::deferred::ORDER;
    assert_eq!(order.len(), 5, "单一延迟队列：新增一项必须同步 ORDER");
    assert_eq!(order[0], sanguosha2::kernel::deferred::DeferredKind::LiangYinHeal);
    assert_eq!(order[1], sanguosha2::kernel::deferred::DeferredKind::KongShengUse);
    assert_eq!(
        order[order.len() - 1],
        sanguosha2::kernel::deferred::DeferredKind::DeathCleanup,
        "死亡清理必须排最后"
    );
    assert!(
        order
            .iter()
            .position(|k| *k == sanguosha2::kernel::deferred::DeferredKind::JudgePhase)
            .unwrap()
            > order
                .iter()
                .position(|k| *k == sanguosha2::kernel::deferred::DeferredKind::SkillTrigger)
                .unwrap(),
        "判定阶段结算必须在阶段开始技能触发之后"
    );
}

// ---------------------------------------------------------------------------
// 门禁 3：每个 pending 都有出口（类型保证 + 运行期断言）
// ---------------------------------------------------------------------------

#[test]
fn gate_pending_always_has_escape() {
    let mut g = game_in_play();
    move_one_card_out_of_game(&mut g);
    let p = pending_of(&g);
    assert!(p.has_option(p.escape_id()), "出口必须在 options 里");
    assert!(["pass", "confirm"].contains(&p.escape_id()));

    // 箜声的多选也有出口
    let mut g2 = game_in_play();
    begin_phase(&mut g2, 0, Phase::Prepare).unwrap();
    let p2 = pending_of(&g2);
    assert!(p2.has_option(p2.escape_id()));
}

// ---------------------------------------------------------------------------
// 门禁 4：每步之后守恒 + 无帧遗留
// ---------------------------------------------------------------------------

#[test]
fn gate_conservation_after_every_step() {
    let mut g = game_in_play();
    g.check_conservation().unwrap();
    move_one_card_out_of_game(&mut g);
    g.check_conservation().unwrap();
    g.check_no_orphan_frames().unwrap();
    // 帧暂存 → 落盘后仍然守恒，且不滞留
    let card = g.hand(2).first().cloned().unwrap();
    g.to_frame(&card).unwrap();
    g.check_no_orphan_frames().expect("栈非空时允许帧暂存");
    sanguosha2::kernel::resolve::land(&mut g, &card, Zone::Hand(2)).unwrap();
    g.check_no_orphan_frames().unwrap();
    g.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 门禁 5：文案与语义互斥（无懈案例）
// ---------------------------------------------------------------------------

#[test]
fn gate_presentation_texts_disjoint() {
    let neg = POutcome::TrickNegated.text("无懈可击");
    let eff = POutcome::TrickEffective.text("无懈可击");
    assert!(neg.contains("被抵消"), "{}", neg);
    assert!(eff.contains("照常生效"), "{}", eff);
    assert_ne!(neg, eff);
    assert!(POutcome::TrickNegated.is_inverse_of(&POutcome::TrickEffective));
    // 反向断言：不能出现「效果生效」这种歧义措辞
    assert!(!neg.contains("效果生效") && !eff.contains("效果生效"));
}

// ---------------------------------------------------------------------------
// 门禁 6：视图/动作由注册表派生（无手写白名单）
// ---------------------------------------------------------------------------

#[test]
fn gate_view_derived_from_registry() {
    let g = game_in_play();
    let expected: usize = g
        .seats
        .iter()
        .enumerate()
        .map(|(seat, _)| {
            skills::SKILLS
                .iter()
                .filter(|d| skills::active(&g, seat, d.code))
                .map(|d| d.triggers.len())
                .sum::<usize>()
        })
        .sum();
    assert_eq!(g.subs.len(), expected, "订阅必须由注册表派生");

    // 转换技状态单一来源：默认阳；切换后写入唯一状态表（不维护第二份清单）
    assert!(g.yang(1, "zhenliang"), "默认应为阳");
    let mut g2 = g.clone();
    g2.toggle_yang(1, "zhenliang");
    assert!(g2.transform.contains_key("1:zhenliang"));
    assert!(!g2.yang(1, "zhenliang"));
}
