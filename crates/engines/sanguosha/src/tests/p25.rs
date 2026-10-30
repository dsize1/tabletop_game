//! P2.5 测试：特殊交互区引擎契约（五谷丰登 `wugu` / 观星 `guanxing` + `arrange`）。

use super::p24::{in_hand, others, set_general, start_and_pick};
use super::{
    clear_hands, edit, give, manager, player_view, public_view, resolve_arrange, set_top, state_of,
};
use crate::resolve_zone::{settle_slots, WUGU_ZONE};
use crate::state::*;
use game_engine_core::GameManager;
use serde_json::Value;

fn pid(st: &SgsState, idx: usize) -> String {
    st.seats[idx].player_id.clone()
}

/// 推进「五谷丰登」直到结算完毕：选牌时取第一张，无懈窗口按 `decide` 决定。
fn run_wugu<F>(mgr: &GameManager, room: &str, mut decide: F) -> Vec<(String, String)>
where
    F: FnMut(&SgsState, &Pending) -> Option<String>,
{
    let mut picked = Vec::new();
    for _ in 0..80 {
        let st = state_of(mgr, room);
        let Some(p) = st.pending.clone() else {
            return picked;
        };
        match p.kind.as_str() {
            "wugu" => {
                let choice = p
                    .options
                    .first()
                    .unwrap_or_else(|| panic!("五谷丰登没有可选的牌"))
                    .id
                    .clone();
                let card = choice.trim_start_matches("pick:").to_string();
                picked.push((p.actor.clone(), card));
                mgr.execute_action(room, &p.actor, &choice).unwrap();
            }
            "wuxie" => {
                let action = decide(&st, &p).unwrap_or_else(|| "pass".to_string());
                mgr.execute_action(room, &p.actor, &action).unwrap();
            }
            other => panic!("五谷丰登过程中出现意外 pending：{other}"),
        }
    }
    panic!("五谷丰登未收敛");
}

#[test]
fn wugu_reveals_for_every_alive_player_and_creator_picks_first() {
    let mgr = manager(70);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wugu_fengdeng");
        set_top(st, &["sha", "shan", "tao", "juedou", "wuzhong_shengyou"]);
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let wugu = in_hand(&st, cur, "wugu_fengdeng");
    let processing_before = st.deck.processing.len();
    let discard_before = st.deck.discard.len();

    mgr.execute_action(&room, &cur_pid, &format!("play:{wugu}"))
        .unwrap();

    // 亮牌：存活角色数 = 5 张，且全部停在处理区（牌数守恒）。
    let st = state_of(&mgr, &room);
    let zone = st
        .special_zones
        .get(WUGU_ZONE)
        .expect("应创建五谷丰登特殊区");
    assert_eq!(zone.visibility, Visibility::Public);
    assert_eq!(zone.owner, None);
    let revealed = zone.payload["cards"].as_array().expect("cards").len();
    assert_eq!(revealed, 5, "亮牌数应等于存活角色数");
    assert_eq!(st.deck.processing.len(), processing_before + 6);
    st.check_conservation().unwrap();

    // 公开区可见（含牌面），个人区同样可见。
    for view in [public_view(&mgr, &room), player_view(&mgr, &room, "u2")] {
        let z = &view["special_zones"]["wugu"];
        assert!(z.is_object(), "五谷丰登应为公开特殊区：{view}");
        assert_eq!(z["cards"].as_array().expect("cards").len(), 5);
    }

    // 选择顺序：释放者先选，之后按座位左手侧。
    let expected: Vec<String> = std::iter::once(cur)
        .chain(others(&st, cur).into_iter())
        .map(|i| pid(&st, i))
        .collect();
    let picked = run_wugu(&mgr, &room, |_, _| None);
    let order: Vec<String> = picked.iter().map(|(a, _)| a.clone()).collect();
    assert_eq!(order, expected, "五谷丰登应由释放者先选");
    let cards: Vec<String> = picked.iter().map(|(_, c)| c.clone()).collect();
    assert_eq!(
        cards
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5,
        "每人拿到不同的牌"
    );

    // 结算完毕：特殊区移除、牌进各自手牌、五谷本身进弃牌堆。
    let st = state_of(&mgr, &room);
    assert!(
        st.special_zones.get(WUGU_ZONE).is_none(),
        "结算后应移除特殊区"
    );
    assert!(st.deck.processing.is_empty(), "亮出的牌应全部离开处理区");
    assert_eq!(
        st.deck.discard.len(),
        discard_before + 1,
        "仅【五谷丰登】本身进弃牌堆"
    );
    for i in 0..st.seats.len() {
        assert_eq!(st.seats[i].hand.len(), 1, "每人各得 1 张");
    }
    st.check_conservation().unwrap();
}

#[test]
fn wugu_wuxie_skips_chooser_and_leftover_goes_to_discard() {
    let mgr = manager(71);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wugu_fengdeng");
        let skipped = st.seats_from_left_of(cur)[1];
        give(st, skipped, "wuxie_keji");
        set_top(st, &["sha", "shan", "tao", "juedou", "wuzhong_shengyou"]);
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let choosers: Vec<usize> = std::iter::once(cur)
        .chain(others(&st, cur).into_iter())
        .collect();
    let skipped = choosers[2];
    let skipped_pid = pid(&st, skipped);
    let skipped_name = st.seats[skipped].general_name.clone();
    let wuxie = in_hand(&st, skipped, "wuxie_keji");
    let wugu = in_hand(&st, cur, "wugu_fengdeng");
    let discard_before = st.deck.discard.len();
    let mut used = false;

    mgr.execute_action(&room, &cur_pid, &format!("play:{wugu}"))
        .unwrap();
    let picked = run_wugu(&mgr, &room, |_st, p| {
        // 只在「针对该被无懈者」的那一层打出无懈（其余窗口一律不出）。
        let root = p.data["root"].as_str().unwrap_or_default().to_string();
        if !used && p.actor == skipped_pid && root.contains(&skipped_name) {
            used = true;
            return Some(format!("respond:{wuxie}"));
        }
        None
    });
    assert!(used, "应命中针对该玩家的无懈窗口");

    let actors: Vec<String> = picked.iter().map(|(a, _)| a.clone()).collect();
    assert!(
        !actors.contains(&skipped_pid),
        "被无懈的玩家不选牌：{actors:?}"
    );
    assert_eq!(actors.len(), 4, "其余 4 人各选 1 张");

    let st = state_of(&mgr, &room);
    assert!(!st.special_zones.contains_key(WUGU_ZONE));
    assert!(st.deck.processing.is_empty());
    assert_eq!(st.seats[skipped].hand.len(), 0, "被无懈者不得牌");
    // 弃牌堆：五谷本身 + 无懈 + 剩余 1 张亮牌。
    assert_eq!(st.deck.discard.len(), discard_before + 3);
    st.check_conservation().unwrap();
}

/// 把下一位玩家换成诸葛亮并结束当前回合，触发准备阶段的观星。
fn enter_guanxing(mgr: &GameManager, room: &str) -> (usize, String) {
    edit(mgr, room, |st| {
        clear_hands(st);
        let next = st.seats_from_left_of(st.turn)[0];
        set_general(st, next, "sgs_hero_4");
    });
    let st = state_of(mgr, room);
    let cur_pid = pid(&st, st.turn);
    mgr.execute_action(room, &cur_pid, "end_play").unwrap();
    let st = state_of(mgr, room);
    let pending = st.pending.clone().expect("观星应在准备阶段停下");
    assert_eq!(pending.kind, "arrange");
    (st.turn, pending.actor)
}

#[test]
fn guanxing_builds_2x5_slots_owner_only() {
    let mgr = manager(72);
    let room = start_and_pick(&mgr, 5);
    let (owner, owner_pid) = enter_guanxing(&mgr, &room);

    let st = state_of(&mgr, &room);
    assert_eq!(pid(&st, owner), owner_pid);
    let zone = st.special_zones.get("guanxing").expect("应创建观星特殊区");
    assert_eq!(zone.visibility, Visibility::Owner);
    assert_eq!(zone.owner.as_deref(), Some(owner_pid.as_str()));
    assert_eq!(zone.payload["rows"], 5);
    assert_eq!(zone.payload["cols"], 2);
    assert_eq!(zone.payload["rev"], 1);
    assert_eq!(zone.payload["interaction"]["kind"], "ordered");
    assert_eq!(zone.payload["interaction"]["commit"], "arrange");

    // 初始分布：L1..Lk 依次填入，右列为空；k = min(存活角色数, 5)。
    let slots = zone.payload["slots"].as_object().expect("slots");
    assert_eq!(slots.len(), 10, "槽位恒为 10");
    let alive = st.alive_indices().len().min(5);
    let mut filled = Vec::new();
    for k in 1..=5 {
        let v = &slots[&format!("L{k}")];
        if k <= alive {
            assert!(v.is_string(), "L{k} 应有牌");
            filled.push(v.as_str().unwrap().to_string());
        } else {
            assert!(v.is_null(), "L{k} 应为空");
        }
        assert!(slots[&format!("R{k}")].is_null(), "R{k} 初始应为空");
    }
    assert_eq!(st.deck.processing.len(), alive, "观星的牌计入处理区");
    st.check_conservation().unwrap();

    // 他人不可见；公共区不可见；本人可见。
    assert!(
        public_view(&mgr, &room)["special_zones"]
            .get("guanxing")
            .is_none(),
        "观星不得出现在公共视图"
    );
    for p in ["u1", "u2", "u3", "u4", "u5"] {
        if p == owner_pid {
            continue;
        }
        assert!(
            player_view(&mgr, &room, p)["special_zones"]
                .get("guanxing")
                .is_none(),
            "{p} 不应看到观星"
        );
    }
    let mine = player_view(&mgr, &room, &owner_pid);
    assert_eq!(
        mine["special_zones"]["guanxing"]["slots"]["L1"],
        slots["L1"].clone(),
        "本人视图应含槽位"
    );

    // 默认提交选项 = 当前结算顺序（左列→右列，跳空压缩）。
    let pending = st.pending.clone().expect("arrange pending");
    let expect_order = filled.join(",");
    assert_eq!(pending.options[0].id, format!("arrange:{expect_order}"));
}

#[test]
fn guanxing_arrange_rejects_bad_input_and_rewrites_deck() {
    let mgr = manager(73);
    let room = start_and_pick(&mgr, 5);
    let (owner, owner_pid) = enter_guanxing(&mgr, &room);

    let st = state_of(&mgr, &room);
    let cards: Vec<String> = (1..=5)
        .map(|k| {
            st.special_zones["guanxing"].payload["slots"][&format!("L{k}")]
                .as_str()
                .expect("槽位有牌")
                .to_string()
        })
        .collect();
    let draw_before = st.deck.draw.len();
    let discard_before = st.deck.discard.len();

    // 非法：少一张 / 重复 / 混入非本次取出的牌。
    let too_few = cards[..4].join(",");
    assert!(mgr
        .execute_action(&room, &owner_pid, &format!("arrange:{too_few}"))
        .is_err());
    let dup = [
        cards[0].clone(),
        cards[0].clone(),
        cards[1].clone(),
        cards[2].clone(),
        cards[3].clone(),
    ]
    .join(",");
    assert!(mgr
        .execute_action(&room, &owner_pid, &format!("arrange:{dup}"))
        .is_err());
    let st = state_of(&mgr, &room);
    let foreign: String = st.seats[owner].hand.first().cloned().unwrap_or_else(|| {
        st.cards
            .keys()
            .find(|c| !cards.contains(*c) && !st.deck.processing.contains(*c))
            .cloned()
            .unwrap()
    });
    let mixed = [
        cards[0].clone(),
        cards[1].clone(),
        cards[2].clone(),
        cards[3].clone(),
        foreign,
    ]
    .join(",");
    assert!(mgr
        .execute_action(&room, &owner_pid, &format!("arrange:{mixed}"))
        .is_err());
    // 非法提交不改状态。
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.pending.as_ref().map(|p| p.kind.as_str()),
        Some("arrange")
    );
    assert_eq!(st.deck.processing.len(), 5);
    st.check_conservation().unwrap();

    // 合法：倒序提交 → 第一个 id 成为新牌堆顶（draw 末尾）。
    let mut order = cards.clone();
    order.reverse();
    mgr.execute_action(&room, &owner_pid, &format!("arrange:{}", order.join(",")))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "提交后 pending 应清空");
    assert!(
        st.special_zones.get("guanxing").is_none(),
        "提交后移除特殊区"
    );
    assert!(st.deck.processing.is_empty(), "观星牌应全部回牌堆");
    // 观星结算后回合继续：判定（无）→ 摸牌（摸走新牌堆顶 2 张）→ 出牌。
    assert_eq!(st.turn, owner);
    assert_eq!(st.turn_phase, TurnPhase::Play);
    assert_eq!(
        st.seats[owner].hand,
        vec![order[0].clone(), order[1].clone()],
        "摸牌阶段应摸走新牌堆顶 2 张"
    );
    // draw_before 是取出观星 5 张之后的牌堆数：先回堆 5 张，再被摸牌阶段摸走 2 张。
    assert_eq!(st.deck.draw.len(), draw_before + 3);
    assert_eq!(st.deck.discard.len(), discard_before);
    // 剩余牌堆的抽取顺序（pop）= 提交顺序（去掉已被摸走的 2 张）。
    let mut probe = st.clone();
    let mut popped = Vec::new();
    while let Some(id) = probe.deck.draw.pop() {
        popped.push(id);
        if popped.len() == 3 {
            break;
        }
    }
    assert_eq!(popped, order[2..].to_vec());
    // 重放被拒（此时已无 pending / 已进入出牌阶段）。
    assert!(mgr
        .execute_action(&room, &owner_pid, &format!("arrange:{}", order.join(",")))
        .is_err());
    st.check_conservation().unwrap();
}

#[test]
fn guanxing_settle_compacts_each_column() {
    let slots = vec![
        ("L1".to_string(), Some("c1".to_string())),
        ("L2".to_string(), None),
        ("L3".to_string(), Some("c3".to_string())),
        ("L4".to_string(), None),
        ("L5".to_string(), None),
        ("R1".to_string(), None),
        ("R2".to_string(), Some("c2".to_string())),
        ("R3".to_string(), None),
        ("R4".to_string(), None),
        ("R5".to_string(), None),
    ];
    assert_eq!(
        settle_slots(&slots),
        vec!["c1".to_string(), "c3".to_string(), "c2".to_string()]
    );
}

#[test]
fn guanxing_zone_is_cleared_when_turn_ends() {
    let mgr = manager(74);
    let room = start_and_pick(&mgr, 5);
    let (_owner, owner_pid) = enter_guanxing(&mgr, &room);
    resolve_arrange(&mgr, &room);
    let st = state_of(&mgr, &room);
    assert!(st.special_zones.get("guanxing").is_none());
    assert!(st.pending.is_none());
    // 提交后牌回牌堆、牌数守恒（观星不再残留处理区）。
    assert!(st.deck.processing.is_empty());
    st.check_conservation().unwrap();
    let _ = owner_pid;
}

#[test]
fn wugu_not_in_play_actions_before_pick() {
    // 五谷丰登已实现：手中有牌时应出现在合法动作里（P2.3 的「不可打出」限制解除）。
    let mgr = manager(75);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wugu_fengdeng");
    });
    let st = state_of(&mgr, &room);
    let cur_pid = pid(&st, st.turn);
    let wugu = in_hand(&st, st.turn, "wugu_fengdeng");
    let legal = mgr.legal_actions(&room, &cur_pid).unwrap();
    let ids: Vec<String> = legal.into_iter().map(|a| a.id).collect();
    assert!(
        ids.contains(&format!("play:{wugu}")),
        "五谷丰登应在合法动作中：{ids:?}"
    );
    // view 里五谷特殊区在结算前不存在。
    let view = public_view(&mgr, &room);
    assert!(view["special_zones"].get(WUGU_ZONE).is_none());
    let _: Value = view;
}

// ---------------------------------------------------------------------------
// 响应询问口径（2026-10-28 用户裁定）：无手牌 → 自动跳过；有手牌 → 即使没有对应牌也显式询问
// ---------------------------------------------------------------------------

#[test]
fn empty_hand_target_is_not_asked_for_shan() {
    let mgr = manager(76);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != cur && st.in_attack_range(cur, *i))
        .expect("应有攻击范围内的目标");
    let target_pid = pid(&st, target);
    let hp_before = st.seats[target].hp;
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none(), "无手牌目标不应被询问【闪】");
    assert_eq!(st.seats[target].hp, hp_before - 1, "应直接结算伤害");
    st.check_conservation().unwrap();
}

#[test]
fn hand_without_matching_card_still_gets_prompt() {
    let mgr = manager(77);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        // 用引擎的目标候选（已排除空城等锁定技），避免选中不可指定目标。
        let target = crate::turn::sha_targets(st, cur)
            .into_iter()
            .next()
            .expect("目标");
        // 有手牌但没有【闪】→ 仍要被询问（保留犹豫博弈），只有「不出」。
        give(st, target, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = st
        .alive_indices()
        .into_iter()
        .find(|i| *i != cur && st.in_attack_range(cur, *i))
        .expect("目标");
    let target_pid = pid(&st, target);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("有手牌应被询问");
    assert_eq!(pending.kind, "respond");
    assert_eq!(pending.actor, target_pid);
    assert_eq!(
        pending
            .options
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>(),
        vec!["pass"],
        "没有【闪】时只有显式 pass"
    );
    mgr.execute_action(&room, &target_pid, "pass").unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(st.seats[target].hp, st.seats[target].max_hp - 1);
}

#[test]
fn wuxie_chain_skips_empty_hand_players() {
    let mgr = manager(78);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "wuzhong_shengyou");
        // 座位 1：有手牌但无无懈；座位 2：有无懈；座位 3/4：空手。
        let left = st.seats_from_left_of(cur);
        give(st, left[0], "sha");
        give(st, left[1], "wuxie_keji");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let left: Vec<usize> = st.seats_from_left_of(cur);
    let with_cards = pid(&st, left[0]);
    let with_wuxie = pid(&st, left[1]);
    let empty_pid = pid(&st, left[2]);
    let card = in_hand(&st, cur, "wuzhong_shengyou");

    mgr.execute_action(&room, &cur_pid, &format!("play:{card}"))
        .unwrap();

    let mut asked = Vec::new();
    for _ in 0..10 {
        let st = state_of(&mgr, &room);
        let Some(p) = st.pending.clone() else { break };
        if p.kind != "wuxie" {
            break;
        }
        asked.push(p.actor.clone());
        mgr.execute_action(&room, &p.actor, "pass").unwrap();
    }
    assert_eq!(
        asked,
        vec![with_cards, with_wuxie],
        "只询问有手牌的玩家（无手牌自动跳过，含使用者）"
    );
    assert!(!asked.contains(&empty_pid));
    let st = state_of(&mgr, &room);
    assert!(st.pending.is_none());
    assert_eq!(st.seats[cur].hand.len(), 2, "无中生有应摸 2 张");
    st.check_conservation().unwrap();
}

#[test]
fn dying_skips_empty_hand_players() {
    let mgr = manager(79);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        // 用引擎的目标候选（已排除空城等锁定技），避免选中不可指定目标。
        let target = crate::turn::sha_targets(st, cur)
            .into_iter()
            .next()
            .expect("目标");
        st.seats[target].hp = 1;
        // 只有「左手侧最后一位」持桃：它前面的空手玩家应被跳过。
        let healer = st
            .seats_from_left_of(cur)
            .into_iter()
            .filter(|i| *i != target)
            .last()
            .expect("持桃者");
        give(st, healer, "tao");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let target = (0..st.seats.len())
        .find(|i| *i != cur && st.seats[*i].hp == 1)
        .expect("濒死目标");
    let target_pid = pid(&st, target);
    let healer = (0..st.seats.len())
        .find(|i| *i != cur && *i != target && !st.seats[*i].hand.is_empty())
        .expect("持桃者");
    let healer_pid = pid(&st, healer);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{target_pid}"))
        .unwrap();

    let st = state_of(&mgr, &room);
    let pending = st.pending.clone().expect("应有求桃询问");
    assert_eq!(pending.kind, "dying");
    assert_eq!(pending.actor, healer_pid, "空手玩家应被跳过，直接问持桃者");
    let tao = pending
        .options
        .iter()
        .find(|o| o.id.starts_with("respond:"))
        .expect("应有出桃选项")
        .id
        .clone();
    mgr.execute_action(&room, &healer_pid, &tao).unwrap();
    let st = state_of(&mgr, &room);
    assert!(st.seats[target].alive);
    assert_eq!(st.seats[target].hp, 1);
    st.check_conservation().unwrap();
}

#[test]
fn lord_skill_not_offered_when_allies_have_no_cards() {
    let mgr = manager(80);
    let room = start_and_pick(&mgr, 5);
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        give(st, cur, "sha");
        // 目标 = 刘备主公（有激将），手里只有一张无关牌（没有【闪】）
        let lord = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur && st.in_attack_range(cur, *i))
            .expect("目标");
        st.seats[lord].identity = Identity::Lord;
        set_general(st, lord, "sgs_hero_1");
        give(st, lord, "tao");
        // 蜀势力盟友存在但空手 → 不应提供「激将」（无牌可代出）
        let ally = st
            .alive_indices()
            .into_iter()
            .find(|i| *i != cur && *i != lord)
            .expect("盟友");
        set_general(st, ally, "sgs_hero_2");
    });
    let st = state_of(&mgr, &room);
    let cur = st.turn;
    let cur_pid = pid(&st, cur);
    let lord = (0..st.seats.len())
        .find(|i| *i != cur && st.seats[*i].identity.is_lord())
        .expect("主公");
    let lord_pid = pid(&st, lord);
    let sha = in_hand(&st, cur, "sha");

    mgr.execute_action(&room, &cur_pid, &format!("play:{sha}"))
        .unwrap();
    mgr.execute_action(&room, &cur_pid, &format!("target:{lord_pid}"))
        .unwrap();
    let pending = state_of(&mgr, &room)
        .pending
        .clone()
        .expect("有手牌应被询问");
    assert_eq!(pending.kind, "respond");
    assert!(
        !pending.options.iter().any(|o| o.id == "skill:jijiang"),
        "空手盟友不应触发激将：{:?}",
        pending.options
    );
    st.check_conservation().unwrap();
}
