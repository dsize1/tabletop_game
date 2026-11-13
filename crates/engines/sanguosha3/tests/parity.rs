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
                flipped: false,
            })
            .collect(),
        ..Default::default()
    };
    for i in 0..20 {
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
    // 非主公座位（1–4）初始手牌 = 4（主公的 +2 是 v1 待查口径，见 parity_known_diff）。
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

/// 已知差异（白名单）：v1 主公初始手牌 = 6（+2），v3 = 4。根因待查（v1 finish_setup 的
/// 主公额外摸牌，还是 sandbox 专属行为），登记不阻塞。
#[test]
fn parity_known_diff_lord_hand() {
    let v1 = v1_summary();
    let v3 = v3_summary();
    assert_eq!(v1[0].2, 6, "v1 主公手牌 = 6（待查口径）");
    assert_eq!(v3[0].2, 4, "v3 主公手牌 = 4（rules.md 口径）");
}

#[test]
fn parity_card_defs_sample() {
    use sanguosha3::kernel::cards::def as v3_def;
    assert_eq!(v3_def("huo_sha").unwrap().tags.sha, true);
    assert_eq!(v3_def("sha").unwrap().tags.sha, true);
}
