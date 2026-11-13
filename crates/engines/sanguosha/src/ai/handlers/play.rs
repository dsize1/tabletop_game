//! 出牌阶段（`play:<card_id>`）打分。
//!
//! 核心口径：**没有明确敌人时不出攻击牌**（未知身份 = 不该主动惹），
//! 优先发展（摸牌/装备/回血/补牌），避免 baseline 乱杀盟友。

use crate::ai::card;
use crate::ai::eval::Evaluator;
use crate::ai::score::{prio, Score};

fn sha_score(ev: &Evaluator) -> Score {
    let pressure = ev.best_enemy_pressure();
    if pressure <= 0.0 {
        // 没有可打的明确敌人：不出杀（保留手牌防守）。
        return Score::new(prio::IDLE, 0.5);
    }
    // 对方残血时压力更大 → 更值得出杀。
    Score::new(prio::STRONG, 10.0 + pressure)
}

fn aoe_score(ev: &Evaluator) -> Score {
    let enemies = ev.enemy_count() as f64;
    let allies = ev.ally_count() as f64;
    if enemies <= 0.0 {
        return Score::new(prio::IDLE, 0.5);
    }
    let net = enemies - allies * 1.2;
    Score::new(prio::NORMAL, (3.0 + net * 2.0).max(0.2))
}

/// 装备牌打分：空槽优先，其次比当前装备更值钱。
fn equip_score(ev: &Evaluator, slot: crate::state::EquipSlot, value: f64) -> Score {
    let me = ev.me();
    match me.equip(slot) {
        None => Score::new(prio::NORMAL, 6.0),
        Some(cur) if value > cur.value() => Score::new(prio::NORMAL, 3.0),
        Some(_) => Score::new(prio::IDLE, 0.5),
    }
}

/// 出牌阶段单张牌打分。
pub fn score_play(ev: &Evaluator, card_id: &str) -> Score {
    let Some(c) = ev.view.hand.iter().find(|c| c.id == card_id) else {
        // 不在手牌（可能是技能指定的牌）：中性，别抢戏。
        return Score::neutral();
    };
    let code = c.code.as_str();
    let pressure = ev.best_enemy_pressure();

    if card::is_sha(code) {
        return sha_score(ev);
    }
    if let Some(slot) = c.equip_slot {
        return equip_score(ev, slot, c.value());
    }

    match code {
        // 回血：URGENT（出牌阶段能用桃说明自己已受伤）。
        "tao" => Score::new(prio::URGENT, 20.0),
        "jiu" => {
            if pressure > 0.0 && ev.hand_of(card::is_sha) > 0 {
                Score::new(prio::NORMAL, 4.0)
            } else {
                Score::new(prio::IDLE, 0.3)
            }
        }
        "wuzhong_shengyou" => Score::new(prio::NORMAL, 6.5),
        "shunshou_qianyang" => {
            if pressure > 0.0 {
                Score::new(prio::STRONG, 9.0)
            } else {
                Score::new(prio::IDLE, 0.4)
            }
        }
        "guohe_chaiqiao" => {
            if pressure > 0.0 {
                Score::new(prio::NORMAL, 7.0)
            } else {
                Score::new(prio::IDLE, 0.4)
            }
        }
        "juedou" => {
            if pressure > 0.0 && ev.hand_of(card::is_sha) >= 2 {
                Score::new(prio::STRONG, 8.0)
            } else {
                Score::new(prio::IDLE, 0.6)
            }
        }
        "huo_gong" => {
            if pressure > 0.0 {
                Score::new(prio::NORMAL, 7.0)
            } else {
                Score::new(prio::IDLE, 0.6)
            }
        }
        "lebu_sishu" | "bingliang_cunduan" => {
            if pressure > 0.0 {
                Score::new(prio::NORMAL, 7.0)
            } else {
                Score::new(prio::IDLE, 0.6)
            }
        }
        "shandian" => {
            // 自己也有风险，且不改局面；baseline 不打。
            Score::new(prio::IDLE, 0.3)
        }
        "tiesuo_lianhuan" => Score::new(prio::IDLE, 1.5),
        "wugu_fengdeng" => Score::new(prio::NORMAL, 5.0),
        "taoyuan_jieyi" => {
            let hurt_ally = ev.ally_in_need().is_some();
            if hurt_ally {
                Score::new(prio::URGENT, 9.0)
            } else {
                Score::new(prio::NORMAL, 2.0)
            }
        }
        c if card::is_aoe(c) => aoe_score(ev),
        _ => Score::new(prio::NORMAL, 3.0),
    }
}
