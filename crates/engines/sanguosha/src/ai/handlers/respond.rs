//! 响应类打分：`respond:<card>` / `pass`（濒死求桃、杀闪、无懈、改判）。

use crate::ai::card;
use crate::ai::eval::Evaluator;
use crate::ai::score::{prio, Score};

fn card_code(ev: &Evaluator, card_id: &str) -> String {
    if let Some(c) = ev.view.hand.iter().find(|c| c.id == card_id) {
        return c.code.clone();
    }
    ev.view
        .known_cards
        .get(card_id)
        .map(|c| c.code.clone())
        .unwrap_or_default()
}

fn victim_is_me(ev: &Evaluator) -> bool {
    ev.view
        .pending
        .as_ref()
        .and_then(|p| p.data.get("victim"))
        .and_then(|v| v.as_str())
        .map(|v| v == ev.view.me_id)
        .unwrap_or(false)
}

fn victim_seat(ev: &Evaluator) -> Option<usize> {
    let pid = ev
        .view
        .pending
        .as_ref()?
        .data
        .get("victim")?
        .as_str()?;
    ev.view.seat_of(pid).map(|s| s.index)
}

/// `pass`（放弃响应 / 放弃可选动作）。
pub fn score_pass(ev: &Evaluator) -> Score {
    match ev.view.pending_kind() {
        "dying" => {
            if victim_is_me(ev) {
                // 绝不放弃自己：给最低优先级，让每个可用桃都压过它。
                Score::new(prio::IDLE, 0.0)
            } else {
                // 救不救未知/敌人：先放弃（救人成本高）。
                Score::new(prio::IDLE, 30.0)
            }
        }
        // 无懈/改判：默认不浪费牌。
        "wuxie" | "judge_rewrite" => Score::new(prio::IDLE, 12.0),
        // 需要出闪/杀：若手上有对应牌会被更高分覆盖，否则放弃。
        "respond" => Score::new(prio::IDLE, 8.0),
        _ => Score::new(prio::IDLE, 1.0),
    }
}

/// `respond:<card_id>` 打分。
pub fn score_respond(ev: &Evaluator, kind: &str, card_id: &str) -> Score {
    // 过期选项过滤：选项里的牌必须真的在本人可见牌里（手牌/装备/已知）。
    for id in card_id.split(',') {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let visible = ev.view.hand.iter().any(|c| c.id == id)
            || ev.view.equips.iter().any(|c| c.id == id)
            || ev.view.known_cards.contains_key(id);
        if !visible {
            return Score::new(prio::IDLE, 0.0);
        }
    }
    let code = card_code(ev, card_id);
    match kind {
        "dying" => {
            if victim_is_me(ev) {
                let value = if card::is_peach(&code) { 60.0 } else { 30.0 };
                return Score::new(prio::MUST, value);
            }
            let ally_need = victim_seat(ev)
                .map(|i| {
                    if ev.threat.is_ally(i) {
                        ev.threat.friendly_score(i) * 60.0 - ev.self_danger() * 10.0
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);
            if ally_need > 0.0 {
                Score::new(prio::URGENT, ally_need)
            } else {
                // 未知身份/敌方的濒死：不救（低于 pass 的 30 分）。
                Score::new(prio::IDLE, 5.0)
            }
        }
        "respond" => {
            let me = ev.me();
            let danger = ev.self_danger();
            let base = if card::is_dodge(&code) {
                // 闪越少越舍不得，但残血必须出。
                6.0 + danger * 8.0
            } else if card::is_sha(&code) {
                // 需要出杀（南蛮/决斗/借刀）：有富余才出。
                let surplus = ev.hand_of(card::is_sha).saturating_sub(1) as f64;
                4.0 + surplus * 2.0 + danger * 4.0
            } else {
                3.0
            };
            let p = if me.health() <= 0.5 {
                prio::URGENT
            } else {
                prio::NORMAL
            };
            Score::new(p, base)
        }
        "wuxie" => {
            // 只有「冲着自己/明确盟友来的」才掏无懈。
            let root = ev
                .view
                .pending
                .as_ref()
                .and_then(|p| p.data.get("root"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let my_name = ev.me().general_name.clone();
            let mine = !my_name.is_empty() && root.contains(&my_name);
            let ally = ev
                .view
                .seats
                .iter()
                .any(|s| s.alive && ev.threat.is_ally(s.index) && !s.general_name.is_empty() && root.contains(&s.general_name));
            if mine || ally {
                Score::new(prio::URGENT, 40.0)
            } else {
                Score::new(prio::IDLE, 2.0)
            }
        }
        // 改判（鬼才/鬼道）：手牌换判定收益不确定，baseline 不用。
        "judge_rewrite" => Score::new(prio::IDLE, 1.0),
        // 技能帧里的 respond:（如 鬼才/流离的牌选择）。
        _ => Score::new(prio::NORMAL, 3.0),
    }
}
