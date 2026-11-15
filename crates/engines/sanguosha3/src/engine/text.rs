//! 事件 → 出牌历史文案（Web 侧栏 `history[].text`）。
//!
//! 只读 `Game.log` 的事件快照 + 牌/座位名，不做业务判定；空串 = 该事件不上历史。

use crate::kernel::event::{Event, EventKind};
use crate::kernel::{cards, Game, Phase};

pub fn seat_name(g: &Game, seat: usize) -> String {
    g.seats
        .get(seat)
        .map(|s| s.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("座位{}", seat + 1))
}

pub fn card_name(g: &Game, id: &str) -> String {
    g.card_defs
        .get(id)
        .and_then(|c| cards::def(&c.def))
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn phase_label(p: Phase) -> &'static str {
    match p {
        Phase::Prepare => "准备",
        Phase::Judge => "判定",
        Phase::Draw => "摸牌",
        Phase::Play => "出牌",
        Phase::Discard => "弃牌",
        Phase::End => "结束",
    }
}

/// 单条事件文案（`""` = 不展示）。
pub fn text_of(g: &Game, ev: &Event) -> String {
    let who = ev.seat.map(|s| seat_name(g, s));
    let who = who.unwrap_or_default();
    let card = || {
        ev.card
            .as_deref()
            .map(|id| card_name(g, id))
            .unwrap_or_default()
    };
    match ev.kind {
        EventKind::TurnStart => {
            if ev.cause.as_deref() == Some("翻面跳过回合") {
                format!("{who} 翻面，跳过本回合")
            } else {
                format!("轮到 {who}")
            }
        }
        EventKind::TurnEnd => String::new(),
        EventKind::PhaseStart => format!("{who} 的{}阶段", phase_label(ev.phase)),
        EventKind::PhaseEnd => String::new(),
        EventKind::CardDrawn => format!("{who} 摸了一张牌"),
        EventKind::CardUsed => format!("{who} 使用【{}】", card()),
        EventKind::CardPlayed => format!("{who} 打出【{}】", card()),
        EventKind::CardDiscarded => format!("{who} 弃置【{}】", card()),
        EventKind::CardGained => format!("{who} 获得【{}】", card()),
        EventKind::CardLost => String::new(),
        EventKind::CardIntoGame => format!("{who} 装备【{}】", card()),
        EventKind::CardOutOfGame => format!("{who} 的【{}】移出游戏", card()),
        EventKind::CardRevealed => format!("{who} 亮出【{}】", card()),
        EventKind::CardConcealed => String::new(),
        EventKind::TargetChosen => String::new(),
        EventKind::DamageAfter => format!("{who} 受到 {} 点伤害", ev.amount.unwrap_or(0)),
        EventKind::HpLost => format!("{who} 失去 {} 点体力", ev.amount.unwrap_or(0)),
        EventKind::Healed => {
            let n = ev.amount.unwrap_or(0);
            if n > 0 {
                format!("{who} 回复 {n} 点体力")
            } else {
                String::new()
            }
        }
        EventKind::MaxHpChanged => format!("{who} 体力上限变化 {}", ev.amount.unwrap_or(0)),
        EventKind::ChainedEnter => format!("{who} 进入连环状态"),
        EventKind::ChainedLeft => format!("{who} 解除连环状态"),
        EventKind::JudgeRevealed => format!("{who} 判定【{}】", card()),
        EventKind::JudgeEffective => String::new(),
        EventKind::JudgeZoneEntered => format!("{who} 的判定区置入【{}】", card()),
        EventKind::JudgeZoneLeft => format!("{who} 的判定区移出【{}】", card()),
        EventKind::ArmorChanged => format!("{who} 更换防具【{}】", card()),
        EventKind::Death => format!("{who} 阵亡"),
        EventKind::HandFrozen => String::new(),
        EventKind::HandUnfrozen => String::new(),
    }
}

/// 历史条目（`(seq, text)`）；空文案过滤。`seq` 单调（日志封顶不回绕）。
pub fn history(g: &Game) -> Vec<(u64, String)> {
    g.log
        .iter()
        .filter_map(|ev| {
            let text = text_of(g, ev);
            if text.is_empty() {
                None
            } else {
                Some((ev.seq, text))
            }
        })
        .collect()
}
