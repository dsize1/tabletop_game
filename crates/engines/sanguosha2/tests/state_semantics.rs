//! C2 状态语义（I2）：存活/死亡是显式状态，不由 `hp` 推导。
//!
//! 规则（用户 2026-11-12 裁定）：
//! - **开局按武将表满血**（`max_hp`，主公 +1）；
//! - **未选武将不展示血量**（视图 `hp/max_hp/alive` 均为 null）；
//! - **身份只在死亡确认结算时公开**（`flow::die` 置 `Seat.alive = false`）；
//! - 濒死（`hp = 0`、等待救援）**仍是存活**：不得公开身份、不得判胜负。

use game_engine_core::SeededRng;
use sanguosha2::cards;
use sanguosha2::identity;
use sanguosha2::kernel::{flow, Game, Phase, Seat};
use sanguosha2::view;

fn seat(name: &str, hp: i32) -> Seat {
    Seat::new(name, "", &[], hp)
}

fn game() -> Game {
    Game::new(
        vec![seat("p0", 0), seat("p1", 0), seat("p2", 0)],
        cards::deck(),
    )
}

/// 未选武将：视图不展示体力与存活（hp/max_hp/alive 全为 null）。
#[test]
fn unpicked_seat_hides_hp_and_alive() {
    let mut g = game();
    g.seats[1].identity = "rebel".into();
    let v = view::public_state(&g);
    let s = &v["public_board"]["seats"][1];
    assert!(s["hp"].is_null(), "未选将不展示体力：{}", s["hp"]);
    assert!(s["max_hp"].is_null());
    assert!(s["alive"].is_null(), "未选将不展示存活");
    assert!(s["identity"].is_null(), "未选将不得公开身份");
}

/// 已选武将：正常展示体力与存活。
#[test]
fn picked_seat_shows_hp_and_alive() {
    let mut g = game();
    g.seats[1].general = "sgs_hero_1".into();
    g.seats[1].hp = 4;
    g.seats[1].max_hp = 4;
    let v = view::public_state(&g);
    let s = &v["public_board"]["seats"][1];
    assert_eq!(s["hp"], 4);
    assert_eq!(s["max_hp"], 4);
    assert_eq!(s["alive"], true);
}

/// 濒死（hp = 0 但未死亡）：存活仍为 true，身份**不公开**。
#[test]
fn dying_seat_is_alive_and_identity_hidden() {
    let mut g = game();
    for i in 0..3 {
        g.seats[i].general = "sgs_hero_1".into();
        g.seats[i].hp = 4;
        g.seats[i].max_hp = 4;
    }
    g.seats[1].identity = "rebel".into();
    g.seats[1].hp = 0; // 濒死：尚未走死亡确认
    let v = view::public_state(&g);
    let s = &v["public_board"]["seats"][1];
    assert_eq!(s["alive"], true, "濒死仍是存活");
    assert!(
        s["identity"].is_null(),
        "濒死不得公开身份（只有死亡确认才公开）"
    );
}

/// 死亡确认（`flow::die`）：alive=false 且身份公开。
#[test]
fn dead_seat_identity_becomes_public() {
    let mut g = game();
    for i in 0..3 {
        g.seats[i].general = "sgs_hero_1".into();
        g.seats[i].hp = 4;
        g.seats[i].max_hp = 4;
    }
    g.seats[1].identity = "rebel".into();
    flow::die(&mut g, 1, None);
    let v = view::public_state(&g);
    let s = &v["public_board"]["seats"][1];
    assert_eq!(s["alive"], false);
    assert_eq!(s["identity"], "rebel", "死亡确认后身份公开");
}

/// 身份局胜负：主公**濒死**不判负，只有死亡确认才判负。
#[test]
fn lord_dying_does_not_end_game_but_death_does() {
    let mut g = game();
    g.identity_game = true;
    g.seats[0].identity = identity::LORD.into();
    g.seats[0].flags.insert("lord".into());
    g.seats[1].identity = identity::REBEL.into();
    g.seats[2].identity = identity::REBEL.into();
    g.seats[0].hp = 0; // 濒死
    identity::check_finished(&mut g);
    assert!(!g.finished, "濒死不判负");
    flow::die(&mut g, 0, Some(1));
    identity::check_finished(&mut g);
    assert!(g.finished, "死亡确认后判负");
    assert_eq!(g.winner.as_deref(), Some(identity::REBEL));
}

/// 开局按武将表满血（沙盒开局：hp == max_hp；主公 +1）。
#[test]
fn sandbox_game_starts_at_full_hp() {
    let mut rng = SeededRng::new(20261112);
    let spec = sanguosha2::sandbox::SandboxSpec {
        players: 5,
        seats: Vec::new(),
    };
    let (state, _events) =
        sanguosha2::sandbox::new_sandbox_game("probe", "probe", &spec, &mut rng).expect("沙盒开局");
    let g: Game = serde_json::from_value(state.data).expect("解析状态");
    let lord = g
        .seats
        .iter()
        .position(|s| s.identity == identity::LORD)
        .expect("应有主公");
    for (i, s) in g.seats.iter().enumerate() {
        assert!(!s.general.is_empty(), "开局应已选将");
        assert_eq!(s.hp, s.max_hp, "开局满血（座位 {i}）: {}/{}", s.hp, s.max_hp);
        assert!(s.alive, "开局存活");
        let def = sanguosha2::generals::by_code(&s.general).expect("武将定义");
        let bonus = if i == lord { 1 } else { 0 };
        assert_eq!(
            s.max_hp,
            def.max_hp + bonus,
            "体力上限按武将表{}",
            if bonus == 1 { "（主公 +1）" } else { "" }
        );
        assert!(s.hp > 0, "开局不得空血");
    }
    // 视图：开局所有座位都展示体力/存活（已选将）
    let v = view::public_state(&g);
    for s in v["public_board"]["seats"].as_array().unwrap() {
        assert!(!s["hp"].is_null(), "开局应展示体力");
        assert_eq!(s["alive"], true);
    }
    // `Phase` 引用避免未使用告警（该用例只关心状态语义）。
    let _ = Phase::Play;
}
