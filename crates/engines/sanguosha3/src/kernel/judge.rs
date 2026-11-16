//! 判定（**单一入口**）+ 判定牌去向。
//!
//! 不变量（mechanics `judge`）：
//! 1. 判定牌只能从牌堆顶翻出（`deck.pop()`），摸牌方向钩子（寸目）**不影响判定**；
//! 2. 判定牌公开即产生 `JudgeRevealed`，生效产生 `JudgeEffective`（颂威等订阅后者）；
//! 3. 判定结果只由 `JudgeResult`（花色 + 点数）表达，调用方不读牌名。

use super::event::{Event, EventKind};
use super::{CardId, Game, Suit, Zone};

/// 判定结果（花色 + 点数即可覆盖乐/兵/闪电/技能判定，不含牌名）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JudgeResult {
    pub card: CardId,
    pub suit: Suit,
    pub rank: u8,
}

impl JudgeResult {
    pub fn is_red(&self) -> bool {
        self.suit.is_red()
    }
    /// 是否黑色（无花色判定牌不存在；用 `is_black` 而非 `!is_red`，避免无花色被当成黑色）。
    pub fn is_black(&self) -> bool {
        self.suit.is_black()
    }
    pub fn is_heart(&self) -> bool {
        self.suit == Suit::Heart
    }
    pub fn is_diamond(&self) -> bool {
        self.suit == Suit::Diamond
    }
    pub fn is_club(&self) -> bool {
        self.suit == Suit::Club
    }
    pub fn is_spade(&self) -> bool {
        self.suit == Suit::Spade
    }
    /// 点数区间（闪电：黑桃 2–9）。
    pub fn rank_in(&self, lo: u8, hi: u8) -> bool {
        self.rank >= lo && self.rank <= hi
    }
}

/// 翻判定牌（**不发生效事件**；需要改判/天妒窗口时一律走 `quick_judge::start`）。
/// 牌堆顶 → 处理区，同时产生 `JudgeRevealed`。
pub fn reveal(g: &mut Game, seat: usize, cause: &str) -> Option<JudgeResult> {
    if g.deck.is_empty() {
        super::flow::reshuffle(g);
    }
    let id = g.deck.pop()?;
    let c = g.card_defs.get(&id)?.clone();
    g.cards.insert(id.clone(), Zone::Processing);
    let mut ev = Event::card(EventKind::JudgeRevealed, Some(seat), id.clone(), Some(cause.to_string()));
    ev.by = Some(seat);
    ev.turn = g.turn;
    ev.phase = g.phase;
    g.push_event(ev);
    Some(JudgeResult {
        card: id,
        suit: super::skills::suit_for_judge(g, seat, c.suit),
        rank: c.rank,
    })
}

/// 判定牌生效（发 `JudgeEffective`；牌仍在处理区，由调用方决定去向）。
pub fn effective(g: &mut Game, seat: usize, cause: &str, card: &CardId) -> Option<JudgeResult> {
    let c = g.card_defs.get(card)?.clone();
    let mut eff = Event::simple(EventKind::JudgeEffective, Some(seat), Some(cause));
    eff.turn = g.turn;
    eff.phase = g.phase;
    eff.card = Some(card.clone());
    g.push_event(eff);
    Some(JudgeResult {
        card: card.clone(),
        suit: super::skills::suit_for_judge(g, seat, c.suit),
        rank: c.rank,
    })
}

/// 同步判定（**无**改判/天妒窗口）：亮牌 → 生效 → 进弃牌堆。
///
/// 只允许「无玩家选择」的场景使用；所有技能判定/延时锦囊判定必须走 `quick_judge`。
pub fn judge(g: &mut Game, seat: usize, cause: &str) -> Option<JudgeResult> {
    let r = reveal(g, seat, cause)?;
    let r = effective(g, seat, cause, &r.card)?;
    super::card_play::to_discard_with(g, &r.card, "判定", None);
    Some(r)
}

/// 只翻不看：给需要「先展示、后改判」的窗口用（改判窗口的单一入口见 `quick_judge`）。
pub fn judge_peek(g: &Game) -> Option<CardId> {
    g.deck.last().cloned()
}
