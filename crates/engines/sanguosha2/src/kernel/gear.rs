//! 装备特效窗口（S2）：青龙偃月刀 / 贯石斧 / 寒冰剑 / 雌雄双股剑 / 麒麟弓。
//!
//! 设计：`FlowKind::Gear` 是**独立结算帧**，由 `card_play::sha_loop` 在四个时点压入并等待：
//! - `target_chosen`：指定目标后（雌雄双股剑：异性目标弃 1 张 / 令使用者摸 1 张）；
//! - `after_dodge`：目标打出【闪】抵消后（青龙偃月刀：再弃一张【杀】令其重新响应 /
//!   贯石斧：弃 2 张手牌强行造成伤害）；
//! - `before_damage`：造成伤害前（寒冰剑：防止伤害，改为弃置目标两张牌）；
//! - `after_damage`：造成伤害后（麒麟弓：弃置目标一匹坐骑）。
//!
//! 结果通过 [`GearResult`] 交回 `sha_loop`：`Retry` = 目标需重新响应【闪】、
//! `Prevented` = 本次伤害被防止、`Declined` = 继续正常结算。
//!
//! 无询问的装备（古锭刀 / 朱雀羽扇 / 方天画戟 / 诸葛连弩 / 青釭剑）在 `card_play` 的
//! 数值链里直接读 `cards::WeaponKind`。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::pending::Pending;
use super::resolve::{Ask, FlowFrame, FlowKind, Frame, Stage, Step};
use super::{CardId, Game, Slot};
use crate::cards::WeaponKind;
use crate::skills;

/// 装备窗口帧的 skill 名（内核专用；不注册为武将技能）。
pub const GEAR_SKILL: &str = "__gear";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GearTag {
    TargetChosen,
    AfterDodge,
    BeforeDamage,
    AfterDamage,
}

impl GearTag {
    fn key(self) -> &'static str {
        match self {
            GearTag::TargetChosen => "target_chosen",
            GearTag::AfterDodge => "after_dodge",
            GearTag::BeforeDamage => "before_damage",
            GearTag::AfterDamage => "after_damage",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "target_chosen" => Some(GearTag::TargetChosen),
            "after_dodge" => Some(GearTag::AfterDodge),
            "before_damage" => Some(GearTag::BeforeDamage),
            "after_damage" => Some(GearTag::AfterDamage),
            _ => None,
        }
    }
}

/// 窗口结果（交回 `sha_loop`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GearResult {
    Declined,
    /// 青龙：目标需重新响应【闪】。
    Retry,
    /// 贯石斧：弃置 2 张手牌后强行造成伤害。
    Accepted,
    /// 寒冰：本次伤害被防止。
    Prevented,
}

/// 该时点是否有需要询问的装备（避免无意义压帧）。
pub fn has_window(g: &Game, tag: GearTag, owner: usize, target: usize) -> bool {
    let weapon = g.weapon(owner);
    match tag {
        GearTag::TargetChosen => {
            weapon == WeaponKind::Cixiong && g.seats[owner].male != g.seats[target].male
        }
        GearTag::AfterDodge => match weapon {
            WeaponKind::Qinglong => g.hand(owner).iter().any(|id| g.def_of(id).tags.sha),
            WeaponKind::Guanshi => g.hand(owner).len() >= 2,
            _ => false,
        },
        GearTag::BeforeDamage => {
            weapon == WeaponKind::Hanbing && owner != target && !target_cards(g, target).is_empty()
        }
        GearTag::AfterDamage => {
            weapon == WeaponKind::Qilin
                && owner != target
                && (g.equip_in(target, Slot::HorseOffense).is_some()
                    || g.equip_in(target, Slot::HorseDefense).is_some())
        }
    }
}

fn target_cards(g: &Game, target: usize) -> Vec<CardId> {
    let mut out: Vec<CardId> = g.hand(target);
    for slot in [
        Slot::Weapon,
        Slot::Armor,
        Slot::HorseOffense,
        Slot::HorseDefense,
        Slot::Treasure,
    ] {
        if let Some(id) = g.equip_in(target, slot) {
            out.push(id);
        }
    }
    out
}

/// 压入装备窗口帧，交给 `sha_loop` 等待。
pub fn start(g: &mut Game, tag: GearTag, owner: usize, target: usize, card: Option<&CardId>) {
    g.stack.push(Frame::Flow(FlowFrame {
        kind: FlowKind::Gear,
        skill: GEAR_SKILL.to_string(),
        actor: owner,
        stage: Stage::GearChoose,
        data: json!({
            "tag": tag.key(),
            "owner": owner,
            "target": target,
            "card": card.map(|c| c.to_string()),
            "step": "",
            "picked": [],
        }),
    }));
}

fn builder(
    actor: usize,
    kind: &'static str,
) -> super::pending::PendingBuilder<super::pending::NoEscape> {
    skills::builder(actor, kind).skill(GEAR_SKILL)
}

/// 保存进度到当前帧并返回 `Step::Need`（与 `card_play::need_at` 同义）。
fn need(g: &mut Game, stage: Stage, data: &Value, p: Pending) -> Result<Step, String> {
    match g.stack.last_mut() {
        Some(Frame::Flow(f)) => {
            f.stage = stage;
            f.data = data.clone();
        }
        Some(Frame::Ask(ask)) => {
            ask.stage = stage;
            ask.data = data.clone();
        }
        _ => return Err("装备窗口：当前帧不可保存进度".into()),
    }
    Ok(Step::Need(p))
}

fn finished(g: &mut Game, result: GearResult) -> Step {
    g.gear_result = Some(result);
    Step::Done
}

/// 装备窗口的步进（由 `resolve::advance` 按 `FlowKind::Gear` 分派）。
pub fn step_gear(g: &mut Game, a: &Ask) -> Result<Step, String> {
    let tag = GearTag::parse(a.data["tag"].as_str().unwrap_or_default())
        .ok_or_else(|| "装备窗口：未知时点".to_string())?;
    let owner = a.data["owner"].as_u64().unwrap_or(0) as usize;
    let target = a.data["target"].as_u64().unwrap_or(0) as usize;
    let card: Option<CardId> = a.data["card"].as_str().map(str::to_string);
    let step = a.data["step"].as_str().unwrap_or("").to_string();
    let answer = g.answer.take();

    match step.as_str() {
        // 选择窗口选项。
        "" => match answer {
            Some(act) => choose(g, a, tag, owner, target, card, &act),
            None => {
                let p = offer(g, tag, owner, target)?;
                need(g, Stage::GearChoose, &a.data.clone(), p)
            }
        },
        // 雌雄：目标弃一张手牌。
        "cixiong_pick" => {
            let act = answer.ok_or_else(|| "装备窗口：等待输入但缺少回答".to_string())?;
            let id = pick_id(&act)?;
            if !g.hand(target).contains(&id) {
                return Err("雌雄双股剑：只能弃置自己的手牌".into());
            }
            g.to_discard(&id, "cixiong")?;
            g.push_log(format!(
                "雌雄双股剑：{} 弃置【{}】",
                g.seats[target].name,
                g.card_label(&id)
            ));
            Ok(finished(g, GearResult::Declined))
        }
        // 贯石斧：逐张弃 2 张手牌。
        "guanshi_pick" => {
            let act = answer.ok_or_else(|| "装备窗口：等待输入但缺少回答".to_string())?;
            let mut picked: Vec<CardId> = a.data["picked"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let id = pick_id(&act)?;
            if !g.hand(owner).contains(&id) {
                return Err("贯石斧：只能弃置自己的手牌".into());
            }
            picked.push(id);
            if picked.len() < 2 {
                let mut data = a.data.clone();
                data["picked"] = json!(picked);
                let p = pick_pending(g, owner, "贯石斧：再弃置一张手牌", &g.hand(owner))?;
                return need(g, Stage::GearPick, &data, p);
            }
            for id in &picked {
                g.to_discard(id, "guanshi")?;
            }
            g.push_log(format!(
                "贯石斧：{} 弃置 2 张手牌，此【杀】依然造成伤害",
                g.seats[owner].name
            ));
            Ok(finished(g, GearResult::Accepted))
        }
        // 寒冰剑：弃置目标两张牌（不足则全弃），本次伤害被防止。
        "hanbing_pick" => {
            let act = answer.ok_or_else(|| "装备窗口：等待输入但缺少回答".to_string())?;
            let mut picked: Vec<CardId> = a.data["picked"]
                .as_array()
                .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let id = pick_id(&act)?;
            if !target_cards(g, target).contains(&id) {
                return Err("寒冰剑：只能弃置目标的牌".into());
            }
            g.to_discard(&id, "hanbing")?;
            picked.push(id);
            let left = target_cards(g, target);
            if picked.len() < 2 && !left.is_empty() {
                let mut data = a.data.clone();
                data["picked"] = json!(picked);
                let p = pick_pending(g, owner, "寒冰剑：再弃置目标一张牌", &left)?;
                return need(g, Stage::GearPick, &data, p);
            }
            g.push_log(format!(
                "寒冰剑：{} 弃置 {} 的 {} 张牌，此伤害被防止",
                g.seats[owner].name,
                g.seats[target].name,
                picked.len()
            ));
            Ok(finished(g, GearResult::Prevented))
        }
        other => Err(format!("装备窗口：未知阶段 {other}")),
    }
}

fn pick_id(act: &str) -> Result<CardId, String> {
    act.strip_prefix("gear:pick:")
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("装备窗口：未知选项 {act}"))
}

fn pick_pending(
    g: &Game,
    actor: usize,
    label: &str,
    cards: &[CardId],
) -> Result<Pending, String> {
    let mut b = builder(actor, "choice");
    for id in cards {
        b = b.option(
            format!("gear:pick:{id}"),
            format!("{label}【{}】", g.card_label(id)),
        );
    }
    Ok(b.escape("不选（结束）").build())
}

fn offer(g: &Game, tag: GearTag, owner: usize, target: usize) -> Result<Pending, String> {
    match tag {
        GearTag::TargetChosen => {
            let mut b = builder(target, "choice");
            if !g.hand(target).is_empty() {
                b = b.option("gear:cixiong:discard", "弃置一张手牌");
            }
            b = b.option("gear:cixiong:draw", "令对方摸一张牌");
            Ok(b.escape("不发动雌雄双股剑").build())
        }
        GearTag::AfterDodge => {
            let weapon = g.weapon(owner);
            let mut b = builder(owner, "choice");
            if weapon == WeaponKind::Qinglong {
                for id in g.hand(owner) {
                    if g.def_of(&id).tags.sha {
                        b = b.option(
                            format!("gear:qinglong:{id}"),
                            format!(
                                "青龙偃月刀：弃置【{}】，目标需再出【闪】",
                                g.card_label(&id)
                            ),
                        );
                    }
                }
            }
            if weapon == WeaponKind::Guanshi && g.hand(owner).len() >= 2 {
                b = b.option("gear:guanshi", "贯石斧：弃置 2 张手牌，此【杀】依然造成伤害");
            }
            Ok(b.escape("不发动武器效果").build())
        }
        GearTag::BeforeDamage => {
            let mut b = builder(owner, "choice");
            b = b.option("gear:hanbing", "寒冰剑：防止此伤害，改为弃置其两张牌");
            Ok(b.escape("照常造成伤害").build())
        }
        GearTag::AfterDamage => {
            let mut b = builder(owner, "choice");
            for (slot, key) in [
                (Slot::HorseOffense, "offense"),
                (Slot::HorseDefense, "defense"),
            ] {
                if let Some(id) = g.equip_in(target, slot) {
                    b = b.option(
                        format!("gear:qilin:{key}"),
                        format!("麒麟弓：弃置其【{}】", g.card_label(&id)),
                    );
                }
            }
            Ok(b.escape("不发动麒麟弓").build())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn choose(
    g: &mut Game,
    a: &Ask,
    tag: GearTag,
    owner: usize,
    target: usize,
    _card: Option<CardId>,
    act: &str,
) -> Result<Step, String> {
    if act == "pass" {
        return Ok(finished(g, GearResult::Declined));
    }
    match (tag, act) {
        (GearTag::TargetChosen, "gear:cixiong:draw") => {
            let n = g.draw(owner, 1).len();
            g.push_log(format!("雌雄双股剑：{} 摸 {n} 张", g.seats[owner].name));
            Ok(finished(g, GearResult::Declined))
        }
        (GearTag::TargetChosen, "gear:cixiong:discard") => {
            let mut data = a.data.clone();
            data["step"] = json!("cixiong_pick");
            let p = pick_pending(g, target, "弃置", &g.hand(target))?;
            need(g, Stage::GearPick, &data, p)
        }
        (GearTag::AfterDodge, act) if act.starts_with("gear:qinglong:") => {
            let id = act.trim_start_matches("gear:qinglong:").to_string();
            if !g.hand(owner).contains(&id) {
                return Err("青龙偃月刀：该牌不在手牌".into());
            }
            g.to_discard(&id, "qinglong")?;
            g.push_log(format!(
                "青龙偃月刀：{} 弃置【{}】，目标需再次响应【杀】",
                g.seats[owner].name,
                g.card_label(&id)
            ));
            Ok(finished(g, GearResult::Retry))
        }
        (GearTag::AfterDodge, "gear:guanshi") => {
            let mut data = a.data.clone();
            data["step"] = json!("guanshi_pick");
            let p = pick_pending(g, owner, "贯石斧：弃置", &g.hand(owner))?;
            need(g, Stage::GearPick, &data, p)
        }
        (GearTag::BeforeDamage, "gear:hanbing") => {
            let mut data = a.data.clone();
            data["step"] = json!("hanbing_pick");
            let p = pick_pending(g, owner, "寒冰剑：弃置", &target_cards(g, target))?;
            need(g, Stage::GearPick, &data, p)
        }
        (GearTag::AfterDamage, act) if act.starts_with("gear:qilin:") => {
            let slot = if act.ends_with("offense") {
                Slot::HorseOffense
            } else {
                Slot::HorseDefense
            };
            if let Some(id) = g.equip_in(target, slot) {
                g.to_discard(&id, "qilin")?;
                g.push_log(format!(
                    "麒麟弓：{} 弃置 {} 的【{}】",
                    g.seats[owner].name,
                    g.seats[target].name,
                    g.card_label(&id)
                ));
            }
            Ok(finished(g, GearResult::Declined))
        }
        _ => Err(format!("装备窗口：未知选项 {act}")),
    }
}
