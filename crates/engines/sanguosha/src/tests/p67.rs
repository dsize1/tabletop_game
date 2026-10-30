//! P4.4.0：武将包数据与选将开关（`Config.general_packs`）。

use super::p24::start_and_pick;
use super::{edit, manager, state_of};
use crate::skills;
use crate::state::*;

#[test]
fn general_packs_default_and_parse() {
    let cfg = Config::default();
    assert_eq!(cfg.general_packs, vec!["standard".to_string()]);
    assert_eq!(cfg.general_packs_mode(), "standard");
    // 顺序归一化（按 data::GENERAL_PACK_KEYS）+ 去重。
    assert_eq!(
        parse_general_packs("wind_fire_forest_mountain+standard+standard").unwrap(),
        vec![
            "standard".to_string(),
            "wind_fire_forest_mountain".to_string()
        ]
    );
    assert!(parse_general_packs("").is_err());
    assert!(parse_general_packs("standard+unknown").is_err());
    // 风林火山包 48 将已生成；当前只实现风包 8 将 → 选将池 33。
    assert_eq!(crate::data::WIND_FIRE_FOREST_MOUNTAIN_GENERALS.len(), 48);
    assert_eq!(
        crate::data::general_codes_in_packs(&["wind_fire_forest_mountain".to_string()]).len(),
        48
    );
    let mut n = 0;
    for code in crate::data::general_codes_in_packs(&[
        "standard".to_string(),
        "wind_fire_forest_mountain".to_string(),
    ]) {
        if skills::general_ready(code) {
            n += 1;
        }
    }
    assert_eq!(n, 41, "标准 25 + 风包 8 + 火包 8");
}

#[test]
fn general_pack_switch_filters_offer_pool() {
    // 标准包：候选池不应出现风林火山武将。
    let mgr = manager(401);
    let room = start_and_pick(&mgr, 5);
    let st = state_of(&mgr, &room);
    let offered: Vec<String> = st.setup.offered.iter().cloned().collect();
    assert!(offered.iter().all(|c| c.starts_with("sgs_hero_")));
    assert!(
        offered
            .iter()
            .all(|c| crate::data::general(c).is_some_and(|g| g.pack == "standard")),
        "标准包候选出现非标准武将：{offered:?}"
    );

    // 标准+风林火山：风包已实现 8 将可进入候选。
    let mgr2 = manager(402);
    let room2 = super::start_with_config(&mgr2, 5, |cfg| {
        cfg.general_packs = vec![
            "standard".to_string(),
            "wind_fire_forest_mountain".to_string(),
        ];
    });
    let mut st2 = state_of(&mgr2, &room2);
    // 主公候选必然包含张角（风包主公将）。
    assert!(
        st2.setup
            .offers
            .values()
            .flatten()
            .any(|c| c == "sgs_hero_212"),
        "开启风林火山后主公候选应包含张角"
    );
    // 其余玩家的候选也只可能来自启用包 + 已实现武将。
    for code in st2.setup.pool.iter() {
        let def = crate::data::general(code).expect("known general");
        assert!(matches!(def.pack, "standard" | "wind_fire_forest_mountain"));
        assert!(skills::general_ready(code));
    }
    // 未实现武将不进池。
    st2.setup.offered.clear();
    drop(st2);
    let st2 = state_of(&mgr2, &room2);
    assert!(!st2.setup.offered.contains("sgs_hero_42"));
}

#[test]
fn old_snapshot_without_general_packs_defaults_to_standard() {
    let mgr = manager(403);
    let room = start_and_pick(&mgr, 5);
    let mut data = mgr.get_state(&room).expect("state").data;
    // 模拟旧快照：删掉 config.general_packs。
    data["config"]
        .as_object_mut()
        .expect("config object")
        .remove("general_packs");
    let st = SgsState::parse(&data).expect("parse old snapshot");
    assert_eq!(st.config.general_packs, vec!["standard".to_string()]);
}

#[test]
fn lobby_general_pack_action_only_before_start() {
    let mgr = manager(404);
    let room = super::lobby_with(&mgr, 5);
    let acts = mgr.legal_actions(&room, "u1").expect("actions");
    assert!(
        acts.iter()
            .any(|a| a.id == "lobby:set_general_packs:standard+wind_fire_forest_mountain"),
        "大厅应提供武将包开关"
    );
    mgr.execute_action(
        &room,
        "u1",
        "lobby:set_general_packs:standard+wind_fire_forest_mountain",
    )
    .unwrap();
    let st = state_of(&mgr, &room);
    assert_eq!(
        st.config.general_packs_mode(),
        "standard+wind_fire_forest_mountain"
    );
    // 已开局后不可再改。
    mgr.execute_action(&room, "u1", "lobby:start").unwrap();
    let err = mgr
        .execute_action(&room, "u1", "lobby:set_general_packs:standard")
        .expect_err("开局后应拒绝");
    assert!(format!("{err:?}").contains("已开局"));
}

#[test]
fn general_ready_gates_unimplemented_generals() {
    assert!(skills::general_ready("sgs_hero_27")); // 魏延
    assert!(skills::general_ready("sgs_hero_212")); // 张角
    assert!(skills::general_ready("sgs_hero_34")); // 典韦（火包，已实现）
    assert!(!skills::general_ready("sgs_hero_42")); // 徐晃（林包，未实现）
    assert!(!skills::general_ready("sgs_hero_400")); // 严颜（后续/SP，未实现）
                                                     // 开启风林火山后，标准将仍可用（不受影响）。
    let mgr = manager(405);
    let room = super::lobby_with(&mgr, 5);
    edit(&mgr, &room, |_st| {});
    assert!(skills::general_ready("sgs_hero_1"));
}
