//! P4.4.2 界限突破批次 J4（8 将）：界孟获 / 界徐晃 / 界蔡文姬 / 界荀彧 / 界典韦 / 界颜良文丑 / 界张昭张纮 / 界徐盛。

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{clear_hands, edit, manager, state_of, take_card};
use crate::skills;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

fn advance_once(st: &mut SgsState) {
    let mut ev = Vec::new();
    crate::resolve::advance(st, &mut ev).expect("advance");
}

// ---------------------------------------------------------------------------
// 界孟获 · 祸首 / 再起
// ---------------------------------------------------------------------------

#[test]
fn zaizhi_jie_end_phase_red_cards_draw() {
    let mgr = manager(3500);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_492");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        st.set_counter("red_discarded", 1);
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("zaizhi_jie"),
        "界再起：结束阶段按红牌数结算"
    );
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:draw")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history.iter().any(|e| e.text.contains("再起：摸")),
        "再起：令目标摸一张"
    );
}

#[test]
fn zaizhi_jie_heal_owner() {
    let mgr = manager(3501);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_492");
        st.seats[seat].hp = st.seats[seat].max_hp - 1;
        hp = st.seats[seat].hp;
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        st.set_counter("red_discarded", 1);
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &target_pid, "choose:heal")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp + 1, "再起：令孟获回复 1 点体力");
}

#[test]
fn zaizhi_jie_no_red_no_trigger() {
    let mgr = manager(3502);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_492");
        st.turn_counters.clear();
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert_ne!(
        pending_skill(&st),
        Some("zaizhi_jie"),
        "无红牌弃置则不触发再起"
    );
}

// ---------------------------------------------------------------------------
// 界徐晃 · 断粮 / 截辎
// ---------------------------------------------------------------------------

#[test]
fn duanliang_jie_no_distance_when_no_damage() {
    let mgr = manager(3503);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut card, mut far_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_496");
        stage_play(st, seat);
        st.set_counter("damage_dealt", 0);
        card = give_filtered(st, seat, |c| {
            skills::is_black(c.suit)
                && crate::data::card_def(&c.def)
                    .is_some_and(|d| d.category == Category::Basic || d.equip_slot.is_some())
        });
        let far = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) >= 2)
            .expect("远处目标");
        set_general(st, far, "sgs_hero_1");
        far_pid = pid_of(st, far);
    });
    mgr.execute_action(
        &room,
        &pid_of(&state_of(&mgr, &room), seat),
        &format!("skill:duanliang:{card}"),
    )
    .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        pending_options_with(&st, "target:").contains(&format!("target:{far_pid}")),
        "界断粮：本回合未造成伤害 → 无距离限制"
    );
}

#[test]
fn duanliang_jie_distance_limited_after_damage() {
    let mgr = manager(3504);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut card, mut far_pid) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_496");
        stage_play(st, seat);
        st.set_counter("damage_dealt", 1);
        card = give_filtered(st, seat, |c| {
            skills::is_black(c.suit)
                && crate::data::card_def(&c.def)
                    .is_some_and(|d| d.category == Category::Basic || d.equip_slot.is_some())
        });
        let far = other_seats(st, seat)
            .into_iter()
            .find(|t| st.seat_distance(seat, *t) >= 2)
            .expect("远处目标");
        set_general(st, far, "sgs_hero_1");
        far_pid = pid_of(st, far);
        // 给远处目标较少手牌，排除「手牌数≥你」分支。
        st.seats[far].hand.clear();
    });
    let pid = pid_of(&state_of(&mgr, &room), seat);
    mgr.execute_action(&room, &pid, &format!("skill:duanliang:{card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        !pending_options_with(&st, "target:").contains(&format!("target:{far_pid}")),
        "界断粮：已造成伤害 → 恢复距离限制"
    );
}

#[test]
fn jiezi_jie_marks_lowest_hand_character() {
    let mgr = manager(3505);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut low, mut low_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_496");
        pid = pid_of(st, seat);
        let others = other_seats(st, seat);
        low = others[0];
        set_general(st, low, "sgs_hero_1");
        low_pid = pid_of(st, low);
        for t in &others[1..] {
            give_def(st, *t, "sha");
            give_def(st, *t, "sha");
        }
        crate::skills::jiezi_on_skip_draw(st, others[1]);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("jiezi"),
        "界截辎：跳过摸牌阶段后询问"
    );
    mgr.execute_action(&room, &pid, &format!("target:{low_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[low].mark_counts.get("zai").copied().unwrap_or(0),
        1,
        "截辎：手牌最少者获得「辎」"
    );
}

#[test]
fn jiezi_jie_extra_draw_phase() {
    let mgr = manager(3506);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut before) = (0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_1");
        *st.seats[seat].mark_counts.entry("zai".into()).or_insert(0) += 1;
        before = st.seats[seat].hand.len();
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].hand.len() >= before + 4,
        "截辎：额外摸牌阶段（摸 2 + 正常摸 2）"
    );
    assert_eq!(
        st.seats[seat].mark_counts.get("zai").copied().unwrap_or(0),
        0,
        "截辎：进入摸牌阶段后移除「辎」"
    );
}

// ---------------------------------------------------------------------------
// 界蔡文姬 · 断肠 / 悲歌
// ---------------------------------------------------------------------------

#[test]
fn beige_jie_gains_judge_card_on_same_suit() {
    let mgr = manager(3507);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut victim, mut source, mut heart, mut hp) =
        (0, String::new(), 0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        owner = other_seats(st, source)[0];
        set_general(st, owner, "sgs_hero_505");
        owner_pid = pid_of(st, owner);
        victim = other_seats(st, source)[1];
        set_general(st, victim, "sgs_hero_1");
        heart = give_filtered(st, owner, |c| c.suit == Suit::Heart);
        put_on_top(st, |c| c.suit == Suit::Heart && c.def != "sha");
        hp = st.seats[victim].hp;
        let sha = find_free_card(st, |c| c.def == "sha");
        damage_and_advance(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("beige"));
    mgr.execute_action(&room, &owner_pid, &format!("choose:{heart}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("花色相同，获得判定牌")),
        "界悲歌：花色相同获得判定牌"
    );
    let _ = (victim, hp);
}

#[test]
fn beige_jie_heals_on_heart() {
    let mgr = manager(3508);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut victim, mut source, mut disc, mut hp) =
        (0, String::new(), 0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        owner = other_seats(st, source)[0];
        set_general(st, owner, "sgs_hero_505");
        owner_pid = pid_of(st, owner);
        victim = other_seats(st, source)[1];
        set_general(st, victim, "sgs_hero_1");
        st.seats[victim].hp = st.seats[victim].max_hp - 1;
        disc = give_filtered(st, owner, |c| c.suit == Suit::Spade);
        put_on_top(st, |c| c.suit == Suit::Heart && c.def != "sha");
        let sha = find_free_card(st, |c| c.def == "sha");
        damage_and_advance(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
        hp = st.seats[victim].hp;
    });
    mgr.execute_action(&room, &owner_pid, &format!("choose:{disc}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[victim].hp,
        hp + 1,
        "悲歌：红桃令受伤者回复 1 点体力"
    );
}

#[test]
fn beige_jie_gains_discarded_on_same_rank() {
    let mgr = manager(3509);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut victim, mut source, mut disc, mut rank) =
        (0, String::new(), 0, 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        owner = other_seats(st, source)[0];
        set_general(st, owner, "sgs_hero_505");
        owner_pid = pid_of(st, owner);
        victim = other_seats(st, source)[1];
        set_general(st, victim, "sgs_hero_1");
        // 弃置牌：黑桃 7；判定牌：红桃 7（花色不同、点数相同）。
        disc = give_filtered(st, owner, |c| c.suit == Suit::Spade && c.rank == 7);
        rank = 7;
        put_on_top(st, |c| {
            c.suit == Suit::Heart && c.rank == rank && c.def != "sha"
        });
        let sha = find_free_card(st, |c| c.def == "sha");
        damage_and_advance(
            st,
            victim,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&sha),
        );
    });
    mgr.execute_action(&room, &owner_pid, &format!("choose:{disc}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("点数相同，获得弃置牌")),
        "界悲歌：点数相同获得弃置的牌"
    );
    assert!(st.seats[owner].hand.iter().any(|c| c == &disc));
}

// ---------------------------------------------------------------------------
// 界荀彧 · 驱虎 / 节命
// ---------------------------------------------------------------------------

#[test]
fn jieming_jie_draws_then_discards_to_x() {
    let mgr = manager(3510);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_515");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        give_def(st, target, "sha");
        st.set_counter("jieming_offered", 0);
        st.seats[seat].hp = 2;
        damage_and_advance(
            st,
            seat,
            1,
            Some(target),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jieming"));
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let x = (st.seats[target].max_hp.max(0) as usize).min(5);
    // 摸 X 后弃至 X：最终手牌 = X（或更少，若弃牌 pending 尚未处理）。
    while let Some(p) = st.pending.clone() {
        if p.kind != "discard" {
            break;
        }
        let action = p.options.first().map(|o| o.id.clone()).expect("弃牌选项");
        mgr.execute_action(&room, &p.actor, &action).unwrap();
        break;
    }
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() <= x,
        "界节命：摸 X 后弃至 X（X={x}）"
    );
}

#[test]
fn jieming_jie_draws_x_when_hand_full() {
    let mgr = manager(3511);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut before) =
        (0, String::new(), 0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_515");
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        for _ in 0..5 {
            give_def(st, target, "sha");
        }
        before = st.seats[target].hand.len();
        st.seats[seat].hp = 2;
        damage_and_advance(
            st,
            seat,
            1,
            Some(target),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target].hand.len() > before,
        "界节命：手牌已多也要先摸 X 张"
    );
}

#[test]
fn quhu_jie_available_and_targets_valid() {
    let mgr = manager(3512);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_515");
        stage_play(st, seat);
        st.seats[seat].hp = 2;
        pid = pid_of(st, seat);
        give_def(st, seat, "sha");
        for t in other_seats(st, seat) {
            set_general(st, t, "sgs_hero_1");
            give_def(st, t, "sha");
        }
    });
    let st = state_of(&mgr, &room);
    assert!(
        crate::turn::legal_actions(&st, &pid)
            .iter()
            .any(|a| a.id == "skill:quhu"),
        "驱虎：体力值大于你的角色存在时可发动"
    );
}

// ---------------------------------------------------------------------------
// 界典韦 · 强袭 / 狞恶
// ---------------------------------------------------------------------------

#[test]
fn qiangxi_jie_costs_damage_instead_of_hp_loss() {
    let mgr = manager(3513);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut hp) = (0, String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_526");
        stage_play(st, seat);
        hp = st.seats[seat].hp;
        pid = pid_of(st, seat);
        set_general(st, other_seats(st, seat)[0], "sgs_hero_1");
    });
    mgr.execute_action(&room, &pid, "skill:qiangxi").unwrap();
    mgr.execute_action(&room, &pid, "choose:qiangxi:hp")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[seat].hp, hp - 1, "界强袭：代价为受到 1 点伤害");
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("强袭：代价已支付")),
        "强袭：代价记录"
    );
}

#[test]
fn ninge_jie_draws_on_second_damage() {
    let mgr = manager(3514);
    let room = start_and_pick(&mgr, 5);
    let (mut dianwei, mut victim, mut before) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dianwei = st.turn;
        set_general(st, dianwei, "sgs_hero_526");
        victim = other_seats(st, dianwei)[0];
        set_general(st, victim, "sgs_hero_1");
        give_def(st, victim, "sha");
        before = st.seats[dianwei].hand.len();
        damage_and_advance(
            st,
            victim,
            1,
            Some(dianwei),
            crate::resolve::DamageType::Normal,
            None,
        );
        damage_and_advance(
            st,
            victim,
            1,
            Some(dianwei),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[dianwei].hand.len() > before,
        "狞恶：目标第二次受伤，典韦摸 1"
    );
    assert!(
        st.history.iter().any(|e| e.text.contains("狞恶：")),
        "狞恶：弃置其一张牌"
    );
}

#[test]
fn ninge_jie_not_on_first_damage() {
    let mgr = manager(3515);
    let room = start_and_pick(&mgr, 5);
    let (mut dianwei, mut victim, mut before) = (0, 0, 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        dianwei = st.turn;
        set_general(st, dianwei, "sgs_hero_526");
        victim = other_seats(st, dianwei)[0];
        set_general(st, victim, "sgs_hero_1");
        before = st.seats[dianwei].hand.len();
        damage_and_advance(
            st,
            victim,
            1,
            Some(dianwei),
            crate::resolve::DamageType::Normal,
            None,
        );
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[dianwei].hand.len(), before, "狞恶：首次受伤不触发");
}

// ---------------------------------------------------------------------------
// 界颜良文丑 · 双雄
// ---------------------------------------------------------------------------

#[test]
fn shuangxiong_jie_discard_sets_color_and_converts() {
    let mgr = manager(3516);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut red, mut black) = (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_550");
        pid = pid_of(st, seat);
        red = give_filtered(st, seat, |c| skills::is_red(c.suit));
        black = give_filtered(st, seat, |c| skills::is_black(c.suit));
        set_general(st, other_seats(st, seat)[0], "sgs_hero_1");
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("shuangxiong_jie"),
        "界双雄：摸牌结束可弃牌"
    );
    mgr.execute_action(&room, &pid, &format!("choose:{red}"))
        .unwrap();
    mgr.execute_action(&room, &pid, "confirm").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("shuangxiong_color"), 1);
    // 异色（黑）牌可当【决斗】。
    assert!(
        crate::resolve_fire::shuangxiong_conversion(&st, seat, &black).is_some(),
        "双雄：异色牌当【决斗】"
    );
    assert!(
        crate::resolve_fire::shuangxiong_conversion(&st, seat, &red).is_none(),
        "双雄：同色牌不可转化"
    );
}

#[test]
fn shuangxiong_jie_gains_damage_cards_at_end() {
    let mgr = manager(3517);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut source, mut card) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        source = st.turn;
        set_general(st, source, "sgs_hero_1");
        seat = other_seats(st, source)[0];
        set_general(st, seat, "sgs_hero_550");
        card = find_free_card(st, |c| c.def == "sha");
        take_card(st, &card);
        st.deck.discard.push(card.clone());
        damage_and_advance(
            st,
            seat,
            1,
            Some(source),
            crate::resolve::DamageType::Normal,
            Some(&card),
        );
        st.turn = seat;
    });
    run_phase(&mgr, &room, seat, TurnPhase::End, false);
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].hand.iter().any(|c| c == &card),
        "界双雄：结束阶段获得本回合对你造成伤害的牌"
    );
}

#[test]
fn shuangxiong_jie_no_discard_no_conversion() {
    let mgr = manager(3518);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut black) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_550");
        pid = pid_of(st, seat);
        black = give_filtered(st, seat, |c| skills::is_black(c.suit));
        give_filtered(st, seat, |c| skills::is_red(c.suit));
    });
    run_phase(&mgr, &room, seat, TurnPhase::Draw, true);
    mgr.execute_action(&room, &pid, "cancel").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.counter("shuangxiong_color"), 0);
    assert!(
        crate::resolve_fire::shuangxiong_conversion(&st, seat, &black).is_none(),
        "双雄：未弃牌则不转化"
    );
}

// ---------------------------------------------------------------------------
// 界张昭张纮 · 直谏 / 固政
// ---------------------------------------------------------------------------

#[test]
fn guzheng_jie_after_two_discards() {
    let mgr = manager(3519);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut owner_pid, mut discarder, mut c1, mut c2) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_563");
        owner_pid = pid_of(st, owner);
        discarder = other_seats(st, owner)[0];
        set_general(st, discarder, "sgs_hero_1");
        c1 = find_free_card(st, |c| c.def == "sha");
        take_card(st, &c1);
        st.deck.discard.push(c1.clone());
        st.bump_counter(&format!("discarded_by:{discarder}"));
        st.seats[discarder]
            .flags
            .insert(format!("gz_jie:{discarder}:{c1}"), true);
        c2 = find_free_card(st, |c| c.def == "tao");
        take_card(st, &c2);
        crate::turn::discard_by(st, Some(discarder), &c2);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("guzheng_jie"),
        "界固政：两张弃牌后触发"
    );
    mgr.execute_action(&room, &owner_pid, "choose:guzheng")
        .unwrap();
    let st = state_of(&mgr, &room);
    let got_discarder = st.seats[discarder]
        .hand
        .iter()
        .filter(|c| **c == c1 || **c == c2)
        .count();
    let got_owner = st.seats[owner]
        .hand
        .iter()
        .filter(|c| **c == c1 || **c == c2)
        .count();
    assert_eq!(got_discarder, 1, "固政：弃置者获得一张");
    assert_eq!(got_owner, 1, "固政：张昭张纮获得其余");
}

#[test]
fn guzheng_jie_not_on_single_discard() {
    let mgr = manager(3520);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut discarder, mut c1) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        owner = st.turn;
        set_general(st, owner, "sgs_hero_563");
        discarder = other_seats(st, owner)[0];
        set_general(st, discarder, "sgs_hero_1");
        c1 = find_free_card(st, |c| c.def == "sha");
        take_card(st, &c1);
        crate::turn::discard_by(st, Some(discarder), &c1);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert_ne!(
        pending_skill(&st),
        Some("guzheng_jie"),
        "单张弃牌不触发固政"
    );
}

#[test]
fn zhijian_jie_places_equip_and_draws() {
    let mgr = manager(3521);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut equip) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_563");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        target_pid = pid_of(st, target);
        equip = give_equip(st, seat);
    });
    mgr.execute_action(&room, &pid, &format!("skill:zhijian:{equip}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[target]
            .equips
            .values()
            .any(|c| c.as_deref() == Some(&equip)),
        "直谏：装备置入目标装备区"
    );
    assert!(st.seats[seat].hand.len() >= 1, "直谏：置入后摸一张");
}

// ---------------------------------------------------------------------------
// 界徐盛 · 破军
// ---------------------------------------------------------------------------

#[test]
fn pojun_jie_damage_bonus() {
    let mgr = manager(3522);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut hp, mut sha) =
        (0, String::new(), 0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_587");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        st.seats[target].hand.clear();
        target_pid = pid_of(st, target);
        hp = st.seats[target].hp;
        sha = give_def(st, seat, "sha");
        give_def(st, seat, "tao");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[target].hp,
        hp - 2,
        "界破军：目标手牌/装备不多于你 → 伤害 +1"
    );
}

#[test]
fn pojun_jie_no_bonus_when_target_has_more_cards() {
    let mgr = manager(3523);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut target, mut bonus) = (0, 0, -1);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_587");
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        for _ in 0..3 {
            give_def(st, target, "tao");
        }
        bonus = crate::resolve_jiexian5::pojun_damage_bonus(st, seat, target);
    });
    assert_eq!(bonus, 0, "界破军：目标手牌更多 → 无伤害加成");
}

#[test]
fn pojun_jie_still_moves_cards() {
    let mgr = manager(3524);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut target, mut target_pid, mut sha) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_587");
        stage_play(st, seat);
        pid = pid_of(st, seat);
        target = other_seats(st, seat)[0];
        set_general(st, target, "sgs_hero_1");
        give_def(st, target, "tao");
        target_pid = pid_of(st, target);
        sha = give_def(st, seat, "sha");
    });
    mgr.execute_action(&room, &pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("pojun"), "破军：可移出目标牌");
    let option = pending_options_with(&st, "choose:pojun:")
        .into_iter()
        .next()
        .expect("移出选项");
    mgr.execute_action(&room, &pid, &option).unwrap();
    let st = state_of(&mgr, &room);
    assert!(!st.deck.void.is_empty(), "破军：牌被移出游戏");
}
