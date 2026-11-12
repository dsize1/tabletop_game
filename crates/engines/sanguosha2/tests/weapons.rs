//! 装备特效（S2）：青龙偃月刀 / 贯石斧 / 寒冰剑 / 雌雄双股剑 / 麒麟弓 / 古锭刀 /
//! 朱雀羽扇 / 方天画戟 / 诸葛连弩。
//!
//! 断言只读属性与状态（`cards::WeaponKind` / 牌区 / 体力 / 判定），不按牌名分支。

use sanguosha2::cards::{self, DamageKind};
use sanguosha2::kernel::resolve::{begin_phase, submit};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Slot, Zone};

fn new_game(n: usize) -> Game {
    let seats: Vec<Seat> = (0..n)
        .map(|i| Seat::new(&format!("p{i}"), "guest", &[], 4))
        .collect();
    let mut g = Game::new(seats, cards::deck());
    begin_phase(&mut g, 0, Phase::Play).expect("进入 p0 出牌阶段");
    g
}

fn give(g: &mut Game, seat: usize, name: &str) -> CardId {
    let id = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.name_of(id) == name)
        .unwrap_or_else(|| panic!("牌堆里没有【{name}】"));
    g.to_hand(&id, seat).expect("给牌");
    id
}

/// 直接装备（不经过使用流程，避免消耗出牌阶段次数）。
fn equip(g: &mut Game, seat: usize, name: &str, slot: Slot) -> CardId {
    let id = give(g, seat, name);
    g.to_equip(&id, seat, slot).expect("装备");
    id
}

/// I1 询问完整性（v1 口径 2026-10-28）：有手牌就必须显式询问。
/// 这里把「无牌可打」的三类响应询问（闪/杀、无懈、濒死）按唯一出口放弃。
fn drain_forced(g: &mut Game) {
    while let Some(p) = g.pending.clone() {
        if !matches!(p.kind(), "card_response" | "wuxie" | "dying") {
            break;
        }
        if p.options().iter().any(|o| o.id != p.escape_id()) {
            break;
        }
        let (actor, escape) = (p.actor(), p.escape_id().to_string());
        submit(g, actor, &escape).expect("放弃唯一出口");
    }
}

fn pass_pending(g: &mut Game, expect_skill: &str, action: &str) {
    let p = g.pending.clone().expect("应有待办");
    assert_eq!(p.skill(), Some(expect_skill), "待办技能不符");
    let actor = p.actor();
    submit(g, actor, action).unwrap_or_else(|e| panic!("提交 {action} 失败：{e}"));
    // I1：作答后可能立刻产生新的「无牌可打」礼貌询问（如锦囊的无懈窗口）→ 继续排空。
    drain_forced(g);
}

// ---------------------------------------------------------------------------

#[test]
fn zhugeliannu_removes_sha_limit() {
    let mut g = new_game(3);
    equip(&mut g, 0, "诸葛连弩", Slot::Weapon);
    assert_eq!(g.sha_limit(0), usize::MAX, "诸葛连弩应取消【杀】次数上限");
    g.seats[0].hp = 4;
    let s1 = give(&mut g, 0, "杀");
    let s2 = give(&mut g, 0, "杀");
    g.resolve_use(0, &s1, vec![1]).expect("第一张杀");
    // 目标不出闪 → 直接结算。
    while g.pending.is_some() {
        let p = g.pending.clone().unwrap();
        submit(&mut g, p.actor(), "pass").unwrap();
    }
    g.resolve_use(0, &s2, vec![1]).expect("第二张杀（连弩）");
}

#[test]
fn fangtian_allows_three_targets_with_last_hand_card() {
    let mut g = new_game(4);
    equip(&mut g, 0, "方天画戟", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    // 只留这一张手牌。
    assert_eq!(g.hand(0), vec![sha.clone()]);
    let actions = sanguosha2::kernel::card_play::legal_uses(&g, 0);
    assert!(
        actions.iter().any(|(id, _)| id == &format!("use:{sha}:1,2,3")),
        "方天画戟应允许 3 个目标：{:?}",
        actions.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    g.resolve_use(0, &sha, vec![1, 2, 3]).expect("三目标杀");
}

#[test]
fn gudingdao_adds_damage_against_empty_hand() {
    let mut g = new_game(3);
    equip(&mut g, 0, "古锭刀", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    // p1 无手牌、无防具 → 不出闪，伤害 1+1。
    let before = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    while g.pending.is_some() {
        let p = g.pending.clone().unwrap();
        submit(&mut g, p.actor(), "pass").unwrap();
    }
    assert_eq!(g.seats[1].hp, before - 2, "古锭刀应 +1 伤害");
}

#[test]
fn zhuque_turns_normal_sha_into_fire() {
    let mut g = new_game(3);
    equip(&mut g, 0, "朱雀羽扇", Slot::Weapon);
    // p1 装备藤甲：普通杀无效、火杀 +1。
    equip(&mut g, 1, "藤甲", Slot::Armor);
    let sha = give(&mut g, 0, "杀");
    assert_eq!(g.def_of(&sha).damage_kind, DamageKind::Normal);
    let before = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    while g.pending.is_some() {
        let p = g.pending.clone().unwrap();
        submit(&mut g, p.actor(), "pass").unwrap();
    }
    assert_eq!(
        g.seats[1].hp,
        before - 2,
        "朱雀羽扇把普通杀改为火杀（藤甲 +1）"
    );
}

#[test]
fn qinglong_retries_after_dodge() {
    let mut g = new_game(3);
    equip(&mut g, 0, "青龙偃月刀", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    let extra_sha = give(&mut g, 0, "杀");
    let shan = give(&mut g, 1, "闪");
    let second_shan = give(&mut g, 1, "闪");
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    // 第一次【闪】→ 青龙窗口 → 弃一张【杀】，目标需再出【闪】。
    let p = g.pending.clone().expect("闪响应");
    submit(&mut g, p.actor(), &format!("respond:{shan}")).unwrap();
    let p = g.pending.clone().expect("青龙窗口");
    assert_eq!(p.skill(), Some("__gear"));
    let opt = p
        .options()
        .iter()
        .find(|o| o.id == format!("gear:qinglong:{extra_sha}"))
        .expect("应有青龙选项")
        .id
        .clone();
    submit(&mut g, p.actor(), &opt).unwrap();
    assert!(
        !g.hand(0).contains(&extra_sha),
        "青龙应弃置一张【杀】"
    );
    // 目标被再次要求出【闪】（第二张可用也可放弃）。
    let p = g.pending.clone().expect("应再次要求出闪");
    assert_eq!(p.kind(), "card_response");
    if p.has_option(&format!("respond:{second_shan}")) {
        submit(&mut g, p.actor(), &format!("respond:{second_shan}")).unwrap();
    } else if p.has_option("pass") {
        submit(&mut g, p.actor(), "pass").unwrap();
    }
}

#[test]
fn hanbing_prevents_damage_and_discards_two_cards() {
    let mut g = new_game(3);
    equip(&mut g, 0, "寒冰剑", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    let c1 = give(&mut g, 1, "桃");
    let c2 = give(&mut g, 1, "桃");
    let before = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    drain_forced(&mut g); // I1：p1 无【闪】但有手牌 → 仍被显式询问（唯一出口=放弃）
    // p1 不出闪 → 造成伤害前，攻击者选择寒冰剑。
    let p = g.pending.clone().expect("应有装备窗口");
    assert_eq!(p.skill(), Some("__gear"), "寒冰剑窗口");
    submit(&mut g, p.actor(), "gear:hanbing").unwrap();
    let p = g.pending.clone().expect("应选第一张牌");
    submit(&mut g, p.actor(), &format!("gear:pick:{c1}")).unwrap();
    let p = g.pending.clone().expect("应选第二张牌");
    submit(&mut g, p.actor(), &format!("gear:pick:{c2}")).unwrap();
    while g.pending.is_some() {
        let p = g.pending.clone().unwrap();
        submit(&mut g, p.actor(), "pass").unwrap();
    }
    assert_eq!(g.seats[1].hp, before, "寒冰剑应防止伤害");
    assert_eq!(
        g.ids_in(|z| *z == Zone::Discard).len() >= 2,
        true,
        "应弃置目标两张牌"
    );
}

#[test]
fn qilin_discards_horse_after_damage() {
    let mut g = new_game(3);
    equip(&mut g, 0, "麒麟弓", Slot::Weapon);
    let horse = equip(&mut g, 1, "赤兔", Slot::HorseOffense);
    let sha = give(&mut g, 0, "杀");
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    // 目标不出闪 → 伤害后麒麟弓窗口。
    let mut guard = 0;
    while g.pending.is_some() && guard < 8 {
        guard += 1;
        let p = g.pending.clone().unwrap();
        if p.skill() == Some("__gear") {
            let opt = p
                .options()
                .iter()
                .find(|o| o.id.starts_with("gear:qilin:"))
                .expect("应有麒麟弓选项")
                .id
                .clone();
            submit(&mut g, p.actor(), &opt).unwrap();
        } else {
            submit(&mut g, p.actor(), "pass").unwrap();
        }
    }
    assert!(
        !g.hand(1).contains(&horse) && g.zone_of(&horse) != Some(&Zone::Equip(1, Slot::HorseOffense)),
        "麒麟弓应弃置目标坐骑"
    );
}

#[test]
fn cixiong_offers_choice_to_opposite_gender_target() {
    let mut g = new_game(3);
    // p0 男、p1 女（直接改属性，避免依赖具体武将）。
    g.seats[1].male = false;
    equip(&mut g, 0, "雌雄双股剑", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    let p = g.pending.clone().expect("雌雄窗口");
    assert_eq!(p.skill(), Some("__gear"));
    assert_eq!(
        p.actor(),
        1,
        "雌雄双股剑由**目标**选择（弃牌或令使用者摸牌）"
    );
}

#[test]
fn guanshi_discards_two_to_force_damage() {
    let mut g = new_game(3);
    equip(&mut g, 0, "贯石斧", Slot::Weapon);
    let sha = give(&mut g, 0, "杀");
    let a = give(&mut g, 0, "桃");
    let b = give(&mut g, 0, "桃");
    give(&mut g, 1, "闪");
    let before = g.seats[1].hp;
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    // p1 出闪 → 贯石斧窗口（攻击者弃 2 张）。
    let mut guard = 0;
    while g.pending.is_some() && guard < 10 {
        guard += 1;
        let p = g.pending.clone().unwrap();
        let skill = p.skill().unwrap_or("").to_string();
        let opt: Option<String> = match skill.as_str() {
            "__card" => p
                .options()
                .iter()
                .find(|o| o.id.starts_with("respond:"))
                .map(|o| o.id.clone()),
            "__gear" => {
                if p.options().iter().any(|o| o.id == "gear:guanshi") {
                    Some("gear:guanshi".to_string())
                } else {
                    p.options()
                        .iter()
                        .find(|o| o.id.starts_with("gear:pick:"))
                        .map(|o| o.id.clone())
                }
            }
            _ => None,
        };
        if let Some(id) = opt {
            submit(&mut g, p.actor(), &id).unwrap();
        } else {
            submit(&mut g, p.actor(), "pass").unwrap();
        }
    }
    assert_eq!(g.seats[1].hp, before - 1, "贯石斧应强行造成伤害");
    let _ = (&a, &b);
    assert_eq!(
        g.hand(0).len(),
        1,
        "贯石斧应弃置 2 张手牌（初始 3 张：杀+2 张桃，弃后只剩 1 张）"
    );
}

#[test]
fn weapon_effects_only_trigger_for_their_weapon() {
    // 没有武器时不出现任何装备窗口（回归：避免无条件压帧）。
    let mut g = new_game(3);
    let sha = give(&mut g, 0, "杀");
    g.resolve_use(0, &sha, vec![1]).expect("使用杀");
    assert!(
        g.pending.as_ref().map(|p| p.skill()) != Some(Some("__gear")),
        "无武器不应出现装备窗口"
    );
    let _ = cards::def("杀");
}
