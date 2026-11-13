//! 局面评估：座位公共资源价值、威胁压力、盟友救援优先级。
//!
//! 口径（可解释，便于调参与复盘）：
//! - `resource_value` = 手牌 × 0.6 + 装备 × 1.5 + 体力 × 1.0 + 标记 × 0.5（公共信息）
//! - `enemy_pressure` = `enemy_score` × `resource_value` + 残血加成（越残越值得补刀）
//! - `ally_need`     = `friendly_score` × (1 − health)（越残的盟友越值得救）

use super::threat::ThreatModel;
use super::view::{SgsView, SeatView};

/// 评估器：视图 + 敌友先验。
#[derive(Debug)]
pub struct Evaluator<'a> {
    pub view: &'a SgsView,
    pub threat: ThreatModel,
}

impl<'a> Evaluator<'a> {
    pub fn new(view: &'a SgsView) -> Self {
        let threat = ThreatModel::build(view);
        Self { view, threat }
    }

    pub fn me(&self) -> &SeatView {
        self.view.me_seat()
    }

    /// 座位公开资源价值。
    pub fn resource_value(&self, seat: usize) -> f64 {
        let Some(s) = self.view.seats.get(seat) else {
            return 0.0;
        };
        let marks: u32 = s.marks_count.values().sum();
        s.hand_count as f64 * 0.6
            + s.equips.len() as f64 * 1.5
            + s.hp.max(0) as f64
            + marks as f64 * 0.5
    }

    /// 对某座位出手的收益（含补刀加成）。
    pub fn enemy_pressure(&self, seat: usize) -> f64 {
        let Some(s) = self.view.seats.get(seat) else {
            return 0.0;
        };
        if !s.alive || seat == self.view.me {
            return 0.0;
        }
        let kill_bonus = 1.0 + (1.0 - s.health()) * 1.5;
        self.threat.enemy_score(seat) * self.resource_value(seat) * kill_bonus
    }

    /// 最值得出手的目标（未过门槛返回 `None`，即「不主动惹未知身份」）。
    pub fn best_enemy(&self) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for s in &self.view.seats {
            if !s.alive || s.index == self.view.me {
                continue;
            }
            if !self.threat.is_attackable(self.view, s) {
                continue;
            }
            let p = self.enemy_pressure(s.index);
            match best {
                Some((_, bp)) if bp >= p => {}
                _ => best = Some((s.index, p)),
            }
        }
        best.map(|(i, _)| i)
    }

    /// 有明确敌人可打时，其压力值（无则 0）。
    pub fn best_enemy_pressure(&self) -> f64 {
        self.best_enemy()
            .map(|i| self.enemy_pressure(i))
            .unwrap_or(0.0)
    }

    /// 最需要救援的盟友（残血且是友方）。
    pub fn ally_in_need(&self) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for s in &self.view.seats {
            if !s.alive || !self.threat.is_ally(s.index) {
                continue;
            }
            let need = self.threat.friendly_score(s.index) * (1.0 - s.health());
            match best {
                Some((_, bn)) if bn >= need => {}
                _ => best = Some((s.index, need)),
            }
        }
        best.map(|(i, _)| i)
    }

    /// 本人危险度 0..1（残血 + 手牌少）。
    pub fn self_danger(&self) -> f64 {
        let me = self.me();
        let health = me.health();
        let shield = (me.hand_count as f64 / 4.0).min(1.0);
        ((1.0 - health) * (1.0 - shield * 0.5)).clamp(0.0, 1.0)
    }

    /// 手牌里某牌型的数量。
    pub fn hand_of(&self, pred: impl Fn(&str) -> bool) -> usize {
        self.view.hand_count_of(pred)
    }

    /// 存活敌人数（`enemy_score > 门槛`）。
    pub fn enemy_count(&self) -> usize {
        self.view
            .seats
            .iter()
            .filter(|s| s.alive && self.threat.enemy_score(s.index) > super::threat::ATTACK_THRESHOLD)
            .count()
    }

    /// 存活盟友数。
    pub fn ally_count(&self) -> usize {
        self.view
            .seats
            .iter()
            .filter(|s| s.alive && self.threat.is_ally(s.index))
            .count()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn resource_value_monotone() {
        // 纯函数口径：这里只验证公式本身，不构造真实局面（真实局面见 ai::tests）。
        let f = |hand: f64, equip: f64, hp: f64, marks: f64| {
            hand * 0.6 + equip * 1.5 + hp + marks * 0.5
        };
        assert!(f(4.0, 0.0, 4.0, 0.0) > f(1.0, 0.0, 1.0, 0.0));
    }
}
