//! 威胁 / 敌友推断模型（P6.1 先验版）。
//!
//! **只用公开信息 + 本人身份**：主公身份公开、死亡角色身份公开、其余隐藏。
//! 因此 P6.1 的先验是：
//!
//! | 本人身份 | 主公 | 其他未知 |
//! |---|---|---|
//! | 主公 / 忠臣 | 友（-1） | 未知（0） |
//! | 反贼 | 敌（+1） | 未知（0） |
//! | 内奸 | 轻敌（+0.15，先削弱不打死） | 未知（0） |
//!
//! `enemy_score` 把嫌疑映射到「伤害他的期望收益」0..1：未知 = 0.5（打他一半概率打错），
//! 未超过 `ATTACK_THRESHOLD` 时 AI **不主动出手**（避免明显误伤盟友）。
//!
//! P6.5 在此之上接入结构化事件（谁打了主公 / 谁救了谁）做贝叶斯后验。

use super::view::{SgsView, SeatView};
use crate::state::Identity;

/// 主动进攻门槛：`enemy_score` 超过它才值得出手（未知身份 = 0.5 < 0.6）。
pub const ATTACK_THRESHOLD: f64 = 0.6;

/// 敌友嫌疑模型（每个座位 -1 友 … +1 敌）。
#[derive(Debug, Clone)]
pub struct ThreatModel {
    suspicion: Vec<f64>,
}

impl ThreatModel {
    /// 仅凭公开信息构造先验 + 用公开观测（伤害记录）做一步后验更新。
    pub fn build(view: &SgsView) -> Self {
        let n = view.seats.len();
        let mut suspicion = vec![0.0f64; n];
        for (i, seat) in view.seats.iter().enumerate() {
            if i == view.me {
                suspicion[i] = 0.0;
                continue;
            }
            let is_lord = seat.identity == Some(Identity::Lord);
            if !is_lord {
                continue;
            }
            suspicion[i] = match view.identity {
                Identity::Lord | Identity::Loyalist => -1.0,
                Identity::Rebel => 1.0,
                Identity::Spy => 0.15,
            };
        }
        let mut model = Self { suspicion };
        model.apply_observations(view);
        model
    }

    /// 用公开观测（`damage`）更新嫌疑：**只上调、不下调**（避免抵消主公锚点）。
    ///
    /// 信号强度：
    /// - 打主公 +0.45/点（最强的「反贼」信号）；
    /// - 打自己 +0.25/点；
    /// - 主公打他 +0.30/次（主公的公开敌意）。
    fn apply_observations(&mut self, view: &SgsView) {
        let lord = view
            .seats
            .iter()
            .find(|s| s.identity == Some(Identity::Lord))
            .map(|s| s.index);
        for obs in &view.observations {
            if obs.kind != "damage" {
                continue;
            }
            let (Some(actor), Some(target)) = (obs.actor.as_deref(), obs.target.as_deref()) else {
                continue;
            };
            let (Some(a_idx), Some(t_idx)) = (
                view.seat_of(actor).map(|s| s.index),
                view.seat_of(target).map(|s| s.index),
            ) else {
                continue;
            };
            if a_idx == t_idx || a_idx == self_suspicion_owner(view) {
                continue;
            }
            let amount = obs.amount.max(1) as f64;
            if lord == Some(t_idx) {
                self.bump(a_idx, 0.45 * amount);
            }
            if t_idx == view.me {
                self.bump(a_idx, 0.25 * amount);
            }
            if lord == Some(a_idx) && lord != Some(t_idx) {
                self.bump(t_idx, 0.30);
            }
        }
    }

    fn bump(&mut self, seat: usize, delta: f64) {
        if let Some(v) = self.suspicion.get_mut(seat) {
            *v = (*v + delta).clamp(-1.0, 1.0);
        }
    }

    /// 嫌疑值（-1 友 … +1 敌）。
    pub fn suspicion(&self, seat: usize) -> f64 {
        self.suspicion.get(seat).copied().unwrap_or(0.0)
    }

    /// 伤害他 / 对他用负面锦囊的期望收益（0..1）。
    pub fn enemy_score(&self, seat: usize) -> f64 {
        ((self.suspicion(seat) + 1.0) / 2.0).clamp(0.0, 1.0)
    }

    /// 帮助他的倾向（0..1；1 = 确定友）。
    pub fn friendly_score(&self, seat: usize) -> f64 {
        (1.0 - self.enemy_score(seat)).clamp(0.0, 1.0)
    }

    /// 是否值得主动进攻（未知身份默认否）。
    pub fn is_attackable(&self, view: &SgsView, seat: &SeatView) -> bool {
        view.seats[seat.index].alive
            && seat.index != view.me
            && self.enemy_score(seat.index) > ATTACK_THRESHOLD
    }

    /// 是否值得治疗 / 给牌（盟友且残血）。
    pub fn is_ally(&self, seat: usize) -> bool {
        self.friendly_score(seat) > 0.6
    }
}

fn self_suspicion_owner(view: &SgsView) -> usize {
    view.me
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_neutral_and_below_threshold() {
        let m = ThreatModel {
            suspicion: vec![0.0, 1.0, -1.0],
        };
        assert!((m.enemy_score(0) - 0.5).abs() < 1e-9);
        assert!(m.enemy_score(0) <= ATTACK_THRESHOLD);
        assert!(m.enemy_score(1) > ATTACK_THRESHOLD);
        assert!(m.friendly_score(2) > 0.6);
    }
}
