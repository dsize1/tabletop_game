//! 谮毁「结算前」钩子（S2 销号）：代用 / 额外目标必须在**结算开始之前**生效。
//!
//! 旧实现（B5）在 `TargetChosen`（结算之后）才询问，只能记状态；本文件锁定新时序：
//! - 额外目标：`p2` 真的被追加为目标并结算一次；
//! - 代用：伤害来源变成协助者（用 Presentation.actor 断言）。

use sanguosha2::cards;
use sanguosha2::kernel::resolve::{begin_phase, submit};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Zone};

fn game() -> Game {
    let seats = vec![
        Seat::new("p0", "sgs_hero_392", &["zanhui", "jiaojin"], 4), // 孙鲁班（谮毁）
        Seat::new("p1", "guest", &[], 4),
        Seat::new("p2", "guest", &[], 4),
    ];
    let mut g = Game::new(seats, cards::deck());
    begin_phase(&mut g, 0, Phase::Play).expect("进入 p0 出牌阶段");
    g
}

fn give(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == name)
        .unwrap_or_else(|| panic!("牌堆缺少【{name}】"));
    g.to_hand(&id, seat).unwrap();
    id
}

fn answer(g: &mut Game, action: &str) {
    let p = g.pending.clone().expect("应有待办");
    submit(g, p.actor(), action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
}

#[test]
fn extra_target_is_really_added_before_settlement() {
    let mut g = game();
    let sha = give(&mut g, 0, "杀");
    let before = (g.seats[1].hp, g.seats[2].hp);

    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    // 结算前钩子：p0 选协助者 p2。
    let p = g.pending.clone().expect("应有谮毁询问");
    assert_eq!(p.skill(), Some("zanhui"));
    assert_eq!(p.actor(), 0);
    assert!(
        p.has_option("target:2"),
        "应能选 p2 作为协助者：{:?}",
        p.options().iter().map(|o| &o.id).collect::<Vec<_>>()
    );
    answer(&mut g, "target:2");
    // p2 选「成为额外目标」。
    let p = g.pending.clone().expect("协助者选择");
    assert_eq!(p.actor(), 2);
    assert!(p.has_option("opt2"));
    answer(&mut g, "opt2");

    // 结算：两个目标都吃伤害（额外目标真的进了结算）。
    let mut guard = 0;
    while g.pending.is_some() && guard < 10 {
        guard += 1;
        answer(&mut g, "pass");
    }
    assert_eq!(g.seats[1].hp, before.0 - 1);
    assert_eq!(g.seats[2].hp, before.1 - 1, "额外目标应真的掉血");
}

#[test]
fn switch_user_changes_damage_source() {
    let mut g = game();
    let sha = give(&mut g, 0, "杀");
    let gift = give(&mut g, 2, "桃");

    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    answer(&mut g, "target:2");
    // p2 选「代用」并交出一张牌。
    answer(&mut g, "opt1");
    answer(&mut g, &format!("choose:{gift}"));

    // 结算：p1 不出闪 → 伤害来源应为 p2。
    let mut guard = 0;
    while g.pending.is_some() && guard < 10 {
        guard += 1;
        answer(&mut g, "pass");
    }
    assert!(g.hand(0).contains(&gift), "交出的牌应到原使用者手里");
    assert!(
        g.log
            .iter()
            .any(|e| e.text.starts_with("p2 对 p1 造成")),
        "代用后伤害来源应为协助者 p2：{:?}",
        g.log.iter().map(|e| e.text.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn pre_use_window_is_once_per_turn_after_activation() {
    let mut g = game();
    // 诸葛连弩：允许本回合使用多张【杀】（只为测试第二次窗口）。
    let bow = give(&mut g, 0, "诸葛连弩");
    g.to_equip(&bow, 0, sanguosha2::kernel::Slot::Weapon).unwrap();
    let a = give(&mut g, 0, "杀");
    let b = give(&mut g, 0, "杀");
    g.resolve_use(0, &a, vec![1]).expect("第一张杀");
    // 发动谮毁（协助者 p2 成为额外目标）。
    answer(&mut g, "target:2");
    answer(&mut g, "opt2");
    while g.pending.is_some() {
        answer(&mut g, "pass");
    }
    // 第二张杀：本回合已发动过 → 不再有 preuse 询问（直接进入响应窗口）。
    g.resolve_use(0, &b, vec![1]).expect("第二张杀");
    assert_ne!(
        g.pending.as_ref().and_then(|p| p.skill()),
        Some("zanhui"),
        "本回合已发动过谮毁，不应再次询问"
    );
}

#[test]
fn black_trick_also_has_window_but_red_one_does_not() {
    let mut g = game();
    // 黑色普通锦囊（过河拆桥）→ 有窗口。
    let black = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| {
            g.name_of(id) == "过河拆桥"
                && g.cards.get(id).map(|c| !c.suit.is_red()).unwrap_or(false)
        })
        .expect("应有黑色过河拆桥");
    g.to_hand(&black, 0).unwrap();
    g.resolve_use(0, &black, vec![1]).expect("使用过河拆桥");
    assert_eq!(
        g.pending.as_ref().and_then(|p| p.skill()),
        Some("zanhui"),
        "黑色普通锦囊应触发谮毁窗口"
    );
}
