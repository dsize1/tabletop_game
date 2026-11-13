//! P4.0 数据/牌堆（标准 + 军争 = 160）/ 包开关（`Config.packs`）用例。
//!
//! 口径：`development-plan.md`「P4.0 交付锚点」——默认 160；`lobby:set_packs` 全局生效；
//! 军争牌定义已入库但结算未实现的牌不得出现在合法动作里。

use super::p24::start_and_pick;
use super::{clear_hands, edit, give, lobby_with, manager, pick_all, state_of};
use crate::state::*;
use crate::turn::PENDING_JUNZHENG_CARDS;
use crate::{data, FULL_DECK_SIZE, JUNZHENG_DECK_SIZE, STANDARD_DECK_SIZE};

/// 军争独有的牌定义（标准包没有同 code 的牌）。
const JUNZHENG_ONLY: &[&str] = &[
    "jiu",
    "huo_sha",
    "lei_sha",
    "huo_gong",
    "bingliang_cunduan",
    "tiesuo_lianhuan",
    "tengjia",
    "baiyin_shizi",
    "guding_dao",
    "zhuque_yushan",
    "hualiu",
];

fn cards_of(st: &SgsState) -> Vec<&str> {
    st.cards.values().map(|c| c.def.as_str()).collect()
}

#[test]
fn default_deck_is_standard_plus_junzheng() {
    let mgr = manager(600);
    let room = start_and_pick(&mgr, 5);
    let st = state_of(&mgr, &room);

    assert_eq!(
        st.config.packs,
        vec!["standard".to_string(), "junzheng".to_string()],
        "默认牌堆组合应为 标准 + 军争"
    );
    assert_eq!(st.config.packs_mode(), "standard+junzheng");
    let counts = st.check_conservation().unwrap();
    assert_eq!(counts.instances, FULL_DECK_SIZE);
    assert_eq!(counts.instances, 160);
    assert_eq!(counts.total_in_zones(), FULL_DECK_SIZE);
    // 5 人初始 4 张 + 主公首回合摸 2。
    assert_eq!(st.deck.draw.len(), 160 - 4 * st.seats.len() - 2);
    assert_eq!(st.cards.len(), 160);

    // 160 张里确实包含全部军争独有定义（每张物理牌都有对应 CardDef）。
    let codes = cards_of(&st);
    for code in JUNZHENG_ONLY {
        assert!(codes.contains(code), "160 牌堆缺少军争牌 {code}");
        assert!(data::card_def(code).is_some(), "缺少牌定义 {code}");
    }
}

#[test]
fn lobby_packs_switch_builds_standard_only_deck() {
    let mgr = manager(601);
    let room = lobby_with(&mgr, 5);
    let acts = mgr.legal_actions(&room, "u1").unwrap();
    assert!(
        acts.iter().any(|a| a.id == "lobby:set_packs:standard"),
        "大厅应提供切换到标准包的配置动作：{acts:?}"
    );
    mgr.execute_action(&room, "u1", "lobby:set_packs:standard")
        .unwrap();
    assert_eq!(
        state_of(&mgr, &room).config.packs,
        vec!["standard".to_string()]
    );

    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    pick_all(&mgr, &room);

    let st = state_of(&mgr, &room);
    let counts = st.check_conservation().unwrap();
    assert_eq!(counts.instances, STANDARD_DECK_SIZE);
    assert_eq!(counts.instances, 108);
    assert_eq!(st.deck.draw.len(), 108 - 4 * st.seats.len() - 2);
    let codes = cards_of(&st);
    for code in JUNZHENG_ONLY {
        assert!(!codes.contains(code), "标准包不应包含军争牌 {code}");
    }
    // 标准包对局结束后再改包组合无效（旧对局按创建时 config 固定）。
    assert!(mgr
        .execute_action(&room, "u1", "lobby:set_packs:standard+junzheng")
        .is_err());
}

#[test]
fn packs_config_is_host_only_and_validated() {
    let mgr = manager(602);
    let room = lobby_with(&mgr, 5);

    // 非房主不可改
    assert!(mgr
        .execute_action(&room, "u2", "lobby:set_packs:standard")
        .is_err());
    // 未知包 / 军争单独启用 / 空组合
    assert!(mgr
        .execute_action(&room, "u1", "lobby:set_packs:standard+unknown")
        .is_err());
    assert!(mgr
        .execute_action(&room, "u1", "lobby:set_packs:junzheng")
        .is_err());
    assert!(mgr.execute_action(&room, "u1", "lobby:set_packs:").is_err());
    // 合法别名：顺序无关 + 去重 → 规范化为 standard+junzheng
    mgr.execute_action(&room, "u1", "lobby:set_packs:junzheng+standard+standard")
        .unwrap();
    assert_eq!(
        state_of(&mgr, &room).config.packs_mode(),
        "standard+junzheng"
    );
}

#[test]
fn deck_cards_helper_obeys_packs() {
    let full = data::deck_cards(&["standard".to_string(), "junzheng".to_string()]).unwrap();
    assert_eq!(full.len(), FULL_DECK_SIZE);
    // 顺序固定：先标准后军争（同种子下 standard-only 对局的实例 id 保持稳定）。
    assert_eq!(full[..STANDARD_DECK_SIZE], data::STANDARD_DECK[..]);
    assert_eq!(
        full[STANDARD_DECK_SIZE..],
        data::JUNZHENG_DECK[..JUNZHENG_DECK_SIZE]
    );

    let standard = data::deck_cards(&["standard".to_string()]).unwrap();
    assert_eq!(standard.len(), STANDARD_DECK_SIZE);
    assert!(data::deck_cards(&[]).is_err());
    assert!(data::deck_cards(&["unknown".to_string()]).is_err());

    // 枚举：同一 code 的物理牌在两个包里的定义字段一致（生成脚本已校验）。
    for pc in data::JUNZHENG_DECK {
        assert_eq!(pc.pack, "junzheng");
    }
    for pc in data::STANDARD_DECK {
        assert_eq!(pc.pack, "standard");
    }
}

#[test]
fn pending_junzheng_cards_cannot_be_played_yet() {
    let mgr = manager(603);
    let room = start_and_pick(&mgr, 5);
    let mut given: Vec<(String, String)> = Vec::new();
    edit(&mgr, &room, |st| {
        clear_hands(st);
        let cur = st.turn;
        for code in PENDING_JUNZHENG_CARDS {
            let id = give(st, cur, code);
            given.push(((*code).to_string(), id));
        }
        give(st, cur, "hualiu"); // 骅骝只是防御马距离，通用装备逻辑可用
    });

    let st = state_of(&mgr, &room);
    let cur_pid = st.seats[st.turn].player_id.clone();
    let acts = mgr.legal_actions(&room, &cur_pid).unwrap();
    for (code, id) in &given {
        assert!(
            !acts.iter().any(|a| a.id == format!("play:{id}")),
            "未实现结算的军争牌 {code} 不应出现在合法动作里"
        );
        assert!(
            mgr.execute_action(&room, &cur_pid, &format!("play:{id}"))
                .is_err(),
            "未实现结算的军争牌 {code} 不应能直接打出"
        );
    }
    let hualiu = crate::tests::find_card(&st, "hualiu");
    assert!(
        acts.iter().any(|a| a.id == format!("play:{hualiu}")),
        "骅骝（纯距离装备）应可装备"
    );
    mgr.execute_action(&room, &cur_pid, &format!("play:{hualiu}"))
        .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.seats[st.turn].equips[&EquipSlot::HorseDefense].as_deref(),
        Some(hualiu.as_str())
    );
    st.check_conservation().unwrap();
}

/// 已实现结算的军争牌（其余必须在 `PENDING_JUNZHENG_CARDS` 里显式登记）。
/// P4.2（装备效果）/ P4.3（军争结算）落地后，把 code 从 pending 移到本表。
const JUNZHENG_IMPLEMENTED: &[&str] = &[
    "hualiu",
    "jiu",
    "huo_sha",
    "lei_sha",
    "tengjia",
    "baiyin_shizi",
    "guding_dao",
    "bingliang_cunduan",
    "zhuque_yushan",
    "huo_gong",
    "tiesuo_lianhuan",
];

/// 牌定义 ↔ 引擎结算覆盖门禁：每张已入库的牌必须属于「标准包」/「已实现军争牌」/
/// 「显式 pending」，防止 cards.json + 生成脚本新增牌后静默落入通用锦囊/装备分支。
#[test]
fn junzheng_card_defs_are_covered_or_explicitly_pending() {
    use std::collections::BTreeSet;

    let standard: BTreeSet<&str> = data::STANDARD_DECK.iter().map(|c| c.def).collect();
    let junzheng: BTreeSet<&str> = data::JUNZHENG_DECK.iter().map(|c| c.def).collect();
    let pending: BTreeSet<&str> = PENDING_JUNZHENG_CARDS.iter().copied().collect();
    let implemented: BTreeSet<&str> = JUNZHENG_IMPLEMENTED.iter().copied().collect();

    for def in data::CARD_DEFS {
        let covered = standard.contains(def.code)
            || pending.contains(def.code)
            || implemented.contains(def.code);
        assert!(
            covered,
            "牌定义 {}（{}）既不在标准包，也未登记为已实现/待实现军争牌；\
             新增牌请更新 turn.rs 的 PENDING_JUNZHENG_CARDS 或实现结算",
            def.code, def.name
        );
    }

    for code in JUNZHENG_IMPLEMENTED {
        assert!(data::card_def(code).is_some(), "已实现军争牌缺定义：{code}");
        assert!(junzheng.contains(code), "已实现军争牌不在军争牌堆：{code}");
    }
    for code in PENDING_JUNZHENG_CARDS {
        assert!(
            data::card_def(code).is_some(),
            "pending 列表含未知定义：{code}"
        );
        assert!(junzheng.contains(code), "pending 列表只能含军争牌：{code}");
        assert!(
            !standard.contains(code),
            "pending 列表不应包含标准包牌（标准结算已实现）：{code}"
        );
        assert!(
            !implemented.contains(code),
            "同一张牌不能同时登记为已实现与待实现：{code}"
        );
    }
    // 军争独有牌必须全部被这两张表覆盖（桃/闪/无懈等跨包牌已在标准包内）。
    for code in &junzheng {
        assert!(
            standard.contains(code) || pending.contains(code) || implemented.contains(code),
            "军争牌 {code} 未归类（已实现 / 待实现）"
        );
    }
}
