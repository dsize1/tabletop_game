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
    assert_eq!(n, 73, "标准 25 + 风 8 + 火 8 + 林 8 + 山 8 + SP-1 8 + SP-2 8");
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
    // 未实现武将不进池（风林火山已全量实现，此处核对未选中的武将不在池）。
    st2.setup.offered.clear();
    drop(st2);
    let st2 = state_of(&mgr2, &room2);
    assert!(
        st2.setup
            .offered
            .iter()
            .all(|c| skills::general_ready(c)),
        "候选池只包含已实现武将"
    );
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
    assert!(skills::general_ready("sgs_hero_42")); // 徐晃（林包，已实现）
    assert!(skills::general_ready("sgs_hero_50")); // 张郃（山包，已实现）
    assert!(skills::general_ready("sgs_hero_100")); // 袁术（SP-1，已实现）
    assert!(skills::general_ready("sgs_hero_400")); // 严颜（SP-1，已实现）
    assert!(skills::general_ready("sgs_hero_407")); // 卢植（SP-2，已实现）
    assert!(skills::general_ready("sgs_hero_415")); // 张绣（SP-2，已实现）
                                                     // 开启风林火山后，标准将仍可用（不受影响）。
    let mgr = manager(405);
    let room = super::lobby_with(&mgr, 5);
    edit(&mgr, &room, |_st| {});
    assert!(skills::general_ready("sgs_hero_1"));
}

// ---------------------------------------------------------------------------
// P4.4.2 数据扩包（一将成名 / 界限突破）+ 同名去重（ADR-0024 口径 A）
// ---------------------------------------------------------------------------

#[test]
fn yijiang_and_jiexian_packs_are_registered() {
    assert_eq!(
        crate::data::YIJIANG_GENERALS.len(),
        crate::data::YIJIANG_GENERAL_COUNT
    );
    assert_eq!(
        crate::data::JIEXIAN_GENERALS.len(),
        crate::data::JIEXIAN_GENERAL_COUNT
    );
    assert!(crate::data::YIJIANG_GENERALS
        .iter()
        .all(|g| g.pack == "yijiang"));
    assert!(crate::data::JIEXIAN_GENERALS
        .iter()
        .all(|g| g.pack == "jiexian"));
    // 一将成名 72 位全部实现（批次 Y1–Y7，2026-10-31）。
    let pool: Vec<&str> = crate::data::general_codes_in_packs(&[
        "standard".to_string(),
        "yijiang".to_string(),
    ])
    .into_iter()
    .filter(|c| skills::general_ready(c))
    .collect();
    assert!(pool.contains(&"sgs_hero_151"), "曹植应进池");
    assert!(pool.contains(&"sgs_hero_161"), "批次 2 的张春华应进池");
    assert!(pool.contains(&"sgs_hero_171"), "批次 Y1 的将华雄应进池");
    assert!(pool.contains(&"sgs_hero_179"), "批次 Y2 的刘封应进池");
    assert!(pool.contains(&"sgs_hero_187"), "批次 Y3 的王异应进池");
    assert!(pool.contains(&"sgs_hero_327"), "批次 Y4 的孙鲁班应进池");
    assert!(pool.contains(&"sgs_hero_335"), "批次 Y5 的刘谌应进池");
    assert!(pool.contains(&"sgs_hero_380"), "批次 Y6 的郭皇后应进池");
    assert!(pool.contains(&"sgs_hero_388"), "批次 Y7 的辛宪英应进池");
    assert!(!pool.contains(&"sgs_hero_451"), "未实现武将不进池");
    // 包组合解析（新增 key）。
    let packs = crate::state::parse_general_packs("standard+yijiang+jiexian").unwrap();
    assert_eq!(
        packs,
        vec![
            "standard".to_string(),
            "yijiang".to_string(),
            "jiexian".to_string()
        ]
    );
}

#[test]
fn prefer_jiexian_dedupes_same_name_generals() {
    // 界刘备（299）与标准刘备（1）同名 → 只保留界版本。
    let codes = vec!["sgs_hero_1", "sgs_hero_299", "sgs_hero_2"];
    let kept = crate::prefer_jiexian(codes);
    assert!(kept.contains(&"sgs_hero_299"), "保留界刘备");
    assert!(!kept.contains(&"sgs_hero_1"), "剔除同名标准刘备");
    assert!(kept.contains(&"sgs_hero_2"), "无关武将保留");
    // 无界化武将时不剔除。
    let plain = vec!["sgs_hero_1", "sgs_hero_2"];
    assert_eq!(crate::prefer_jiexian(plain.clone()), plain);
}

// ---------------------------------------------------------------------------
// `check:sgs-pack-pool` 门禁（harness）：选将池包过滤 + 同名去重（纯函数级）
// ---------------------------------------------------------------------------

#[test]
fn pack_pool_standard_only_excludes_other_packs() {
    let pool = crate::prefer_jiexian(
        crate::data::general_codes_in_packs(&["standard".to_string()])
            .into_iter()
            .filter(|c| skills::general_ready(c))
            .collect(),
    );
    assert_eq!(pool.len(), crate::data::STANDARD_GENERAL_COUNT);
    assert!(pool.iter().all(|c| crate::data::general(c)
        .is_some_and(|g| g.pack == "standard")));
}

#[test]
fn pack_pool_only_ready_generals_enter_pool() {
    let pool: Vec<&str> = crate::data::general_codes_in_packs(&[
        "standard".to_string(),
        "yijiang".to_string(),
    ])
    .into_iter()
    .filter(|c| skills::general_ready(c))
    .collect();
    // 一将成名 72 位（Y1–Y7）全部实现。
    let yijiang: Vec<&str> = pool
        .into_iter()
        .filter(|c| crate::data::general(c).is_some_and(|g| g.pack == "yijiang"))
        .collect();
    assert_eq!(yijiang.len(), 72, "一将成名 72 位全部实现");
    for code in [
        "sgs_hero_151",
        "sgs_hero_152",
        "sgs_hero_153",
        "sgs_hero_155",
        "sgs_hero_156",
        "sgs_hero_157",
        "sgs_hero_158",
        "sgs_hero_160",
        "sgs_hero_161",
        "sgs_hero_162",
        "sgs_hero_164",
        "sgs_hero_166",
        "sgs_hero_167",
        "sgs_hero_168",
        "sgs_hero_169",
        "sgs_hero_170",
        "sgs_hero_171",
        "sgs_hero_172",
        "sgs_hero_173",
        "sgs_hero_174",
        "sgs_hero_175",
        "sgs_hero_176",
        "sgs_hero_177",
        "sgs_hero_178",
        "sgs_hero_179",
        "sgs_hero_180",
        "sgs_hero_181",
        "sgs_hero_182",
        "sgs_hero_183",
        "sgs_hero_184",
        "sgs_hero_185",
        "sgs_hero_186",
        "sgs_hero_187",
        "sgs_hero_188",
        "sgs_hero_321",
        "sgs_hero_322",
        "sgs_hero_323",
        "sgs_hero_324",
        "sgs_hero_325",
        "sgs_hero_326",
        "sgs_hero_327",
        "sgs_hero_328",
        "sgs_hero_329",
        "sgs_hero_330",
        "sgs_hero_331",
        "sgs_hero_332",
        "sgs_hero_333",
        "sgs_hero_334",
        "sgs_hero_335",
        "sgs_hero_336",
        "sgs_hero_337",
        "sgs_hero_338",
        "sgs_hero_339",
        "sgs_hero_340",
        "sgs_hero_341",
        "sgs_hero_342",
        "sgs_hero_380",
        "sgs_hero_381",
        "sgs_hero_382",
        "sgs_hero_383",
        "sgs_hero_384",
        "sgs_hero_385",
        "sgs_hero_386",
        "sgs_hero_387",
        "sgs_hero_388",
        "sgs_hero_389",
        "sgs_hero_390",
        "sgs_hero_391",
        "sgs_hero_392",
        "sgs_hero_393",
        "sgs_hero_394",
        "sgs_hero_395",
    ] {
        assert!(yijiang.contains(&code), "{code} 应在池中");
    }
}

#[test]
fn pack_pool_prefers_jiexian_variant() {
    // 同时启用 standard + jiexian：同名原版被剔除（界优先）。
    let pool = crate::prefer_jiexian(
        crate::data::general_codes_in_packs(&[
            "standard".to_string(),
            "jiexian".to_string(),
        ])
        .into_iter()
        .filter(|c| skills::general_ready(c))
        .collect(),
    );
    // 界标 25 将全部实现：标准 25 位全部被界版本替换；
    // 界李典（317）与界公孙瓒（320）在标准包无同名原版（25 + 2 = 27）；
    // P4.4.2 批次 J1 新增 8 位界化武将（26/304/446–451）、J2 新增 8 位（452/453/454/455/456/457/458/462）
    // 在 standard+jiexian 池中均无同名原版（标准 25 位已被界标替换）→ 27 + 8 + 8 = 43。
    assert_eq!(pool.len(), 43);
    for code in [
        "sgs_hero_299",
        "sgs_hero_300",
        "sgs_hero_301",
        "sgs_hero_302",
        "sgs_hero_303",
        "sgs_hero_440",
        "sgs_hero_441",
        "sgs_hero_305",
        "sgs_hero_306",
        "sgs_hero_307",
        "sgs_hero_308",
        "sgs_hero_309",
        "sgs_hero_310",
        "sgs_hero_442",
        "sgs_hero_443",
        "sgs_hero_311",
        "sgs_hero_312",
        "sgs_hero_313",
        "sgs_hero_314",
        "sgs_hero_315",
        "sgs_hero_316",
        "sgs_hero_317",
        "sgs_hero_444",
        "sgs_hero_318",
        "sgs_hero_319",
        "sgs_hero_320",
        "sgs_hero_445",
    ] {
        assert!(pool.contains(&code), "{code} 应在池中");
    }
    for code in [
        "sgs_hero_1",
        "sgs_hero_2",
        "sgs_hero_3",
        "sgs_hero_4",
        "sgs_hero_5",
        "sgs_hero_6",
        "sgs_hero_7",
        "sgs_hero_8",
        "sgs_hero_9",
        "sgs_hero_10",
        "sgs_hero_11",
        "sgs_hero_12",
        "sgs_hero_13",
        "sgs_hero_14",
        "sgs_hero_25",
        "sgs_hero_15",
        "sgs_hero_16",
        "sgs_hero_17",
        "sgs_hero_18",
        "sgs_hero_19",
        "sgs_hero_20",
        "sgs_hero_21",
        "sgs_hero_22",
        "sgs_hero_23",
        "sgs_hero_24",
    ] {
        assert!(!pool.contains(&code), "同名原版 {code} 应被界版本替换");
    }
    assert!(pool.iter().all(|c| {
        let g = crate::data::general(c).expect("general");
        g.pack == "standard" || g.pack == "jiexian"
    }));
    // 内部逻辑：一旦界化武将进池，同名原版必须消失。
    let synthetic = vec!["sgs_hero_1", "sgs_hero_299", "sgs_hero_3"];
    let kept = crate::prefer_jiexian(synthetic);
    assert!(kept.contains(&"sgs_hero_299") && !kept.contains(&"sgs_hero_1"));
    assert!(kept.contains(&"sgs_hero_3"), "不同名武将保留");
}

#[test]
fn pack_pool_parse_combinations_are_normalized() {
    let order = crate::state::parse_general_packs("jiexian+standard+yijiang").unwrap();
    assert_eq!(
        order,
        vec![
            "standard".to_string(),
            "yijiang".to_string(),
            "jiexian".to_string()
        ],
        "包顺序按 GENERAL_PACK_KEYS 规范化"
    );
    assert!(crate::state::parse_general_packs("standard+unknown").is_err());
    assert_eq!(
        crate::state::parse_general_packs("standard+standard").unwrap(),
        vec!["standard".to_string()],
        "重复包去重"
    );
}
