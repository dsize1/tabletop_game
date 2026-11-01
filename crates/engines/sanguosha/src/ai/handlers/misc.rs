//! 技能 / 选将 / 大厅动作打分。

use crate::ai::eval::Evaluator;
use crate::ai::score::{prio, Score};
use crate::data;
use crate::state::Identity;

/// 本回合本人已「发动过该技能」的次数（按公开历史里的技能名匹配）。
///
/// 用途：防止「技能发动 → 目标放弃 → 技能仍合法 → 再发动」的空转
/// （自对弈已复现：誓战 / 激将 / 护驾 等需要他人响应的技能）。
fn used_this_turn(ev: &Evaluator, code: &str) -> usize {
    let Some(def) = crate::skills::def_of(code) else {
        return 0;
    };
    ev.view
        .logs
        .iter()
        .filter(|e| e.actor.as_deref() == Some(ev.view.me_id.as_str()))
        .filter(|e| e.text.contains(def.name))
        .count()
}

/// `skill:<code>[:arg]` 打分。
pub fn score_skill(ev: &Evaluator, rest: &str) -> Score {
    let code = rest.split(':').next().unwrap_or(rest);
    // 重复守卫：同一技能已在本局历史里被本人发动 ≥2 次 → 视为空转，不再发动。
    // （引擎侧「发动失败后仍保持合法」的问题待修，见 docs/games/sanguosha/ai.md）
    if used_this_turn(ev, code) >= 2 {
        return Score::new(prio::IDLE, 0.1);
    }
    let hurt_ally = ev.ally_in_need().is_some();
    match code {
        // 制衡：弃牌换牌，手牌多时很值。
        "zhiheng" => Score::new(prio::NORMAL, 4.0 + ev.me().hand_count as f64 * 0.3),
        // 苦肉：掉血摸牌；残血时不干。
        "kurou" => {
            if ev.me().hp > 2 {
                Score::new(prio::NORMAL, 3.5)
            } else {
                Score::new(prio::IDLE, 0.2)
            }
        }
        // 青囊/结姻：给残血盟友回血。
        "qingnang" | "jieyin" => {
            if hurt_ally {
                Score::new(prio::URGENT, 10.0)
            } else {
                Score::new(prio::IDLE, 0.5)
            }
        }
        // 看牌类：低成本信息收益。
        "gongxin" | "tuxi" | "qiaobian" | "guose" => Score::new(prio::NORMAL, 4.0),
        // 主公技接力（激将/护驾）：**baseline 不发动**。
        //
        // 已知引擎缺陷（P6 待修，见 docs/games/sanguosha/ai.md「已知问题」）：
        // 全体盟友放弃后，该选项会被再次下发，形成「发动→放弃→再发动」空转；
        // 自对弈已复现（seed=7，刘备+蜀势力）。修好之前 AI 一律不用，避免死循环。
        "jijiang" | "hujia" => Score::new(prio::IDLE, 0.2),
        // 觉醒/限定技：收益高，直接发。
        "juexing" | "wusheng" | "qixi" => Score::new(prio::NORMAL, 5.0),
        _ => Score::new(prio::NORMAL, 3.0),
    }
}

/// `setup:choose_general:<code>` 打分（按身份倾向）。
///
/// - 主公/忠臣：偏体力与辅助（活得久才护得住）；
/// - 反贼：偏体力与技能数量（爆发/输出）；
/// - 内奸：偏体力与单挑能力（同样看体力）。
pub fn score_general(ev: &Evaluator, code: &str) -> Score {
    let Some(g) = data::general(code) else {
        return Score::neutral();
    };
    let mut v = g.max_hp as f64 * 2.0 + g.skills.len() as f64 * 1.5;
    // 主公：同势力主公技更配（刘备/曹操/孙权/袁绍之类）。
    if ev.view.identity == Identity::Lord {
        let my_kingdom = ev
            .view
            .general
            .as_deref()
            .and_then(data::general)
            .map(|x| x.kingdom);
        if my_kingdom.is_some() && Some(g.kingdom) == my_kingdom {
            v += 1.5;
        }
        if g.is_lord {
            v += 1.0;
        }
    }
    // 反贼优先能打的主公将之外的输出位（体力高、技能多）。
    if ev.view.identity == Identity::Rebel {
        v += g.skills.len() as f64 * 0.5;
    }
    Score::new(prio::NORMAL, v)
}

/// 大厅动作打分（自对弈驱动 / 机器人房主用）。
pub fn score_lobby(id: &str) -> Score {
    match id {
        "lobby:start" => Score::new(prio::COMMIT, 50.0),
        "lobby:join" => Score::new(prio::COMMIT, 50.0),
        "lobby:add_bot" => Score::new(prio::NORMAL, 3.0),
        _ if id.starts_with("lobby:set_") => Score::new(prio::NORMAL, 1.0),
        _ => Score::neutral(),
    }
}
