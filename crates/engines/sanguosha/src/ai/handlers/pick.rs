//! 选牌类打分：`discard:` / `choose:` / `pick:` / `confirm`。

use crate::ai::card;
use crate::ai::eval::Evaluator;
use crate::ai::score::{prio, Score};

fn data_u64(ev: &Evaluator, key: &str) -> Option<u64> {
    ev.view.pending.as_ref()?.data.get(key)?.as_u64()
}

fn selected_ids(ev: &Evaluator) -> Vec<String> {
    ev.view
        .pending
        .as_ref()
        .and_then(|p| p.data.get("selected"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn selected_len(ev: &Evaluator) -> usize {
    selected_ids(ev).len()
}

/// 弃牌：优先丢价值最低的牌；残血时避免丢桃。
pub fn score_discard(ev: &Evaluator, card_id: &str) -> Score {
    let Some(c) = ev.view.hand.iter().find(|c| c.id == card_id) else {
        return Score::neutral();
    };
    let mut jank = 10.0 - c.value();
    if card::is_peach(&c.code) && ev.me().health() < 0.6 {
        jank -= 20.0;
    }
    if card::is_wuxie(&c.code) && ev.self_danger() > 0.3 {
        jank -= 4.0;
    }
    Score::new(prio::NORMAL, jank)
}

/// 多选提交：选够下限才确认。
pub fn score_confirm(ev: &Evaluator) -> Score {
    let kind = ev.view.pending_kind();
    if kind == "choose_many" {
        let min = data_u64(ev, "min").unwrap_or(0) as usize;
        if selected_len(ev) >= min.max(1) {
            Score::new(prio::COMMIT, 40.0)
        } else {
            Score::new(prio::IDLE, 0.0)
        }
    } else {
        Score::new(prio::COMMIT, 20.0)
    }
}

/// 选项关键词语义打分（引擎的 `choose:<code>` 都是 ascii 关键字）。
fn keyword_score(code: &str) -> f64 {
    let mut v = 1.0;
    for (key, add) in [
        ("draw", 6.0),
        ("gain", 4.0),
        ("heal", 8.0),
        ("recover", 8.0),
        ("exchange", 2.0),
        ("discard", -2.0),
        ("lose", -3.0),
        ("drop", -2.0),
        ("give", -1.0),
        ("hanbing", 3.0),
        ("question", 0.0),
    ] {
        if code.contains(key) {
            v += add;
        }
    }
    v
}

/// 按「选项文案」打分（未登记动作 id 的兜底，如技能帧 `opt1`/`opt2`）。
///
/// 只做语义关键词匹配：摸牌/回复/获得为正，弃牌/失去为负。
pub fn score_label(label: &str) -> Score {
    let mut v = 1.0;
    for (key, add) in [
        ("摸", 6.0),
        ("回复", 8.0),
        ("获得", 4.0),
        ("视为", 3.0),
        ("伤害", 2.0),
        ("弃", -2.0),
        ("失去", -3.0),
        ("翻面", -1.5),
        ("交给", -1.0),
    ] {
        if label.contains(key) {
            v += add;
        }
    }
    Score::new(prio::NORMAL, v)
}

/// `choose:<code>` 打分。
pub fn score_choose(ev: &Evaluator, kind: &str, code: &str) -> Score {
    // 已知引擎缺陷（P6 待修，见 docs/games/sanguosha/ai.md「已知问题」）：
    // 贯石斧「弃 2 张手牌」确认后，多选帧会残留旧 `selected`，再次 confirm 会被判非法
    // （自对弈已复现：`非法动作 confirm: 手牌中没有该牌`）。修好之前 AI 不发动。
    if code == "guanshi" {
        return Score::new(prio::IDLE, 0.2);
    }
    if kind == "choose_many" {
        let max = data_u64(ev, "max").unwrap_or(u64::MAX) as usize;
        if selected_len(ev) >= max {
            return Score::new(prio::IDLE, 0.0);
        }
        // 已选中的牌：不要「再点一次取消选择」——那会与原状态来回切换（自对弈已复现死循环）。
        if selected_ids(ev).iter().any(|x| x == code) {
            return Score::new(prio::IDLE, 0.0);
        }
        // 多选：尽量交出价值最低的牌。
        let value = ev
            .view
            .hand
            .iter()
            .find(|c| c.id == code || code.contains(&c.id))
            .map(|c| c.value())
            .unwrap_or(5.0);
        return Score::new(prio::NORMAL, 20.0 - value);
    }
    // `kind=choice` 的「多步选目标」帧（除疠/陷嗣等）也用 `data.selected` 暴露进度：
    // 已选项若再被选中等于取消，会让交互来回空转 → 直接降级。
    if selected_ids(ev).iter().any(|x| x == code) {
        return Score::new(prio::IDLE, 0.0);
    }
    Score::new(prio::NORMAL, keyword_score(code))
}

/// `pick:<...>` 打分（五谷选牌 / 拆牌 / 顺手 / 反馈等）。
pub fn score_pick(ev: &Evaluator, rest: &str) -> Score {
    if rest == "hand" {
        // 随机一张手牌：中等价值（拆对手手牌方差大但期望为正）。
        return Score::new(prio::NORMAL, 5.0);
    }
    if let Some(slot) = rest.strip_prefix("equip:") {
        // 拆/拿对手装备：武器最值，其次防具。
        let v = match slot {
            "weapon" => 6.0,
            "armor" => 5.0,
            "horse_offense" | "horse_defense" => 3.5,
            _ => 3.0,
        };
        return Score::new(prio::NORMAL, v);
    }
    if rest.starts_with("judge:") {
        // 拆掉对手判定区里的负面锦囊收益最大。
        return Score::new(prio::STRONG, 7.0);
    }
    // 五谷：拿价值最高的牌。
    let value = ev
        .view
        .known_cards
        .get(rest)
        .map(|c| c.value())
        .unwrap_or(4.0);
    Score::new(prio::NORMAL, value)
}
