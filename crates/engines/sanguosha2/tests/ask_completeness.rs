//! I1 询问完整性（v1 口径，用户 2026-10-28 裁定 / 2026-11-12 重审）。
//!
//! 规则只有一条：**只有「合法动作集为空」才允许不询问**。
//! - 无手牌 → 打不出任何牌 → 不询问（正确跳过）；
//! - 有手牌 → 即使没有对应的牌，也必须显式询问（保留犹豫博弈；引擎不得代放弃、代选择）。
//!
//! 覆盖三类响应窗口：闪/杀（card_response）、无懈（wuxie）、濒死（dying）。

use sanguosha2::cards;
use sanguosha2::kernel::pending::Pending;
use sanguosha2::kernel::resolve::{begin_phase, submit};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Zone};

fn new_game(n: usize) -> Game {
    let seats: Vec<Seat> = (0..n)
        .map(|i| Seat::new(&format!("p{i}"), "guest", &[], 4))
        .collect();
    let mut g = Game::new(seats, cards::deck());
    begin_phase(&mut g, 0, Phase::Play).expect("进入 p0 出牌阶段");
    g
}

fn give(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == name)
        .unwrap_or_else(|| panic!("牌堆里没有【{name}】"));
    g.to_hand(&id, seat).expect("给牌");
    id
}

fn pend(g: &Game) -> &Pending {
    g.pending.as_ref().expect("应有 pending")
}

fn option_ids(g: &Game) -> Vec<String> {
    pend(&g).options().iter().map(|o| o.id.clone()).collect()
}

/// 闪/杀：**有手牌但没有【闪】→ 仍要询问**（选项只有出口）。
#[test]
fn target_with_hand_without_shan_is_still_asked() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    give(&mut g, 1, "桃"); // 有手牌，但打不出【闪】
    let hp = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用【杀】");
    assert_eq!(pend(&g).actor(), 1, "有手牌 → 必须显式询问");
    assert_eq!(option_ids(&g), vec!["pass".to_string()], "唯一合法动作是放弃");
    // 放弃 → 才结算伤害
    submit(&mut g, 1, "pass").expect("放弃响应");
    assert_eq!(g.seats[1].hp, hp - 1);
}

/// 闪/杀：**空手牌 → 不询问**，直接结算（合法动作集为空）。
#[test]
fn empty_hand_target_is_not_asked() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    let hp = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用【杀】");
    assert!(g.pending.is_none(), "空手牌不得询问：{:?}", g.pending);
    assert_eq!(g.seats[1].hp, hp - 1);
}

/// 无懈：有手牌但没有【无懈可击】→ 仍要询问。
#[test]
fn wuxie_window_asks_players_with_hand() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "无中生有");
    give(&mut g, 1, "桃"); // 有手牌，无【无懈可击】
    g.resolve_use(0, &card, vec![]).expect("使用锦囊");
    // 从使用者左手侧开始询问；p1 有手牌 → 被问（只有出口）。
    assert_eq!(pend(&g).actor(), 1, "有手牌 → 必须显式询问无懈");
    assert_eq!(option_ids(&g), vec!["pass".to_string()]);
}

/// 濒死：有手牌但没有【桃】→ 仍要询问。
#[test]
fn dying_asks_players_with_hand() {
    let mut g = new_game(3);
    give(&mut g, 1, "桃"); // 旁人手里有牌（不是【桃】以外的救援手段也行，这里给【桃】给濒死者无关）
    give(&mut g, 2, "杀"); // 有手牌，无【桃】
    g.seats[1].hp = 1;
    // p0 对 p1 造成伤害 → p1 濒死（p1 有【桃】可自救）
    sanguosha2::kernel::flow::damage(&mut g, Some(0), 1, 1);
    // 驱动到濒死询问
    let mut guard = 0;
    while g.pending.is_none() && guard < 8 {
        sanguosha2::kernel::resolve::advance(&mut g).expect("advance");
        guard += 1;
    }
    assert!(g.pending.is_some(), "濒死应询问");
    // 濒死者本人有【桃】→ 拒绝后，其他人（有手牌）也必须被询问
    let first = pend(&g).actor();
    let escape = pend(&g).escape_id().to_string();
    submit(&mut g, first, &escape).expect("放弃自救");
    assert!(
        g.pending.is_some(),
        "有手牌的其他人仍须被询问（不得静默跳过）"
    );
}

/// 强制选择类：提交占位出口必须被拒绝（I1：不得代选）。
#[test]
fn mandatory_pending_rejects_escape() {
    let mut g = new_game(3);
    let card = give(&mut g, 0, "过河拆桥");
    give(&mut g, 1, "桃");
    g.resolve_use(0, &card, vec![1]).expect("使用【过河拆桥】");
    // 拆/顺的取牌步骤是强制选择：出口只是占位。
    let mut guard = 0;
    while g.pending.as_ref().map(|p| p.kind()) != Some("card_pick") && guard < 12 {
        let actor = pend(&g).actor();
        let escape = pend(&g).escape_id().to_string();
        submit(&mut g, actor, &escape).expect("排空礼貌询问");
        guard += 1;
    }
    assert!(pend(&g).escape_hidden(), "取牌步骤应为强制选择");
    let err = submit(&mut g, 0, "pass").unwrap_err();
    assert!(err.contains("必须选择"), "{err}");
    assert!(g.pending.is_some(), "被拒绝后待办仍在");
}

/// 拆/顺手：`pick:hand` 必须是**随机**一张（不得固定取第一张，见 I1 代选）。
#[test]
fn pick_hand_is_random_not_first() {
    let mut decided = std::collections::BTreeSet::new();
    for seed in 0..24u64 {
        let mut g = new_game(3);
        g.rand_state = 0x5EED_0000 + seed;
        let card = give(&mut g, 0, "过河拆桥");
        for name in ["桃", "杀", "闪"] {
            give(&mut g, 1, name);
        }
        let before: Vec<CardId> = g.hand(1).to_vec();
        g.resolve_use(0, &card, vec![1]).expect("使用【过河拆桥】");
        let mut guard = 0;
        while g.pending.as_ref().map(|p| p.kind()) != Some("card_pick") && guard < 12 {
            let actor = pend(&g).actor();
            let escape = pend(&g).escape_id().to_string();
            submit(&mut g, actor, &escape).expect("排空礼貌询问");
            guard += 1;
        }
        // 无懈窗口先排空后再取牌
        submit(&mut g, 0, "pick:hand").expect("随机取一张手牌");
        let removed: Vec<CardId> = before
            .into_iter()
            .filter(|c| !g.hand(1).contains(c))
            .collect();
        assert_eq!(removed.len(), 1, "应弃置恰好一张手牌");
        decided.insert(removed[0].clone());
    }
    assert!(
        decided.len() > 1,
        "`pick:hand` 必须随机（不得固定第一张）：{decided:?}"
    );
}

/// 门禁锚点：三类响应窗口的 kind 名（端侧/测试共用的单一事实源）。
#[test]
fn response_pending_kinds_are_stable() {
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    give(&mut g, 1, "桃");
    g.resolve_use(0, &sha, vec![1]).expect("使用【杀】");
    assert_eq!(pend(&g).kind(), "card_response");
    assert_eq!(pend(&g).skill(), Some("__card"));
}
