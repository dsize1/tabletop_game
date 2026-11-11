//! 明任（卢植）：游戏开始时摸 1 张，并**必选**一张手牌作为「任」；结束阶段可一进一出替换。

use serde_json::json;

use crate::kernel::event::{EventKind, Window};
use crate::kernel::resolve::{Ask, Stage, Step};
use crate::kernel::{Game, Zone};
use crate::presentation::{POutcome, Presentation};

use super::{SkillDef, SkillKind};

pub const DEF: SkillDef = SkillDef { kind: SkillKind::Normal,
    code: "mingren",
    name: "明任",
    triggers: &[EventKind::PhaseStart],
    window: Window::OwnTurn,
    priority: 4,
    can_active: None,
    handler,
    dying_hook: None,
    flip_hook: None,
    wuxie_opts: None,
    wuxie_use: None,
    target_blocker: None,
    yang_window: None,
    yin_window: None,
    subject_window: None,
};

/// 开局：摸 1 张并进入「必选一张作为任」的询问（由 `Game::new` 驱动）。
pub fn start_init(g: &mut Game, seat: usize) {
    // 摸 1 延迟到首次询问时（保证调用方已布置好手牌/牌堆）。
    crate::kernel::resolve::push_ask(g, "mingren", Stage::Choose, seat, json!({ "init": true }));
}

fn handler(g: &mut Game, a: &Ask) -> Result<Step, String> {
    if a.data["init"].as_bool().unwrap_or(false) == false
        && a.data.get("event").is_some()
        && a.data["cause"].as_str() == Some("End")
    {
        if g.mark_cards(a.actor, "ren").is_empty() || g.hand(a.actor).is_empty() {
            return Ok(Step::Done);
        }
    }
    match a.stage {
        Stage::Choose => {
            // 初始化询问 或 结束阶段替换询问
            let init = a.data["init"].as_bool().unwrap_or(false);
            if init && g.counter("turn.mingren_drawn") == 0 {
                g.draw(a.actor, 1);
                g.bump("turn.mingren_drawn");
            }
            let hand = g.hand(a.actor);
            if hand.is_empty() {
                return Ok(Step::Done);
            }
            match g.answer.take().as_deref() {
                None => {
                    let mut b = super::builder(a.actor, "choice").skill("mingren");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("作为「任」：【{}】", g.card_label(id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不选（必须在下一步选一张）").build()))
                }
                Some("pass") => {
                    // 「必选」：继续问（不给放弃语义）
                    let mut b = super::builder(a.actor, "choice").skill("mingren");
                    for id in &hand {
                        b = b.option(
                            format!("choose:{id}"),
                            format!("必须选一张作为「任」：【{}】", g.card_label(id)),
                        );
                    }
                    Ok(Step::Need(b.escape("不选（必须选一张）").build()))
                }
                Some(act) => {
                    let id = act
                        .strip_prefix("choose:")
                        .ok_or_else(|| format!("明任：未知选项 {act}"))?
                        .to_string();
                    Ok(Step::Next(Stage::Target, json!({ "card": id, "init": init })))
                }
                Some(o) => Err(format!("明任：未知选项 {o}")),
            }
        }
        Stage::Target => {
            let id = a.data["card"].as_str().unwrap_or("").to_string();
            let init = a.data["init"].as_bool().unwrap_or(false);
            if init {
                g.to_mark(&id, a.actor, "ren")?;
                g.presentation.push(
                    Presentation::new("mingren", POutcome::Activated { skill: "明任".into() })
                        .actor(a.actor)
                        .cards(vec![id])
                        .fx("mingren_init"),
                );
                return Ok(Step::Done);
            }
            // 结束阶段替换：一进一出（旧「任」回手 → 新的置为「任」）
            let old = g.mark_cards(a.actor, "ren").into_iter().next();
            if let Some(old) = old {
                g.to_hand(&old, a.actor)?;
            }
            g.to_mark(&id, a.actor, "ren")?;
            g.presentation.push(
                Presentation::new("mingren", POutcome::Activated { skill: "明任".into() })
                    .actor(a.actor)
                    .cards(vec![id])
                    .fx("mingren_swap"),
            );
            Ok(Step::Done)
        }
        _ => Err("明任：未知阶段".into()),
    }
}

/// 结束阶段：询问是否替换「任」（一进一出）。
pub fn end_phase_ask(g: &mut Game, seat: usize) -> bool {
    if g.mark_cards(seat, "ren").is_empty() || g.hand(seat).is_empty() {
        return false;
    }
    crate::kernel::resolve::push_ask(g, "mingren", Stage::Choose, seat, json!({}));
    true
}

#[allow(dead_code)]
fn _unused(_g: &mut Game) -> bool {
    let _ = Zone::Discard;
    false
}
