//! M5 差分对拍：v1（oracle）↔ v3 的归一化摘要比对。
//!
//! 口径（`v3-parity-baseline.md` §4.1）：同 seed 同场景，比对「体力 / 存活 / 手牌数」。
//! 用**显式武将**（曹仁/袁术/周泰/严颜/王平，均 max_hp=4）固定体力基准，消除随机选将噪声。

use game_engine_core::SeededRng;
use sanguosha::sandbox::{new_sandbox_game, SandboxSeat, SandboxSpec};
use sanguosha::SgsState;
use sanguosha3::kernel::turn::begin_game;
use sanguosha3::kernel::view::summary;
use sanguosha3::kernel::{Game, Identity as V3Identity, Seat, Zone};

/// v1 沙盒开局（显式武将 + 身份）→ 归一化座位摘要（hp/alive/hand 数）。
fn v1_summary() -> Vec<(i32, bool, usize)> {
    let mut rng = SeededRng::new(1);
    let generals = ["sgs_hero_29", "sgs_hero_100", "sgs_hero_210", "sgs_hero_400", "sgs_hero_401"];
    let identities = [
        sanguosha::Identity::Lord,
        sanguosha::Identity::Loyalist,
        sanguosha::Identity::Rebel,
        sanguosha::Identity::Rebel,
        sanguosha::Identity::Spy,
    ];
    let seats: Vec<SandboxSeat> = (0..5)
        .map(|i| SandboxSeat {
            index: Some(i),
            player_id: None,
            general: Some(generals[i].to_string()),
            identity: Some(identities[i]),
        })
        .collect();
    let spec = SandboxSpec { players: 5, seats };
    let (state, _) = new_sandbox_game("r1", "g1", &spec, &mut rng).expect("v1 沙盒开局");
    let sgs = SgsState::parse(&state.data).expect("解析 v1 状态");
    (0..5).map(|i| (sgs.seats[i].hp, sgs.seats[i].alive, sgs.seats[i].hand.len())).collect()
}

/// v3 等价开局（max_hp 4，主公 +1）→ 归一化座位摘要。
fn v3_summary() -> Vec<(i32, bool, usize)> {
    let mut g = Game {
        seats: (0..5)
            .map(|i| Seat {
                id: i,
                hp: 4,
                max_hp: 4,
                alive: true,
                granted: vec![],
                skills: vec![],
                identity: Some(V3Identity::Lord),
                flipped: false, ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    for i in 0..22 {
        let id = format!("c{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.deck.push(id);
    }
    // 主公（座位 0）体力 +1。
    g.seats[0].max_hp = 5;
    g.seats[0].hp = 5;
    let ids = vec![
        V3Identity::Loyalist,
        V3Identity::Loyalist,
        V3Identity::Rebel,
        V3Identity::Rebel,
    ];
    begin_game(&mut g, &ids, 4);
    // v1 沙盒停在**主公出牌阶段**（已结算摸牌阶段）；v3 同步推进到同一阶段再比对。
    sanguosha3::kernel::flow::draw_phase(&mut g, 0);
    let s = summary(&g);
    (0..5)
        .map(|i| {
            (
                s["seats"][i]["hp"].as_i64().unwrap() as i32,
                s["seats"][i]["alive"].as_bool().unwrap(),
                s["seats"][i]["hand"].as_u64().unwrap() as usize,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 对拍：不变式一致（座位数 / 存活 / 非主公手牌数 / 主公体力 +1）
// ---------------------------------------------------------------------------

#[test]
fn parity_seat_count_and_alive_match() {
    let v1 = v1_summary();
    let v3 = v3_summary();
    assert_eq!(v1.len(), v3.len(), "座位数一致");
    for i in 0..5 {
        assert_eq!(v1[i].1, v3[i].1, "座位 {i} alive 一致");
    }
}

#[test]
fn parity_non_lord_hand_is_four() {
    // 非主公座位（1–4）初始手牌 = 4（v1 沙盒停在主公出牌阶段，其余座位尚未摸牌）。
    let v1 = v1_summary();
    let v3 = v3_summary();
    for i in 1..5 {
        assert_eq!(v1[i].2, v3[i].2, "座位 {i} 手牌数一致");
        assert_eq!(v3[i].2, 4, "座位 {i} 初始手牌 4 张");
    }
}

#[test]
fn parity_lord_hp_plus_one() {
    let v1 = v1_summary();
    let v3 = v3_summary();
    assert_eq!(v1[0].0, v3[0].0, "主公体力 v1≡v3（max_hp+1）");
    assert!(v3[0].0 > v3[1].0, "主公体力高于其他座位（+1）");
}

#[test]
fn parity_lord_first_turn_hand_is_six() {
    // 主公**首回合出牌阶段**手牌 = 4 张初始 + 摸牌阶段 2 张 = 6（v1≡v3）。
    // 注意：这不是「初始手牌 6」——2026-11-13 旧裁定把该 6 误读为初始手牌，
    // 2026-11-13 B1 对拍复核已回正（见 `v3-parity-baseline.md` §5.1）。
    let v1 = v1_summary();
    let v3 = v3_summary();
    assert_eq!(v1[0].2, 6, "v1 主公首回合手牌 = 6（4 + 2）");
    assert_eq!(v3[0].2, 6, "v3 主公首回合手牌 = 6（4 + 2）");
}

#[test]
fn parity_card_defs_sample() {
    use sanguosha3::kernel::cards::def as v3_def;
    assert_eq!(v3_def("huo_sha").unwrap().tags.sha, true);
    assert_eq!(v3_def("sha").unwrap().tags.sha, true);
}

// ---------------------------------------------------------------------------
// B1 对拍（风林火山·SP-1）：v1 oracle 可比项
//
// 口径（`v3-parity-baseline.md` §5）：同武将/同身份开局，比对「主公首回合手牌数」
// 与「开局计数标记」。其余 B1 技能由 `tests/b1.rs` 按 B1.md 权威描述逐条断言
// （v1 全流程动作脚本待 M6 回合驱动接入，见 baseline §4.3）。
// ---------------------------------------------------------------------------

fn v1_state_with(generals: [&str; 5]) -> SgsState {
    let mut rng = SeededRng::new(7);
    let identities = [
        sanguosha::Identity::Lord,
        sanguosha::Identity::Loyalist,
        sanguosha::Identity::Rebel,
        sanguosha::Identity::Rebel,
        sanguosha::Identity::Spy,
    ];
    let seats: Vec<SandboxSeat> = (0..5)
        .map(|i| SandboxSeat {
            index: Some(i),
            player_id: None,
            general: Some(generals[i].to_string()),
            identity: Some(identities[i]),
        })
        .collect();
    let spec = SandboxSpec { players: 5, seats };
    let (state, _) = new_sandbox_game("r1", "g1", &spec, &mut rng).expect("v1 沙盒开局");
    SgsState::parse(&state.data).expect("解析 v1 状态")
}

/// v3 等价开局：显式**武将**（势力/体力读生成物）+ 技能，主公先手；返回对局（未推进回合）。
fn v3_state_with(generals: [&str; 5], skills: &[&str]) -> Game {
    let mut g = Game {
        seats: (0..5)
            .map(|i| Seat {
                id: i,
                hp: 4,
                max_hp: 4,
                alive: true,
                skills: if i == 0 { skills.iter().map(|s| s.to_string()).collect() } else { vec![] },
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    for i in 0..40 {
        let id = format!("c{i}");
        g.cards.insert(id.clone(), Zone::Draw);
        g.deck.push(id);
    }
    let ids = vec![V3Identity::Loyalist, V3Identity::Rebel, V3Identity::Rebel, V3Identity::Spy];
    begin_game(&mut g, &ids, 4);
    // 武将最小字段（势力/体力/主公 +1）来自生成物 `generals_generated`。
    for (i, code) in generals.iter().enumerate() {
        sanguosha3::kernel::generals::apply_to_seat(&mut g, i, code).expect("武将应存在");
    }
    g
}

#[test]
fn parity_b1_yongsi_draw_bonus_matches_v1() {
    // v1：主公 袁术（群）+ 曹仁（魏）/周泰（吴）/严颜（蜀）/王平（蜀）→ 存活不同势力 = 4。
    let sgs = v1_state_with(["sgs_hero_100", "sgs_hero_29", "sgs_hero_210", "sgs_hero_400", "sgs_hero_401"]);
    let v1_hand = sgs.seats[0].hand.len();
    let v1_lord_hand = 4;
    let v1_draw = 2;
    let v1_x = v1_hand as i32 - v1_lord_hand - v1_draw;
    assert_eq!(v1_x, 4, "v1 庸肆 X = 存活不同势力数 4（v1 oracle）");

    let mut g = v3_state_with(["sgs_hero_100", "sgs_hero_29", "sgs_hero_210", "sgs_hero_400", "sgs_hero_401"], &["yongsi"]);
    assert_eq!(g.kingdom_count(), v1_x as usize, "v3 势力数与 v1 一致");
    let before = g.hand_size(0);
    assert_eq!(before, 4, "主公初始手牌 4 张（与 v1/v2 一致）");
    let drawn = sanguosha3::kernel::flow::draw_phase(&mut g, 0);
    assert_eq!(drawn.len(), v1_draw as usize + v1_x as usize, "庸肆：2 + X");
    assert_eq!(g.hand_size(0), v1_hand, "庸肆：v1 ≡ v3 首回合摸牌后手牌数");
}

#[test]
fn parity_b1_huaiju_game_start_three_ju_matches_v1() {
    // v1：主公 陆绩（吴）→ 开局 3 枚「橘」（计数标记，非牌）。
    let sgs = v1_state_with(["sgs_hero_402", "sgs_hero_29", "sgs_hero_100", "sgs_hero_400", "sgs_hero_401"]);
    let v1_ju = sgs.seats[0].mark_counts.get("ju").copied().unwrap_or(0);
    assert_eq!(v1_ju, 3, "v1 怀橘开局 3 橘");

    let g = v3_state_with(["sgs_hero_402", "sgs_hero_29", "sgs_hero_100", "sgs_hero_400", "sgs_hero_401"], &["huaiju"]);
    assert_eq!(sanguosha3::kernel::skills::b1::ju(&g, 0), v1_ju as i32, "v3 怀橘开局 3 橘");
    // 有橘 → 摸牌阶段 +1（v1 首回合手牌 = 6 + 2 + 1）
    let v1_hand = sgs.seats[0].hand.len();
    assert_eq!(v1_hand, 7, "v1 陆绩首回合手牌 4+2+橘1");
    let mut g2 = v3_state_with(["sgs_hero_402", "sgs_hero_29", "sgs_hero_100", "sgs_hero_400", "sgs_hero_401"], &["huaiju"]);
    sanguosha3::kernel::flow::draw_phase(&mut g2, 0);
    assert_eq!(g2.hand_size(0), v1_hand, "怀橘：v1 ≡ v3 首回合摸牌后手牌数");
}

#[test]
fn parity_b1_cunmu_bottom_draw_keeps_hand_size() {
    // 寸目只改摸牌方向（牌堆底），不改数量：v1 首回合手牌 = 4 初始 + 2 摸。
    let sgs = v1_state_with(["sgs_hero_406", "sgs_hero_29", "sgs_hero_100", "sgs_hero_400", "sgs_hero_401"]);
    assert_eq!(sgs.seats[0].hand.len(), 6, "v1 许攸首回合手牌 6（方向不影响数量）");
    let mut g = v3_state_with(["sgs_hero_406", "sgs_hero_29", "sgs_hero_100", "sgs_hero_400", "sgs_hero_401"], &["cunmu"]);
    assert!(sanguosha3::kernel::skills::draw_from_bottom(&g, 0));
    sanguosha3::kernel::flow::draw_phase(&mut g, 0);
    assert_eq!(g.hand_size(0), 6, "寸目：v1 ≡ v3 首回合手牌数");
}
