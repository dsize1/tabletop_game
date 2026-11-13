//! P4.4.4 全武将验证 · 真机缺陷回归（M66 复验轮）。
//!
//! 覆盖：
//! - 固政/界固政：交给弃牌者的牌由持有者选择（非固定首张）
//! - 解围②：被放逐从背面翻回正面时触发（弃 1 张 + 移动场上一张牌）
//! - 判定公开区：判定结算后不再在出牌阶段投影（视图层）
//! - 弃牌数可解释：手牌/上限/原因下发（不屈手牌上限 = 创数），且不被场上袁术影响
//! - 直谏：一阶段选装备后进入二阶段选目标，重复提交一阶段动作应被拒

use super::p24::{set_general, start_and_pick};
use super::support::*;
use super::{
    clear_hands, drive_phases, drive_state_inject, edit, give, manager, public_view, state_of,
    take_card,
};
use game_engine_core::GameManager;
use crate::state::*;

fn pid_of(st: &SgsState, seat: usize) -> String {
    st.seats[seat].player_id.clone()
}

/// 取 n 张「不在任何区域（手牌/装备/判定/标记）」的牌，并从牌堆/弃牌堆移除。
fn take_free_ids(st: &mut SgsState, pred: impl Fn(&CardInst) -> bool, n: usize) -> Vec<String> {
    let ids: Vec<String> = st
        .cards
        .iter()
        .filter(|(id, c)| {
            pred(c)
                && !st.seats.iter().any(|s| {
                    s.hand.iter().any(|h| h == *id)
                        || s.judge_zone.iter().any(|h| h == *id)
                        || s.equips.values().flatten().any(|h| h == *id)
                        || s.marks.values().flatten().any(|h| h == *id)
                })
        })
        .map(|(id, _)| id.clone())
        .take(n)
        .collect();
    for id in &ids {
        take_card(st, id);
    }
    ids
}

fn advance_once(st: &mut SgsState) {
    let mut ev = Vec::new();
    crate::resolve::advance(st, &mut ev).expect("advance");
}

// ---------------------------------------------------------------------------
// 固政：交给弃牌者的牌由持有者选择
// ---------------------------------------------------------------------------

#[test]
fn guzheng_pick_second_card_goes_to_discarder() {
    let mgr = manager(3700);
    let room = start_and_pick(&mgr, 5);
    let (mut owner, mut discarder, mut cards) = (0, 0, Vec::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        discarder = st.turn;
        owner = st.seats_from_left_of(discarder)[0];
        set_general(st, owner, "sgs_hero_55");
        set_general(st, discarder, "sgs_hero_2");
        st.seats[discarder].hp = 2;
        for _ in 0..5 {
            cards.push(give(st, discarder, "sha"));
        }
        let pid_d = pid_of(st, discarder);
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        drive_state_inject(
            st,
            &mut vec![
                format!(
                    "{pid_d}|discard=discard:{},{},{}",
                    cards[0], cards[1], cards[2]
                ),
                format!("{}|choice=choose:guzheng", pid_of(st, owner)),
                // 选第二张（非首张）：旧实现固定取首张。
                format!("{}|choice=give:{}", pid_of(st, owner), cards[1]),
            ],
        );
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[discarder].hand.contains(&cards[1]),
        "固政：持有者选择的第二张应交给弃牌者"
    );
    assert!(
        !st.seats[discarder].hand.contains(&cards[0]),
        "固政：未被选择的牌不交给弃牌者"
    );
    assert!(
        st.seats[owner].hand.contains(&cards[0]) && st.seats[owner].hand.contains(&cards[2]),
        "固政：其余弃牌归持有者"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 解围②：被放逐翻回正面时触发
// ---------------------------------------------------------------------------

#[test]
fn jiewei_triggers_when_fangzhu_flips_face_up() {
    let mgr = manager(3701);
    let room = start_and_pick(&mgr, 5);
    let (mut caopi, mut caoren, mut caoren_pid, mut caopi_pid) =
        (0, 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caopi = st.turn;
        set_general(st, caopi, "sgs_hero_43"); // 曹丕：放逐
        caopi_pid = pid_of(st, caopi);
        caoren = other_seats(st, caopi)[0];
        set_general(st, caoren, "sgs_hero_29"); // 曹仁：解围
        caoren_pid = pid_of(st, caoren);
        // 曹仁背面朝上 + 有可弃的牌（解围发动前提）。
        st.seats[caoren].flipped = true;
        give(st, caoren, "sha");
        give(st, caoren, "shan");
        // 曹丕受伤 → 放逐。
        damage_and_advance(st, caopi, 1, Some(caoren), crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("fangzhu"), "曹丕受伤后应询问放逐");
    mgr.execute_action(&room, &caopi_pid, &format!("target:{caoren_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(!st.seats[caoren].flipped, "放逐把曹仁翻回正面");
    assert_eq!(
        pending_skill(&st),
        Some("jiewei"),
        "翻回正面应触发解围（弃 1 张 + 移动场上 1 张）"
    );
    assert!(
        pending_options_with(&st, "choose:jiewei").len() == 1,
        "解围询问选项应为 choose:jiewei"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 判定公开区：结算后不再投影到出牌阶段
// ---------------------------------------------------------------------------

#[test]
fn judge_zone_hidden_after_judge_resolved() {
    let mgr = manager(3702);
    let room = start_and_pick(&mgr, 5);
    let mut cur = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        give(st, cur, "sha");
        // 红桃乐不思蜀：判定生效但非红桃才跳过 → 用红桃保证进入出牌阶段。
        let heart = find_free_card(st, |c| c.def == "lebu_sishu" && c.suit == Suit::Heart);
        take_card(st, &heart);
        st.seats[cur].judge_zone.push(heart);
        st.turn_phase = TurnPhase::Judge;
        st.turn_counters.clear();
        // 推进到出牌阶段（无人持鬼才/鬼道时直接结算；阶段切换需要 phase_step）。
        for _ in 0..30 {
            if st.turn_phase == TurnPhase::Play {
                break;
            }
            if st.pending.is_some() {
                // 判定前的无懈窗口等：自动放弃。
                drive_phases(st, &mut Vec::new());
                continue;
            }
            let mut ev = Vec::new();
            crate::turn::phase_step(st, &mut ev).expect("phase_step");
            crate::resolve::advance(st, &mut ev).expect("advance");
        }
    });
    let st = state_of(&mgr, &room);
    assert_eq!(st.turn_phase, TurnPhase::Play, "红桃乐不思蜀不生效，应进入出牌阶段");
    let view = public_view(&mgr, &room);
    assert!(
        view["special_zones"].get("judge").is_none(),
        "出牌阶段不应再投影判定公开区（真机：残留兵粮寸断）：{view:?}"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 弃牌数可解释：不屈手牌上限 = 创数；场上袁术不影响周泰
// ---------------------------------------------------------------------------

fn discard_pending_for(mgr: &GameManager, room: &str, seat: usize, with_yuanshu: bool) -> serde_json::Value {
    let mut pending = serde_json::Value::Null;
    edit(mgr, room, |st| {
        clear_hands(st);
        st.turn = seat;
        set_general(st, seat, "sgs_hero_210"); // 周泰
        let others = other_seats(st, seat);
        for (i, s) in others.iter().enumerate() {
            // 一位邻座为袁术（庸肆/伪帝），其余为同势力，确保不再是“只有周泰”的场面。
            if with_yuanshu && i == 0 {
                set_general(st, *s, "sgs_hero_100");
            } else {
                set_general(st, *s, "sgs_hero_1");
            }
        }
        // 复用同一房间时先归还上一次的「创」，避免牌重复计数。
        if let Some(old) = st.seats[seat].marks.remove("buqu") {
            for id in old {
                st.deck.discard.push(id);
            }
        }
        // 先给 5 张手牌。
        for _ in 0..5 {
            give_filtered(st, seat, |c| c.def == "shan");
        }
        // 两张「创」：手牌上限 = 2（取不在任何区域的牌，避免与手牌/标记重复）。
        let chuang: Vec<String> = take_free_ids(st, |c| c.def == "sha", 2);
        st.seats[seat].marks.insert("buqu".into(), chuang);
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
        pending = serde_json::to_value(st.pending.clone()).unwrap_or(serde_json::Value::Null);
    });
    pending
}

#[test]
fn discard_pending_explains_limit_and_ignores_yuanshu() {
    let mgr = manager(3703);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        seat = st.turn;
    });
    let alone = discard_pending_for(&mgr, &room, seat, false);
    let with_yuanshu = discard_pending_for(&mgr, &room, seat, true);
    for (label, p) in [("无袁术", &alone), ("有袁术", &with_yuanshu)] {
        assert_eq!(p["skill"], serde_json::Value::Null, "{label}：普通弃牌 pending");
        assert_eq!(p["data"]["reason"], "discard_phase", "{label}：原因=手牌上限");
        assert_eq!(p["data"]["required"], 3, "{label}：手牌 5 - 上限 2 = 弃 3");
        assert_eq!(p["data"]["hand"], 5, "{label}：下交手牌数");
        assert_eq!(p["data"]["limit"], 2, "{label}：不屈上限 = 创数");
    }
    assert_eq!(
        alone["data"]["required"], with_yuanshu["data"]["required"],
        "场上袁术（庸肆只影响自己）不应改变周泰的弃牌数"
    );
}

// ---------------------------------------------------------------------------
// 直谏：二阶段等目标选择；一阶段动作不可重复提交
// ---------------------------------------------------------------------------

#[test]
fn zhijian_stage2_waits_target_and_rejects_replay() {
    let mgr = manager(3704);
    let room = start_and_pick(&mgr, 5);
    let (mut actor, mut actor_pid, mut target_pid, mut equip) = (0, String::new(), String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        actor = st.turn;
        set_general(st, actor, "sgs_hero_55"); // 张昭张弘：直谏
        actor_pid = pid_of(st, actor);
        target_pid = pid_of(st, other_seats(st, actor)[0]);
        equip = give_filtered(st, actor, |c| c.def == "zhuge_liannu");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        advance_once(st);
    });
    mgr.execute_action(&room, &actor_pid, &format!("skill:zhijian:{equip}"))
        .expect("直谏一阶段：选装备");
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("zhijian"), "直谏应进入二阶段选目标");
    assert!(
        !pending_options_with(&st, "target:").is_empty(),
        "二阶段应给出目标选项"
    );
    // 重复提交一阶段动作（旧客户端在浮层未刷新时会这么做）必须被拒，而不是静默成功。
    let replay = mgr.execute_action(&room, &actor_pid, &format!("skill:zhijian:{equip}"));
    assert!(replay.is_err(), "二阶段重复提交一阶段动作应报错");
    mgr.execute_action(&room, &actor_pid, &format!("target:{target_pid}"))
        .expect("直谏二阶段：选目标");
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), None, "直谏结算完毕");
    assert!(
        st.seats[actor].hand.len() >= 1,
        "直谏完成后摸 1 张（手牌非空）"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 弃牌：手牌已达上限不再询问；hand_cap 只按持有上限（手牌上限只在弃牌阶段生效）
// ---------------------------------------------------------------------------

#[test]
fn discard_not_asked_when_hand_equals_limit() {
    let mgr = manager(3705);
    let room = start_and_pick(&mgr, 5);
    let mut seat = 0;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_210"); // 周泰
        if let Some(old) = st.seats[seat].marks.remove("buqu") {
            for id in old {
                st.deck.discard.push(id);
            }
        }
        // 2 创 → 手牌上限 2；手牌正好 2 张 → 不应要求弃牌。
        let chuang: Vec<String> = take_free_ids(st, |c| c.def == "sha", 2);
        st.seats[seat].marks.insert("buqu".into(), chuang);
        for _ in 0..2 {
            give_filtered(st, seat, |c| c.def == "shan");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| p.kind != "discard"),
        "手牌 2 = 上限 2（不屈 2 创）时不应再要求弃牌：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.data.clone()))
    );
}

/// 周泰 2「创」场景：手牌上限 = 创数 = 2；持有上限单独由 `config.max_hand_cards` 决定。
fn buqu_seat_with_six_cards(mgr: &GameManager, room: &str) -> usize {
    let mut seat = 0;
    edit(mgr, room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_210");
        if let Some(old) = st.seats[seat].marks.remove("buqu") {
            for id in old {
                st.deck.discard.push(id);
            }
        }
        let chuang: Vec<String> = take_free_ids(st, |c| c.def == "sha", 2);
        st.seats[seat].marks.insert("buqu".into(), chuang);
        for _ in 0..6 {
            give_filtered(st, seat, |c| c.def == "shan");
        }
    });
    seat
}

/// 真机反馈（严颜·拒战阴获得 1 张即被要求弃牌）：**手牌上限只在弃牌阶段生效**，
/// 回合内获得牌只要不超过持有上限（`config.max_hand_cards`，默认 20）就不该弃牌。
#[test]
fn hand_cap_not_asked_below_hold_limit() {
    let mgr = manager(3706);
    let room = start_and_pick(&mgr, 5);
    let seat = buqu_seat_with_six_cards(&mgr, &room);
    edit(&mgr, &room, |st| {
        // 不屈上限 2、手牌 6：远低于持有上限 20 → 不得压弃牌帧。
        assert_eq!(crate::skills::hand_limit(st, seat), 2);
        crate::resolve::push_discard(st, seat, 1, "hand_cap");
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| p.kind != "discard"),
        "获得牌后未超持有上限时不应弃牌：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.data.clone()))
    );
    st_check(&mgr, &room);
}

/// 超过持有上限（`config.max_hand_cards`）时仍弃至持有上限，而不是弃到体力/技能手牌上限。
#[test]
fn hand_cap_uses_hold_limit_not_hand_limit() {
    let mgr = manager(3707);
    let room = start_and_pick(&mgr, 5);
    let seat = buqu_seat_with_six_cards(&mgr, &room);
    let (mut required, mut limit) = (0, 0);
    edit(&mgr, &room, |st| {
        st.config.max_hand_cards = 4;
        crate::resolve::push_discard(st, seat, 1, "hand_cap");
        advance_once(st);
        if let Some(p) = st.pending.as_ref() {
            required = p.data["required"].as_u64().unwrap_or(0) as usize;
            limit = p.data["limit"].as_u64().unwrap_or(0) as usize;
        }
    });
    assert_eq!(required, 2, "持有上限 4、手牌 6 → 需弃 2 张（不是按不屈上限 2 弃 4 张）");
    assert_eq!(limit, 4, "上限展示应为持有上限 4");
    st_check(&mgr, &room);
}

/// 弃牌阶段仍按技能手牌上限（不屈=创数）弃牌。
#[test]
fn discard_phase_still_uses_skill_hand_limit() {
    let mgr = manager(3708);
    let room = start_and_pick(&mgr, 5);
    buqu_seat_with_six_cards(&mgr, &room);
    let (mut required, mut limit) = (0, 0);
    edit(&mgr, &room, |st| {
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
        if let Some(p) = st.pending.as_ref() {
            required = p.data["required"].as_u64().unwrap_or(0) as usize;
            limit = p.data["limit"].as_u64().unwrap_or(0) as usize;
        }
    });
    assert_eq!(required, 4, "弃牌阶段：手牌 6 − 不屈上限 2 = 4");
    assert_eq!(limit, 2, "弃牌阶段上限展示应为技能上限 2");
    st_check(&mgr, &room);
}

/// 濒死被【桃】救起后，仍须结算「受到伤害后」钩子（真机反馈：审时阴不发动）。
#[test]
fn shenshi_yin_triggers_after_dying_rescue() {
    let mgr = manager(3711);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut victim, mut rescuer) = (0, 0, 0);
    let mut tao = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        let others = other_seats(st, attacker);
        victim = others[0];
        rescuer = others[1];
        set_general(st, victim, "sgs_hero_404"); // 蒯越蒯良：审时
        st.seats[victim].hp = 1;
        st.seats[victim].flags.insert("yj:shenshi".into(), true); // 阴面
        // 审时（阴）需要受害者有手牌（交给来源一张）。
        give_filtered(st, victim, |c| c.def == "shan");
        // 来源留一张牌：反馈/节命等「获得来源牌」类钩子需要目标有牌。
        give_filtered(st, attacker, |c| c.def == "shan");
        tao = give(st, rescuer, "tao");
        crate::resolve::apply_damage(st, victim, 1, Some(attacker), crate::resolve::DamageType::Normal, None);
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    let victim_pid = pid_of(&st, victim);
    let mut p = st.pending.clone().expect("应进入濒死求桃");
    assert_eq!(p.kind, "dying");
    if p.actor == victim_pid {
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
        p = state_of(&mgr, &room).pending.clone().expect("仍有救援者求桃窗口");
    }
    assert_eq!(p.kind, "dying");
    mgr.execute_action(&room, &p.actor, &format!("respond:{tao}"))
        .expect("救援者出【桃】");
    // 救起后：伤害后钩子必须结算 → 审时（阴）询问。
    edit(&mgr, &room, |st| advance_once(st));
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1, "被【桃】救起，体力 1");
    let pending = st.pending.clone().expect("救起后应结算审时（阴）");
    assert_eq!(
        pending.skill.as_deref(),
        Some("shenshi"),
        "审时（阴）应在濒死救起后发动：{:?}",
        (pending.kind.clone(), pending.skill.clone())
    );
    assert!(pending.options.iter().any(|o| o.id == "choose:shenshi"));
    st_check(&mgr, &room);
}

/// 濒死+被【桃】救起 → 检查救起后结算的「受到伤害后」技能（pending.skill）。
fn damage_to_dying_then_rescue(victim_general: &str, seed: u64) -> (GameManager, String, usize, usize) {
    let mgr = manager(seed);
    let room = start_and_pick(&mgr, 5);
    let (mut attacker, mut victim, mut rescuer) = (0, 0, 0);
    let mut tao = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        attacker = st.turn;
        set_general(st, attacker, "sgs_hero_1");
        let others = other_seats(st, attacker);
        victim = others[0];
        rescuer = others[1];
        set_general(st, victim, victim_general);
        st.seats[victim].hp = 1;
        give_filtered(st, victim, |c| c.def == "shan");
        // 来源留一张牌：反馈/节命等「获得来源牌」类钩子需要目标有牌。
        give_filtered(st, attacker, |c| c.def == "shan");
        tao = give(st, rescuer, "tao");
        crate::resolve::apply_damage(
            st,
            victim,
            1,
            Some(attacker),
            crate::resolve::DamageType::Normal,
            None,
        );
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    let victim_pid = pid_of(&st, victim);
    let mut p = st.pending.clone().expect("应进入濒死求桃");
    assert_eq!(p.kind, "dying");
    if p.actor == victim_pid {
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
        p = state_of(&mgr, &room).pending.clone().expect("仍有救援者求桃窗口");
    }
    assert_eq!(p.kind, "dying");
    mgr.execute_action(&room, &p.actor, &format!("respond:{tao}"))
        .expect("救援者出【桃】");
    edit(&mgr, &room, |st| advance_once(st));
    (mgr, room, victim, rescuer)
}

#[test]
fn yiji_triggers_after_dying_rescue() {
    let (mgr, room, victim, _rescuer) = damage_to_dying_then_rescue("sgs_hero_20", 3720); // 郭嘉
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("yiji"),
        "遗计应在濒死救起后发动"
    );
    st_check(&mgr, &room);
}

#[test]
fn fankui_triggers_after_dying_rescue() {
    let (mgr, room, victim, _rescuer) = damage_to_dying_then_rescue("sgs_hero_16", 3721); // 司马懿
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1);
    // 反馈走 `PickCard`（pending.skill 为空），用 kind + 选项确认。
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.clone()),
        Some("pick_card".into()),
        "反馈应在濒死救起后发动（pick_card 选牌）"
    );
    st_check(&mgr, &room);
}

#[test]
fn jieming_triggers_after_dying_rescue() {
    let (mgr, room, victim, _rescuer) = damage_to_dying_then_rescue("sgs_hero_35", 3722); // 荀彧
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[victim].hp, 1);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.skill.as_deref()),
        Some("jieming"),
        "节命应在濒死救起后发动"
    );
    st_check(&mgr, &room);
}

/// 溃诛令**自己**摸牌后不得再要求弃牌（弃牌阶段只按上限弃一次；真机 `#247-251`）。
#[test]
fn kuizhu_self_draw_does_not_ask_second_discard() {
    let mgr = manager(3710);
    let room = start_and_pick(&mgr, 5);
    let (mut cur, mut cur_pid, mut first) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        cur = st.turn;
        set_general(st, cur, "sgs_hero_403"); // 孙亮：溃诛（max_hp 3）
        st.seats[cur].max_hp = 3;
        st.seats[cur].hp = 3;
        for seat in other_seats(st, cur) {
            set_general(st, seat, "sgs_hero_1");
        }
        for _ in 0..4 {
            give_filtered(st, cur, |c| c.def == "sha");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        cur_pid = st.seats[cur].player_id.clone();
        first = st.seats[cur].hand[0].clone();
    });
    // 弃 1 张（手牌 4 → 3，上限 3）→ 溃诛 X=1 → 令自己摸 1（回到 4 > 上限 3，不得再弃）
    edit(&mgr, &room, |st| advance_once(st));
    let st = state_of(&mgr, &room);
    assert_eq!(st.pending.as_ref().map(|p| p.kind.clone()), Some("discard".into()));
    mgr.execute_action(&room, &cur_pid, &format!("discard:{first}"))
        .unwrap();
    edit(&mgr, &room, |st| advance_once(st));
    mgr.execute_action(&room, &cur_pid, "choose:kuizhu:draw").unwrap();
    edit(&mgr, &room, |st| advance_once(st));
    mgr.execute_action(&room, &cur_pid, &format!("target:{cur_pid}"))
        .unwrap();
    edit(&mgr, &room, |st| advance_once(st));
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[cur].hand.len(), 4, "溃诛令自己摸 1：手牌回到 4");
    assert!(
        st.pending.as_ref().is_none_or(|p| !(p.kind == "discard" && p.actor == cur_pid)),
        "溃诛补牌后不得再要求孙亮弃牌：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.actor.clone(), p.data.clone()))
    );
    assert!(st.history.iter().any(|e| e.text.contains("溃诛")));
    st_check(&mgr, &room);
}

/// 拒战（阴）：使用【杀】指定目标后获得其一张牌；若未超持有上限，**不得**要求弃牌。
#[test]
fn juzhan_yin_gain_does_not_ask_discard() {
    let mgr = manager(3709);
    let room = start_and_pick(&mgr, 5);
    let (mut yan, mut target, mut sha) = (0, 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        yan = st.turn;
        set_general(st, yan, "sgs_hero_400"); // 严颜：拒战
        target = other_seats(st, yan)[0];
        set_general(st, target, "sgs_hero_1");
        give(st, target, "shan");
        // 手牌 4 = 体力上限 4：获得 1 张后 5 > 手牌上限，但仍远低于持有上限 20。
        for _ in 0..4 {
            give_filtered(st, yan, |c| c.def == "tao");
        }
        sha = give(st, yan, "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        st.seats[yan].flags.insert("yj:juzhan".into(), true); // 阴面
    });
    let st = state_of(&mgr, &room);
    let (yan_pid, target_pid) = (pid_of(&st, yan), pid_of(&st, target));
    mgr.execute_action(&room, &yan_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &yan_pid, &format!("target:{target_pid}"))
        .unwrap();
    mgr.execute_action(&room, &yan_pid, "choose:juzhan").unwrap();
    mgr.execute_action(&room, &yan_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[yan].hand.len(), 5, "拒战（阴）：获得目标一张牌");
    assert!(
        st.pending
            .as_ref()
            .is_none_or(|p| p.kind != "discard"),
        "拒战（阴）获得牌后不应要求弃牌：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.data.clone()))
    );
    st_check(&mgr, &room);
}

fn st_check(mgr: &GameManager, room: &str) {
    let st = state_of(mgr, room);
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 解围②：移动「场上一张牌」— 来源含自己、目的为任意角色（两阶段）
// ---------------------------------------------------------------------------

#[test]
fn jiewei_move_lists_self_and_moves_to_chosen_player() {
    let mgr = manager(3707);
    let room = start_and_pick(&mgr, 5);
    let (mut caopi, mut caoren, mut caoren_pid, mut caopi_pid, mut other_pid) =
        (0, 0, String::new(), String::new(), String::new());
    let (mut own_equip, mut other_equip) = (String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        caopi = st.turn;
        set_general(st, caopi, "sgs_hero_43"); // 曹丕：放逐
        caopi_pid = pid_of(st, caopi);
        caoren = other_seats(st, caopi)[0];
        set_general(st, caoren, "sgs_hero_29"); // 曹仁：解围
        caoren_pid = pid_of(st, caoren);
        let third = other_seats(st, caopi)[1];
        other_pid = pid_of(st, third);
        st.seats[caoren].flipped = true;
        own_equip = give_filtered(st, caoren, |c| c.def == "qinggang_jian");
        st.seats[caoren].hand.retain(|c| c != &own_equip);
        st.seats[caoren]
            .equips
            .insert(EquipSlot::Weapon, Some(own_equip.clone()));
        other_equip = give_filtered(st, third, |c| c.def == "zhangba_shemao");
        st.seats[third].hand.retain(|c| c != &other_equip);
        st.seats[third]
            .equips
            .insert(EquipSlot::Weapon, Some(other_equip.clone()));
        give_filtered(st, caoren, |c| c.def == "shan");
        damage_and_advance(st, caopi, 1, Some(caoren), crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("fangzhu"));
    mgr.execute_action(&room, &caopi_pid, &format!("target:{caoren_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiewei"), "翻回正面触发解围");
    assert!(
        pending_options_with(&st, "choose:jiewei").len() == 1,
        "解围询问"
    );
    mgr.execute_action(&room, &caoren_pid, "choose:jiewei").unwrap();
    // 一阶段：弃 1 张牌（选装备「弃置装备」也行，这里弃手牌）。
    let hand_card = state_of(&mgr, &room).seats[caoren].hand[0].clone();
    mgr.execute_action(&room, &caoren_pid, &format!("choose:{hand_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    // 三阶段 1：选择移动来源玩家（手牌/装备/判定区任一非空，含自己）。
    let srcs = pending_options_with(&st, "msrc:");
    assert!(
        srcs.contains(&format!("msrc:{caoren_pid}")) && srcs.contains(&format!("msrc:{other_pid}")),
        "来源玩家列表应包含自己与他人：{srcs:?}"
    );
    mgr.execute_action(&room, &caoren_pid, &format!("msrc:{caoren_pid}"))
        .unwrap();
    // 三阶段 2：该玩家的装备/判定区 + 随机手牌；不列他人的牌。
    let st = state_of(&mgr, &room);
    let moves = pending_options_with(&st, "move:");
    assert!(
        moves.contains(&format!("move:{own_equip}")) && moves.contains(&"move:hand".to_string()),
        "来源玩家的牌应含装备与「随机手牌」：{moves:?}"
    );
    assert!(
        !moves.contains(&format!("move:{other_equip}")),
        "不应列出其他玩家的牌（来源已限定）：{moves:?}"
    );
    mgr.execute_action(&room, &caoren_pid, &format!("move:{own_equip}"))
        .unwrap();
    // 三阶段 3：目的玩家 = 排除源玩家 + 排除同槽位已装备者。
    let st = state_of(&mgr, &room);
    let dests = pending_options_with(&st, "to:");
    assert!(
        !dests.contains(&format!("to:{caoren_pid}")),
        "目的不应包含源玩家：{dests:?}"
    );
    assert!(
        !dests.contains(&format!("to:{other_pid}")),
        "目的不应包含武器槽已装备的玩家（冲突排除）：{dests:?}"
    );
    assert!(!dests.is_empty(), "应有其他可用目的玩家");
    let dest_pid = dests[0].trim_start_matches("to:").to_string();
    mgr.execute_action(&room, &caoren_pid, &dests[0]).unwrap();
    let st = state_of(&mgr, &room);
    let dest = st.seat_index(&dest_pid).unwrap();
    assert_eq!(
        st.seats[dest]
            .equips
            .get(&EquipSlot::Weapon)
            .cloned()
            .flatten(),
        Some(own_equip),
        "解围：装备移动到指定玩家的武器区"
    );
    assert!(st.pending.is_none(), "解围交互结束");
    st.check_conservation().unwrap();
}

#[test]
fn jiewei_moves_random_hand_card_to_any_player() {
    let mgr = manager(3709);
    let room = start_and_pick(&mgr, 5);
    let (mut caopi, mut caoren, mut caoren_pid, mut caopi_pid) =
        (0, 0, String::new(), String::new());
    let mut target_pid = String::new();
    let mut target_hand_before = 0usize;
    edit(&mgr, &room, |st| {
        clear_hands(st);
        for seat in st.seats.iter_mut() {
            for slot in EquipSlot::ALL {
                if let Some(Some(id)) = seat.equips.insert(slot, None) {
                    st.deck.discard.push(id);
                }
            }
            let judge = std::mem::take(&mut seat.judge_zone);
            for id in judge {
                st.deck.discard.push(id);
            }
        }
        caopi = st.turn;
        set_general(st, caopi, "sgs_hero_43");
        caopi_pid = pid_of(st, caopi);
        caoren = other_seats(st, caopi)[0];
        set_general(st, caoren, "sgs_hero_29");
        caoren_pid = pid_of(st, caoren);
        // 场上放一名周泰（奋激持有者）：移动手牌会压奋激帧，用于回归「移动后重复询问」。
        let zhoutai = other_seats(st, caopi)[2];
        set_general(st, zhoutai, "sgs_hero_210");
        // 来源玩家（用于随机手牌移动；目的手牌无冲突限制）：给 2 张手牌。
        let target = other_seats(st, caopi)[1];
        target_pid = pid_of(st, target);
        give_filtered(st, target, |c| c.def == "tao");
        give_filtered(st, target, |c| c.def == "sha");
        target_hand_before = 2;
        give_filtered(st, caoren, |c| c.def == "shan");
        st.seats[caoren].flipped = true;
        damage_and_advance(st, caopi, 1, Some(caoren), crate::resolve::DamageType::Normal, None);
    });
    // 放逐先结算（曹丕选目标 → 曹仁翻面并摸牌），随后触发解围。
    mgr.execute_action(&room, &caopi_pid, &format!("target:{caoren_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiewei"), "翻回正面触发解围");
    mgr.execute_action(&room, &caoren_pid, "choose:jiewei").unwrap();
    let hand_card = state_of(&mgr, &room).seats[caoren].hand[0].clone();
    mgr.execute_action(&room, &caoren_pid, &format!("choose:{hand_card}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().and_then(|p| p.data["stage"].as_str()),
        Some("jiewei_src"),
        "解围：弃牌后进入选来源玩家"
    );
    // 来源 = 目标玩家（手里有牌，非曹仁）。
    mgr.execute_action(&room, &caoren_pid, &format!("msrc:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        pending_options_with(&st, "move:").contains(&"move:hand".to_string()),
        "来源玩家有手牌时应提供「随机手牌」选项"
    );
    mgr.execute_action(&room, &caoren_pid, "move:hand").unwrap();
    // 手牌移动无区域限制：所有存活玩家（除来源）都可作为目的。
    let st = state_of(&mgr, &room);
    let dests = pending_options_with(&st, "to:");
    assert!(
        dests.contains(&format!("to:{caoren_pid}")) && dests.contains(&format!("to:{caopi_pid}")),
        "随机手牌无区域限制，应可选任意其他存活玩家：{dests:?}"
    );
    mgr.execute_action(&room, &caoren_pid, &format!("to:{caoren_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let target_seat = st.seat_index(&target_pid).unwrap();
    assert_eq!(
        st.seats[target_seat].hand.len(),
        target_hand_before - 1,
        "来源玩家随机失去一张手牌"
    );
    assert!(
        st.seats[caoren].hand.len() >= 1,
        "目的玩家（曹仁）获得该手牌"
    );
    // 真机缺陷回归：移动手牌后应结束，不得再次要求移动（即使场上有奋激持有者）。
    assert!(
        st.pending
            .as_ref()
            .map_or(true, |p| p.skill.as_deref() != Some("jiewei")),
        "移动手牌后解围应结束，不应再次询问：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone(), p.data.clone()))
    );
    st.check_conservation().unwrap();
}

#[test]
fn jiewei_interaction_ends_when_no_cards_anywhere() {
    let mgr = manager(3708);
    let room = start_and_pick(&mgr, 5);
    let (mut caoren, mut caoren_pid, mut equip) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        // 全场清空手牌/装备/判定，只给曹仁一张装备用于解围的弃置。
        let mut cleared: Vec<String> = Vec::new();
        for seat in st.seats.iter_mut() {
            for slot in EquipSlot::ALL {
                if let Some(Some(id)) = seat.equips.insert(slot, None) {
                    cleared.push(id);
                }
            }
            cleared.extend(std::mem::take(&mut seat.judge_zone));
        }
        for id in cleared {
            st.deck.discard.push(id);
        }
        caoren = st.turn;
        set_general(st, caoren, "sgs_hero_29");
        caoren_pid = pid_of(st, caoren);
        equip = give_filtered(st, caoren, |c| c.def == "qinggang_jian");
        st.seats[caoren].hand.retain(|c| c != &equip);
        st.seats[caoren]
            .equips
            .insert(EquipSlot::Weapon, Some(equip.clone()));
        // 背面朝上 + 回合开始 → 跳过回合并触发解围。
        st.seats[caoren].flipped = true;
        st.turn_phase = TurnPhase::Prepare;
        st.turn_counters.clear();
        advance_once(st);
    });
    let _ = caoren;
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiewei"), "翻回正面触发解围");
    mgr.execute_action(&room, &caoren_pid, "choose:jiewei").unwrap();
    mgr.execute_action(&room, &caoren_pid, "choose:equip:Weapon")
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .map_or(true, |p| p.skill.as_deref() != Some("jiewei")),
        "全场（含自己）无手牌/装备/判定牌时应弃牌后直接结束解围交互：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("没有可移动的牌")),
        "应写历史说明交互结束"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 庸肆：弃牌阶段先弃 X（仅一次）→ 再按上限判定，不允许「反复要求弃 X」
// ---------------------------------------------------------------------------

#[test]
fn yongsi_discards_x_once_then_limits_to_hand_cap() {
    let mgr = manager(3710);
    let room = start_and_pick(&mgr, 5);
    let (mut ys, mut ys_pid) = (0, String::new());
    let (mut x, mut hand0) = (0usize, 0usize);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        ys = st.turn;
        set_general(st, ys, "sgs_hero_100"); // 袁术：庸肆
        ys_pid = pid_of(st, ys);
        for _ in 0..13 {
            give_filtered(st, ys, |c| c.def == "sha");
        }
        hand0 = st.seats[ys].hand.len();
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
        x = crate::skills::kingdom_count(st) as usize;
    });
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("应要求庸肆弃牌");
    assert_eq!(p.data["reason"], "yongsi");
    assert_eq!(p.data["required"], x.min(hand0), "庸肆：先弃 X 张");
    // 固定张数：必须一次凑齐（少选被拒，避免「反复要求弃 X 张」）
    let ids: Vec<String> = p.options.iter().map(|o| o.id.clone()).collect();
    assert!(
        mgr.execute_action(&room, &ys_pid, &format!("discard:{}", ids[0].trim_start_matches("discard:")))
            .is_err(),
        "庸肆：X>1 时不允许只弃 1 张（避免误以为在循环）"
    );
    // 一次弃满 X → 进入普通弃牌阶段（按上限），且庸肆不再出现。
    let csv = ids.iter().take(x.min(hand0)).map(|o| o.trim_start_matches("discard:")).collect::<Vec<_>>().join(",");
    mgr.execute_action(&room, &ys_pid, &format!("discard:{csv}")).unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.turn_counters.get("yongsi_discard_done").copied(),
        Some(1),
        "庸肆：本回合已结算标记"
    );
    let limit = crate::skills::hand_limit(&st, ys);
    let hand = st.seats[ys].hand.len();
    if hand > limit {
        let p2 = st.pending.clone().expect("应按上限继续要求弃牌");
        assert_eq!(p2.data["reason"], "discard_phase", "庸肆只触发一次");
        assert_eq!(p2.data["required"], hand - limit);
        let ids: Vec<String> = p2
            .options
            .iter()
            .map(|o| o.id.trim_start_matches("discard:").to_string())
            .take(hand - limit)
            .collect();
        mgr.execute_action(&room, &ys_pid, &format!("discard:{}", ids.join(",")))
            .unwrap();
        let st = state_of(&mgr, &room);
        assert!(
            st.pending
                .as_ref()
                .map_or(true, |p| p.data["reason"] != "yongsi"),
            "庸肆不得再次出现"
        );
        assert!(
            st.seats[ys].hand.len() <= crate::skills::hand_limit(&st, ys),
            "弃牌后手牌应 ≤ 上限"
        );
    }
    let st = state_of(&mgr, &room);
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 矢志（张嶷，锁定）：体力 1 时【闪】视为【杀】— 合法动作必须可执行
// ---------------------------------------------------------------------------

#[test]
fn shizhi_conversion_action_is_executable() {
    let mgr = manager(3711);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut shan) = (0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_337"); // 张嶷：矢志（锁定）
        pid = pid_of(st, seat);
        st.seats[seat].hp = 1;
        shan = give_filtered(st, seat, |c| c.def == "shan");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    let action = format!("skill:shizhi:{shan}");
    let legal: Vec<String> = crate::turn::legal_actions(&st, &pid).into_iter().map(|a| a.id).collect();
    assert!(legal.contains(&action), "体力 1 的【闪】应出现矢志转化动作：{legal:?}");
    mgr.execute_action(&room, &pid, &action)
        .expect("矢志转化动作必须可执行（legal/execute 同源）");
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.kind == "target" || p.kind == "choose_many"),
        "应进入【杀】选目标：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    st.check_conservation().unwrap();
}

#[test]
fn yongsi_discard_ends_even_with_fenji_owner_on_field() {
    // 真机缺陷回归：场上存在周泰（奋激）时，弃牌后压入奋激帧；
    // 若先压奋激再 pop，弃牌帧残留栈顶 → 反复要求弃 X 张直到手牌空。
    let mgr = manager(3712);
    let room = start_and_pick(&mgr, 5);
    let (mut ys, mut ys_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        ys = st.turn;
        set_general(st, ys, "sgs_hero_100"); // 袁术：庸肆
        ys_pid = pid_of(st, ys);
        // 周泰（奋激）：弃牌者手牌被弃置后可令其摸 2。
        let zt = other_seats(st, ys)[0];
        set_general(st, zt, "sgs_hero_210");
        for _ in 0..9 {
            give_filtered(st, ys, |c| c.def == "sha");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("庸肆应先弃 X 张");
    assert_eq!(p.data["reason"], "yongsi");
    let x = p.data["required"].as_u64().unwrap() as usize;
    let csv: Vec<String> = p
        .options
        .iter()
        .take(x)
        .map(|o| o.id.trim_start_matches("discard:").to_string())
        .collect();
    // 袁术自己弃满 X（若 fenji 帧顺序错误，这里之后会再次出现 yongsi 询问）。
    mgr.execute_action(&room, &ys_pid, &format!("discard:{}", csv.join(",")))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .map_or(true, |p| p.data["reason"] != "yongsi"),
        "庸肆只应结算一次：{:?}",
        st.pending
            .as_ref()
            .map(|p| (p.kind.clone(), p.skill.clone(), p.data.clone()))
    );
    assert!(
        !st.history
            .iter()
            .any(|e| e.text.contains("奋激") || e.text.contains("choose:fenji")),
        "自己弃牌（庸肆）不应触发奋激（奋激要求「手牌被其他角色弃置/获得」）"
    );
    assert_eq!(
        st.seats[ys].hand.len(),
        9 - x,
        "一次弃 X 张后手牌减少 X"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 奋激：仅「一名角色的手牌被其他角色弃置/获得」时触发
// ---------------------------------------------------------------------------

#[test]
fn fenji_triggers_on_guohe_hand_discard() {
    let mgr = manager(3713);
    let room = start_and_pick(&mgr, 5);
    let (mut actor, mut actor_pid, mut victim, mut victim_pid, mut guohe) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        actor = st.turn;
        actor_pid = pid_of(st, actor);
        let others = other_seats(st, actor);
        victim = others[0];
        victim_pid = pid_of(st, victim);
        // 周泰（奋激）持有者：第三人，既非使用者也不是被弃牌者。
        set_general(st, others[1], "sgs_hero_210");
        guohe = give_filtered(st, actor, |c| c.def == "guohe_chaiqiao");
        give_filtered(st, victim, |c| c.def == "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        advance_once(st);
    });
    mgr.execute_action(&room, &actor_pid, &format!("play:{guohe}"))
        .unwrap();
    mgr.execute_action(&room, &actor_pid, &format!("target:{victim_pid}"))
        .unwrap();
    // 无懈窗口：全员不出。
    for _ in 0..6 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "wuxie" {
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.clone()).as_deref(),
        Some("pick_card"),
        "过河拆桥应先选择要弃置的区域：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    mgr.execute_action(&room, &actor_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        pending_skill(&st),
        Some("fenji"),
        "过河拆桥弃置他人手牌应触发奋激：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    assert_eq!(st.seats[victim].hand.len(), 0, "受害者手牌已被弃置");
    st.check_conservation().unwrap();
}

#[test]
fn fenji_not_triggered_by_own_turn_draw() {
    // 自己回合内摸牌（此处郭嘉在自己回合受伤触发遗计摸 2）→ 不满足「回合外」，不触发奋激。
    let mgr = manager(3714);
    let room = start_and_pick(&mgr, 5);
    let (mut guojia, mut guojia_pid) = (0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        guojia = st.turn;
        set_general(st, guojia, "sgs_hero_20"); // 郭嘉：天妒/遗计
        guojia_pid = pid_of(st, guojia);
        let zt = other_seats(st, guojia)[0];
        set_general(st, zt, "sgs_hero_210"); // 周泰（奋激）
        damage_and_advance(st, guojia, 1, Some(zt), crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("yiji"), "受伤后应先询问遗计");
    // 遗计结算：摸/分配不应产生奋激询问。
    let mut inject = Vec::new();
    crate::tests::drive_mgr_inject(&mgr, &room, &mut inject);
    let st = state_of(&mgr, &room);
    assert!(
        st.pending
            .as_ref()
            .map_or(true, |p| p.skill.as_deref() != Some("fenji")),
        "自己回合内摸牌不应触发奋激：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    let _ = guojia_pid;
    st.check_conservation().unwrap();
}

#[test]
fn fenji_triggers_on_out_of_turn_hand_gain() {
    // 回合外手牌数增加（五谷丰登：当前回合玩家先选，其他角色随后各获得一张）→ 触发奋激。
    let mgr = manager(3715);
    let room = start_and_pick(&mgr, 5);
    let (mut actor, mut actor_pid, mut wugu) = (0, String::new(), String::new());
    let mut gainer_pid = String::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        actor = st.turn;
        actor_pid = pid_of(st, actor);
        let others = other_seats(st, actor);
        // 周泰（奋激）持有者：第三人。
        set_general(st, others[2], "sgs_hero_210");
        wugu = give_filtered(st, actor, |c| c.def == "wugu_fengdeng");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        advance_once(st);
    });
    mgr.execute_action(&room, &actor_pid, &format!("play:{wugu}"))
        .unwrap();
    // 处理无懈窗口（全员不出）+ 五谷逐个选牌（当前回合玩家先选，其余回合外角色随后获得）。
    let mut triggered = false;
    for _ in 0..40 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        match p.kind.as_str() {
            "wuxie" => {
                mgr.execute_action(&room, &p.actor, "pass").unwrap();
            }
            "wugu" => {
                if p.actor != actor_pid {
                    gainer_pid = p.actor.clone();
                }
                let pick = p
                    .options
                    .iter()
                    .find(|o| o.id.starts_with("pick:"))
                    .map(|o| o.id.clone());
                let Some(pick) = pick else { break };
                mgr.execute_action(&room, &p.actor, &pick).unwrap();
                let st2 = state_of(&mgr, &room);
                if st2
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.skill.as_deref() == Some("fenji"))
                {
                    triggered = true;
                    break;
                }
            }
            _ => break,
        }
    }
    let st = state_of(&mgr, &room);
    // 奋激在结算栈空时统一询问（五谷全部选完后），因此最终 pending 应为奋激。
    assert!(
        triggered || st.pending.as_ref().is_some_and(|p| p.skill.as_deref() == Some("fenji")),
        "回合外获得手牌应触发奋激（不判具体技能）：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    assert_eq!(pending_skill(&st), Some("fenji"));
    assert!(
        !gainer_pid.is_empty(),
        "触发者应为回合外获得手牌的角色"
    );
    st.check_conservation().unwrap();
}

#[test]
fn fenji_not_triggered_on_hand_transfer_to_turn_player() {
    // 顺手牵羊：受害者手牌「被获得」（进入对方手牌，未进弃牌堆），且获得者是当前回合玩家
    // → 按用户裁定（回合外 + 弃置到弃牌堆 / 手牌数增加）不触发。
    let mgr = manager(3716);
    let room = start_and_pick(&mgr, 5);
    let (mut actor, mut actor_pid, mut victim, mut victim_pid, mut shunshou) =
        (0, String::new(), 0, String::new(), String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        actor = st.turn;
        actor_pid = pid_of(st, actor);
        let others = other_seats(st, actor);
        victim = others[0];
        victim_pid = pid_of(st, victim);
        set_general(st, others[1], "sgs_hero_210"); // 周泰：奋激
        shunshou = give_filtered(st, actor, |c| c.def == "shunshou_qianyang");
        give_filtered(st, victim, |c| c.def == "sha");
        st.turn_phase = TurnPhase::Play;
        st.turn_counters.clear();
        advance_once(st);
    });
    mgr.execute_action(&room, &actor_pid, &format!("play:{shunshou}"))
        .unwrap();
    mgr.execute_action(&room, &actor_pid, &format!("target:{victim_pid}"))
        .unwrap();
    for _ in 0..6 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "wuxie" {
            break;
        }
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    mgr.execute_action(&room, &actor_pid, "pick:hand").unwrap();
    let st = state_of(&mgr, &room);
    assert!(
        st.history
            .iter()
            .any(|e| e.text.contains("顺手牵羊")),
        "顺手牵羊应已结算"
    );
    assert!(
        st.pending
            .as_ref()
            .map_or(true, |p| p.skill.as_deref() != Some("fenji")),
        "手牌被「获得」但未进弃牌堆、且获得者为当前回合玩家 → 不触发奋激：{:?}",
        st.pending.as_ref().map(|p| (p.kind.clone(), p.skill.clone()))
    );
    let _ = victim;
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 固政：弃牌阶段内「庸肆先弃 X 张」也属于本阶段因弃置进弃牌堆的牌
// ---------------------------------------------------------------------------

#[test]
fn guzheng_includes_yongsi_discards() {
    let mgr = manager(3717);
    let room = start_and_pick(&mgr, 5);
    let (mut ys, mut ys_pid, mut owner, mut owner_pid) = (0, String::new(), 0, String::new());
    edit(&mgr, &room, |st| {
        clear_hands(st);
        ys = st.turn;
        set_general(st, ys, "sgs_hero_100"); // 袁术：庸肆
        ys_pid = pid_of(st, ys);
        owner = other_seats(st, ys)[0];
        set_general(st, owner, "sgs_hero_55"); // 张昭张弘：固政
        owner_pid = pid_of(st, owner);
        for _ in 0..9 {
            give_filtered(st, ys, |c| c.def == "sha");
        }
        st.turn_phase = TurnPhase::Discard;
        st.turn_counters.clear();
        st.phase_discarded.clear();
        advance_once(st);
    });
    let st = state_of(&mgr, &room);
    let p = st.pending.clone().expect("庸肆先弃 X 张");
    assert_eq!(p.data["reason"], "yongsi");
    let x = p.data["required"].as_u64().unwrap() as usize;
    let take: Vec<String> = p
        .options
        .iter()
        .take(x)
        .map(|o| o.id.trim_start_matches("discard:").to_string())
        .collect();
    let yongsi_cards = take.clone();
    mgr.execute_action(&room, &ys_pid, &format!("discard:{}", take.join(",")))
        .unwrap();
    // 若仍超上限，继续弃；随后进入固政询问。
    for _ in 0..6 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.skill.as_deref() == Some("guzheng") {
            break;
        }
        if p.data["reason"] == "discard_phase" {
            let need = p.data["required"].as_u64().unwrap() as usize;
            let ids: Vec<String> = p
                .options
                .iter()
                .take(need)
                .map(|o| o.id.trim_start_matches("discard:").to_string())
                .collect();
            mgr.execute_action(&room, &ys_pid, &format!("discard:{}", ids.join(",")))
                .unwrap();
            continue;
        }
        break;
    }
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("guzheng"), "弃牌阶段结束应询问固政");
    let owner_seat = st.seat_index(&owner_pid).unwrap();
    assert_eq!(owner_seat, owner);
    // 固政的候选牌应包含庸肆弃置的牌。
    let frame_cards: Vec<String> = match crate::resolve::top(&st) {
        Some(crate::resolve::Frame::Guzheng(f)) => f.cards.clone(),
        _ => Vec::new(),
    };
    assert!(
        yongsi_cards.iter().all(|c| frame_cards.contains(c)),
        "固政候选牌应包含庸肆弃置的牌：yongsi={yongsi_cards:?} frame={frame_cards:?}"
    );
    mgr.execute_action(&room, &owner_pid, "choose:guzheng").unwrap();
    let st = state_of(&mgr, &room);
    let options: Vec<String> = st
        .pending
        .as_ref()
        .map(|p| {
            p.options
                .iter()
                .map(|o| o.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let target = yongsi_cards
        .iter()
        .map(|c| format!("give:{c}"))
        .find(|id| options.contains(id))
        .expect("固政应可选择庸肆弃置的牌交还给袁术");
    mgr.execute_action(&room, &owner_pid, &target).unwrap();
    let st = state_of(&mgr, &room);
    let given = target.trim_start_matches("give:").to_string();
    assert!(
        st.seats[ys].hand.contains(&given),
        "固政：选中的庸肆弃牌应交给弃牌者（袁术）"
    );
    st.check_conservation().unwrap();
}

// ---------------------------------------------------------------------------
// 骄矜（孙鲁班）：可选装备区的牌弃置（legal/execute 此前不一致）
// ---------------------------------------------------------------------------

#[test]
fn jiaojin_can_discard_equipment_area_card() {
    let mgr = manager(3718);
    let room = start_and_pick(&mgr, 5);
    let (mut seat, mut pid, mut equip, mut hp_before) = (0, String::new(), String::new(), 0);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        seat = st.turn;
        set_general(st, seat, "sgs_hero_327"); // 孙鲁班：骄矜
        pid = pid_of(st, seat);
        equip = give_filtered(st, seat, |c| c.def == "qinggang_jian");
        st.seats[seat].hand.retain(|c| c != &equip);
        st.seats[seat]
            .equips
            .insert(EquipSlot::Weapon, Some(equip.clone()));
        hp_before = st.seats[seat].hp;
        let src = other_seats(st, seat)[0];
        damage_and_advance(st, seat, 1, Some(src), crate::resolve::DamageType::Normal, None);
    });
    let st = state_of(&mgr, &room);
    assert_eq!(pending_skill(&st), Some("jiaojin"), "应询问骄矜");
    let opt = format!("equip:{equip}");
    assert!(
        st.pending
            .as_ref()
            .is_some_and(|p| p.options.iter().any(|o| o.id == opt)),
        "骄矜应提供装备区牌的选项：{:?}",
        st.pending.as_ref().map(|p| p.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())
    );
    mgr.execute_action(&room, &pid, &opt)
        .expect("骄矜弃置装备区的牌必须可执行（legal/execute 同源）");
    let st = state_of(&mgr, &room);
    assert!(
        st.seats[seat].equips.values().all(|c| c.is_none()),
        "装备区的牌应已弃置"
    );
    assert_eq!(st.seats[seat].hp, hp_before, "弃置装备牌应抵消该 1 点伤害");
    assert!(st.pending.is_none());
    st.check_conservation().unwrap();
}
