//! P3 交互层门禁：pending 选牌/选目标/多选/二选一，接 advance 全流程。
//!
//! 验收口径：无技能时「出杀选目标 · 响应闪 · 弃牌阶段 · 无懈链 · 拆顺选牌」都可交互；
//! 提交非法选项/他人待办/强制选择的出口 → 拒绝且不改状态（constitution I1）。

mod common;

use common::{add, game, has_option, options, pending_actor, start_play, submit, use_card};
use sanguosha3::kernel::event::EventKind;
use sanguosha3::kernel::{Phase, Suit, Zone};

#[test]
fn play_phase_asks_with_card_options_and_end_escape() {
    let mut g = game(3);
    start_play(&mut g);
    assert_eq!(pending_actor(&g), Some(0));
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
    // 牌堆里全是【杀】，手上必然有【杀】→ 有出牌选项，且一定有出口（结束出牌阶段）。
    let opts = options(&g);
    assert!(opts.iter().any(|o| o.starts_with("card:")), "有出牌选项");
    assert!(has_option(&g, "end_play"), "有出口 end_play（Web「结束出牌」按钮）");
    assert_eq!(g.pending.as_ref().unwrap().escape_id(), "end_play");
}

#[test]
fn slash_asks_target_then_shan_blocks() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "s0", "sha", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    // 1) 出牌 → 选目标
    use_card(&mut g, 0, "s0").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "sgs3_target");
    assert!(has_option(&g, "t:1"));
    // 2) 选目标 → 目标出闪询问
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(pending_actor(&g), Some(1));
    assert_eq!(g.pending.as_ref().unwrap().kind(), "shan_ask");
    assert!(has_option(&g, "s:sh1"));
    // 3) 打出闪 → 抵消
    submit(&mut g, 1, "s:sh1").unwrap();
    assert_eq!(g.seats[1].hp, 4, "闪抵消，无伤害");
    assert_eq!(g.cards.get("s0"), Some(&Zone::Discard), "杀进弃牌堆");
    assert_eq!(g.cards.get("sh1"), Some(&Zone::Discard), "闪进弃牌堆");
    assert!(g.log.iter().any(|e| e.kind == EventKind::CardPlayed), "打出事件");
    assert!(g.log.iter().any(|e| e.kind == EventKind::TargetChosen), "目标事件");
    // 回到出牌阶段继续问
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
    assert_eq!(g.phase, Phase::Play);
}

#[test]
fn slash_without_shan_deals_damage_without_asking() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "s0", "sha", Zone::Hand(0));
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 目标没有【闪】→ 合法集为空 → 不询问（I1），直接结算。
    assert_eq!(g.seats[1].hp, 3);
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
}

#[test]
fn pending_rejects_wrong_actor_and_illegal_option() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "s0", "sha", Zone::Hand(0));
    let err = submit(&mut g, 1, "end").unwrap_err();
    assert!(err.contains("不属于"), "他人待办拒绝：{err}");
    let err = submit(&mut g, 0, "card:nope").unwrap_err();
    assert!(err.contains("非法选项"), "非法选项拒绝：{err}");
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act", "状态未变");
}

#[test]
fn multi_target_selection_uses_confirm_option() {
    let mut g = game(4);
    start_play(&mut g);
    // 铁索连环：1–2 个目标（多选 + 确定）。
    add(&mut g, "tie", "tiesuo_lianhuan", Zone::Hand(0));
    use_card(&mut g, 0, "tie").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "sgs3_target");
    assert!(!has_option(&g, "ok"), "还没选目标，不给「确定」");
    submit(&mut g, 0, "t:1").unwrap();
    assert!(has_option(&g, "ok"), "已选 1 个（下界）即可确定");
    submit(&mut g, 0, "t:2").unwrap();
    assert!(g.seats[1].chained && g.seats[2].chained, "横置");
}

#[test]
fn wuxie_chain_two_layers_lets_trick_through() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "wz", "wuzhong_shengyou", Zone::Hand(0));
    add(&mut g, "wx1", "wuxie_keji", Zone::Hand(1));
    add(&mut g, "wx2", "wuxie_keji", Zone::Hand(2));
    let before = g.hand_size(0);
    use_card(&mut g, 0, "wz").unwrap();
    // 层 1：座位 0 无无懈（自动跳过，不询问）→ 座位 1 有无懈。
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "w:wx1").unwrap();
    // 层 2：重新从座位 0 问起 → 座位 2 有无懈。
    assert_eq!(pending_actor(&g), Some(2));
    submit(&mut g, 2, "w:wx2").unwrap();
    // 两层 = 偶数 → 无中不抵消：摸 2 张（净 +1）。
    assert_eq!(g.hand_size(0), before - 1 + 2);
    assert_eq!(g.cards.get("wx1"), Some(&Zone::Discard));
    assert_eq!(g.cards.get("wx2"), Some(&Zone::Discard));
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
}

#[test]
fn wuxie_single_layer_cancels_trick() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "wz", "wuzhong_shengyou", Zone::Hand(0));
    add(&mut g, "wx1", "wuxie_keji", Zone::Hand(1));
    let before = g.hand_size(0);
    use_card(&mut g, 0, "wz").unwrap();
    // 只有座位 1 有无懈 → 直接问他
    assert_eq!(pending_actor(&g), Some(1));
    submit(&mut g, 1, "w:wx1").unwrap();
    // 一层无懈：其他人都没有 → 抵消
    assert_eq!(g.hand_size(0), before - 1, "无中没有摸牌（被抵消）");
    assert_eq!(g.cards.get("wz"), Some(&Zone::Discard));
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
}

#[test]
fn dismantle_asks_pick_and_discards() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "gh", "guohe_chaiqiao", Zone::Hand(0));
    add(&mut g, "h1", "tao", Zone::Hand(1));
    use_card(&mut g, 0, "gh").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    assert_eq!(g.pending.as_ref().unwrap().kind(), "sgs3_pick");
    assert!(has_option(&g, "c:h1"));
    submit(&mut g, 0, "c:h1").unwrap();
    assert_eq!(g.cards.get("h1"), Some(&Zone::Discard));
    assert!(g.log.iter().any(|e| e.kind == EventKind::CardLost), "失去牌事件");
}

#[test]
fn snatch_requires_distance_one() {
    let mut g = game(4);
    start_play(&mut g);
    add(&mut g, "ss", "shunshou_qianyang", Zone::Hand(0));
    add(&mut g, "h3", "tao", Zone::Hand(3));
    use_card(&mut g, 0, "ss").unwrap();
    // 座位 3 距离 1？4 人局 0→3 距离 1（环形）。
    assert!(has_option(&g, "t:3"));
    // 座位 2 距离 2 → 不可选
    assert!(!has_option(&g, "t:2"));
}

#[test]
fn choice_question_offers_each_option_and_escape() {
    // 雌雄双股剑：目标二选一（弃牌 / 令使用者摸牌）。
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "w", "cixiong_shuanggujian", Zone::Equip(0, sanguosha3::kernel::Slot::Weapon));
    add(&mut g, "s0", "sha", Zone::Hand(0));
    add(&mut g, "h1", "tao", Zone::Hand(1));
    g.seats[1].male = false;
    let before = g.hand_size(0);
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 目标无【闪】→ 命中前的雌雄双股剑二选一（无伤害前先问目标）
    assert_eq!(pending_actor(&g), Some(1));
    assert!(has_option(&g, "draw"), "二选一：令使用者摸一张牌");
    submit(&mut g, 1, "draw").unwrap();
    assert_eq!(g.seats[1].hp, 3, "选择摸牌后仍受伤害");
    assert_eq!(g.hand_size(0), before - 1 + 1, "杀已用掉 + 摸 1 张");
}

#[test]
fn suit_matters_for_black_slash_against_renwang() {
    let mut g = game(3);
    start_play(&mut g);
    add(&mut g, "armor", "renwang_dun", Zone::Equip(1, sanguosha3::kernel::Slot::Armor));
    add(&mut g, "s0", "sha", Zone::Hand(0));
    add(&mut g, "sh1", "shan", Zone::Hand(1));
    g.card_defs.get_mut("s0").unwrap().suit = Suit::Spade; // 黑杀
    use_card(&mut g, 0, "s0").unwrap();
    submit(&mut g, 0, "t:1").unwrap();
    // 黑杀被仁王盾无效 → 不问闪、不伤害。
    assert_eq!(g.pending.as_ref().unwrap().kind(), "play_act");
    assert_eq!(g.seats[1].hp, 4);
    assert_eq!(g.cards.get("sh1"), Some(&Zone::Hand(1)), "闪未被打出");
}
