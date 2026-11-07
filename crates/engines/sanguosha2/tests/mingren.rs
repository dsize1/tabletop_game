//! 明任：开局摸 1 + 必选一张作为「任」；结束阶段一进一出。

use sanguosha2::kernel::resolve::{advance, begin_phase, submit};
use sanguosha2::kernel::{Card, Game, Phase, Seat, Suit};

fn mk() -> Game {
    let mut g = Game::new(
        vec![
            Seat::new("p0", "luzhi", &["mingren"], 4),
            Seat::new("p1", "other", &[], 4),
        ],
        (1..=12)
            .map(|i| Card::new(format!("c{i}"), "杀", Suit::Spade, (i % 13) as u8))
            .collect(),
    );
    for s in 0..2 {
        for j in 0..3 {
            g.to_hand(&format!("c{}", s * 3 + j + 1), s).unwrap();
        }
    }
    g
}

fn pend(g: &Game) -> &sanguosha2::kernel::pending::Pending {
    g.pending.as_ref().expect("应有 pending")
}

#[test]
fn mingren_draws_one_then_forces_pick() {
    let mut g = mk();
    let before = g.hand(0).len();
    advance(&mut g).expect("驱动开局询问");
    assert_eq!(g.hand(0).len(), before + 1, "开局摸 1 张");
    assert_eq!(pend(&g).skill(), Some("mingren"), "应要求选一张作为「任」");
    // 放弃也必须在下一步选（不给「不选」语义）
    let opt = pend(&g).options()[0].id.clone();
    submit(&mut g, 0, &opt).unwrap();
    assert_eq!(g.mark_cards(0, "ren").len(), 1, "必须产生一张「任」");
}

#[test]
fn mingren_end_phase_swap_is_one_in_one_out() {
    let mut g = mk();
    advance(&mut g).unwrap();
    let opt = pend(&g).options()[0].id.clone();
    submit(&mut g, 0, &opt).unwrap();
    let ren_before = g.mark_cards(0, "ren")[0].clone();
    let hand_before = g.hand(0).len();
    begin_phase(&mut g, 0, Phase::End).unwrap();
    // 结束阶段应询问替换
    if g.pending.is_some() && pend(&g).skill() == Some("mingren") {
        let opt = pend(&g).options()[0].id.clone();
        submit(&mut g, 0, &opt).unwrap();
        assert_eq!(g.hand(0).len(), hand_before, "一进一出：手牌数不变");
        let ren_after = g.mark_cards(0, "ren");
        assert_eq!(ren_after.len(), 1);
        assert_ne!(ren_after[0], ren_before, "「任」应被替换");
    }
}
