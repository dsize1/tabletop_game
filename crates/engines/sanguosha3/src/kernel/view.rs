//! 视图投影（constitution I3：**视图是投影，不是判定**）。
//!
//! 不变量：
//! 1. 视图只做字段投影与裁剪，不写业务规则（身份公开/存活/响应由引擎状态决定）；
//! 2. 隐藏信息不泄露：他人手牌明细、牌堆顺序、他人身份、私密展示、公共 pending 的 options/data；
//! 3. 9 类区投影：公共区（Draw/Discard/Void/Processing）只给数量，Hand 只在个人视图，
//!    Equip/Judge/Mark 公共可见（牌面公开），Frame 只给数量。

use serde_json::{json, Value};

use super::cards;
use super::identity;
use super::{Game, Zone};

/// 牌名（显示名优先；牌在牌堆/手牌时按持有者可见性决定是否给出）。
fn card_label(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// 公共视图（脱敏后给 `get_state`）。
pub fn public_state(g: &Game) -> Value {
    let mut seats = Vec::new();
    for (i, s) in g.seats.iter().enumerate() {
        // 手牌数是公开信息（只有手牌**明细**是隐藏信息）；翻面/横置同样公开。
        let hand = g
            .cards
            .values()
            .filter(|z| matches!(z, Zone::Hand(s0) if *s0 == i))
            .count();
        let mut seat = json!({
            "id": i,
            "hp": s.hp,
            "max_hp": s.max_hp,
            "alive": s.alive,
            "hand": hand,
            "hand_limit": g.hand_limit(i),
            "flipped": s.flipped,
            "chained": s.chained,
            "kingdom": s.kingdom,
            // 护甲（mechanics `armor-points`）：独立防护值，公开可见（否则无法判断剩余体力）。
            "armor": s.armor,
            // 废除装备栏 / 判定区（雄乱/决堰/破势）：公开事实。
            "abolished_slots": s.abolished_slots.iter().map(|x| format!("{x:?}")).collect::<Vec<_>>(),
            "judge_abolished": s.judge_abolished,
            // 手牌冻结数（明细仍隐藏；只给“有几张不能用”）。
            "frozen": s.frozen.len() + s.frozen_phase.len(),
        });
        // 身份只在公开时给（主公 / 已死亡）。
        if identity::identity_public(s) {
            seat["identity"] = json!(format!("{:?}", s.identity));
        }
        // 装备/判定/标记公共可见（牌面公开）。
        let equips: Vec<String> = g
            .cards
            .iter()
            .filter(|(_, z)| matches!(z, Zone::Equip(s0, _) if *s0 == i))
            .map(|(id, _)| card_label(g, id))
            .collect();
        let judge: Vec<String> = g
            .cards
            .iter()
            .filter(|(_, z)| matches!(z, Zone::Judge(s0) if *s0 == i))
            .map(|(id, _)| card_label(g, id))
            .collect();
        seat["equips"] = json!(equips);
        seat["judge"] = json!(judge);
        seats.push(seat);
    }

    // 9 类区投影（公共侧：Draw/Discard/Void/Processing 数量；Hand 不出现）。
    let count = |z: &dyn Fn(&Zone) -> bool| g.cards.values().filter(|&zone| z(zone)).count();
    let deck = json!({
        "draw": count(&|z| matches!(z, Zone::Draw)),
        "discard": count(&|z| matches!(z, Zone::Discard)),
        "void": count(&|z| matches!(z, Zone::Void)),
        "processing": count(&|z| matches!(z, Zone::Processing)),
    });

    // 公共 pending 只给 { kind, actor, skill }，不给 options/data（脱敏）。
    // 公共可见集合（mechanics `reveal`）：展示中的牌牌面公开。
    let revealed: Vec<Value> = g
        .revealed
        .iter()
        .map(|id| json!({ "id": id, "name": card_label(g, id) }))
        .collect();
    // 判定反转目标（思蜀类）：公开可读，供玩家理解判定结果。
    let judge_reverse = g
        .judge_reverse
        .iter()
        .map(|(seat, code)| json!({ "seat": seat, "card": code }))
        .collect::<Vec<_>>();

    let pending = g.pending.as_ref().map(|p| {
        json!({
            "kind": p.kind(),
            "actor": p.actor(),
            "skill": p.skill(),
        })
    });

    json!({
        "started": g.started,
        "turn": g.turn,
        "phase": format!("{:?}", g.phase),
        "round": g.round,
        "turn_seat": g.turn,
        "deck": deck,
        "seats": seats,
        "pending": pending,
        "revealed": revealed,
        "judge_reverse": judge_reverse,
        "winner": g.winner.map(|w| format!("{:?}", w)),
    })
}

/// 个人视图（`player_view`）：本人私有信息 + 公共信息。
pub fn player_view(g: &Game, viewer: usize) -> Value {
    let mut v = public_state(g);
    // 本人手牌明细（仅本人可见）。
    let hand: Vec<String> = g
        .cards
        .iter()
        .filter(|(_, z)| matches!(z, Zone::Hand(s0) if *s0 == viewer))
        .map(|(id, _)| card_label(g, id))
        .collect();
    v["hand"] = json!(hand);
    // 本人身份（自己永远知道）。
    if let Some(s) = g.seats.get(viewer) {
        v["my_identity"] = json!(s.identity.map(|i| format!("{:?}", i)));
    }
    // 本人 pending 完整（options/data/escape）。
    if let Some(p) = &g.pending {
        if p.actor() == viewer {
            v["my_pending"] = json!({
                "kind": p.kind(),
                "options": p.options().iter().map(|o| json!({"id": o.id, "label": o.label})).collect::<Vec<_>>(),
                "data": p.data(),
                "escape": p.escape_id(),
            });
        }
    }
    v
}

/// 归一化摘要（差分对拍用）：座位（hp/alive/公开身份/手牌数）+ 区域计数 + 事件日志。
/// 屏蔽两引擎 schema 差异，是 v1↔v3 对拍的唯一比对口径（见 `v3-parity-baseline.md` §4.1）。
pub fn summary(g: &Game) -> Value {
    let seats: Vec<Value> = g
        .seats
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let hand = g.cards.values().filter(|z| matches!(z, Zone::Hand(s0) if *s0 == i)).count();
            json!({
                "hp": s.hp,
                "alive": s.alive,
                "identity": identity::identity_public(s).then(|| format!("{:?}", s.identity)),
                "hand": hand,
            })
        })
        .collect();
    let zone_count = |pred: &dyn Fn(&Zone) -> bool| g.cards.values().filter(|z| pred(z)).count();
    json!({
        "seats": seats,
        "zones": {
            "draw": zone_count(&|z| matches!(z, Zone::Draw)),
            "discard": zone_count(&|z| matches!(z, Zone::Discard)),
            "void": zone_count(&|z| matches!(z, Zone::Void)),
        },
    })
}
