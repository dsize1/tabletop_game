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
        let mut seat = json!({
            "id": i,
            "hp": s.hp,
            "max_hp": s.max_hp,
            "alive": s.alive,
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
    let pending = g.pending.as_ref().map(|p| {
        json!({
            "kind": p.kind(),
            "actor": p.actor(),
            "skill": p.skill(),
        })
    });

    json!({
        "turn": g.turn,
        "phase": format!("{:?}", g.phase),
        "round": g.round,
        "deck": deck,
        "seats": seats,
        "pending": pending,
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
