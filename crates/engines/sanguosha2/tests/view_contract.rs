//! M3 视图契约（`check:sgs-view-contract` 对 v2 运行）。
//!
//! 冻结 `public_board` / `player_area` / `special_zones` 的字段集，并验证隐藏信息不泄漏：
//! 手牌明细、牌堆顺序、他人 pending 选项/数据、他人身份、私密展示。
//!
//! 新增视图字段必须先改本文件的期望集（等价一次契约评审）。

use sanguosha2::cards;
use sanguosha2::kernel::resolve::{begin_phase, land};
use sanguosha2::kernel::{CardId, Game, Phase, Seat, Slot, Zone};
use sanguosha2::view;

const ROOT_KEYS: &[&str] = &[
    "schema_version",
    // v1 兼容分派键（Web 按 `data.game === "sanguosha"` 走三分区页）。
    "game",
    "game_id",
    "turn",
    "phase",
    "round",
    // 终局房规：第 100 轮后桃/桃园回血无效（端侧提示用）。
    "endgame",
    "public_board",
    "player_area",
    "special_zones",
];

const BOARD_KEYS: &[&str] = &[
    // v1 兼容（Web 三分区页消费）
    "phase",
    "round",
    "turn_index",
    "turn",
    "config",
    "seat_count",
    "seats",
    "deck",
    "discard_top",
    "discard",
    "pending",
    "setup",
    "winner",
    "aborted",
    "history",
    // v2 自有（数量投影 / 表现 / 日志）
    "draw_count",
    "void_count",
    "processing_count",
    "frame_count",
    "revealed",
    "presentation",
    "log",
    "finished",
];

const SEAT_KEYS: &[&str] = &[
    "seat",
    "player_id",
    "identity",
    "general",
    "general_name",
    "kingdom",
    "hp",
    "max_hp",
    "hand_count",
    "hand_limit",
    "equips",
    "judge_zone",
    "marks",
    "marks_count",
    "abolished_slots",
    "judge_abolished",
    "flipped",
    "chained",
    "alive",
    "is_bot",
    "has_picked",
    "skills",
    "transforms",
];

/// 公共视图的 `player_area`（无 viewer：全部私有字段为空）。
const AREA_KEYS_PUBLIC: &[&str] = &[
    "seat",
    "player_id",
    "identity",
    "hand",
    "hand_limit",
    "private_reveals",
    "pending",
    "setup",
];

/// 个人视图的 `player_area`（v1 兼容 + v2 补充）。
const AREA_KEYS_PRIVATE: &[&str] = &[
    "seat",
    "player_id",
    "identity",
    "general",
    "general_gid",
    "kingdom",
    "general_name",
    "hp",
    "max_hp",
    "hand",
    "hand_limit",
    "equips",
    "judge_zone",
    "marks",
    "marks_count",
    "transforms",
    "abolished_slots",
    "judge_abolished",
    "flipped",
    "chained",
    "skills",
    "equipment_skills",
    "is_bot",
    "is_current",
    "pending",
    "setup",
    "private_reveals",
];

const CARD_KEYS: &[&str] = &[
    "id",
    "def",
    "name",
    "name_is_virtual",
    "suit",
    "suit_label",
    "rank",
    "kind",
    // v1 兼容：牌面详情（category/equip_slot/range_*）。
    "category",
    "equip_slot",
    "range_value",
    "range_type",
    "slot",
    "effect",
    "damage_kind",
    "target_mode",
    "delayed",
    "revealed",
    "marks",
];

const PENDING_PUBLIC_KEYS: &[&str] = &["kind", "actor", "skill"];
const PENDING_PRIVATE_KEYS: &[&str] = &["kind", "actor", "skill", "options", "escape", "data"];
const PRESENTATION_KEYS: &[&str] = &[
    "skill", "actor", "targets", "cards", "outcome", "fx", "text",
];
const LOG_KEYS: &[&str] = &["seq", "text"];
const EQUIP_SLOTS: &[&str] = &[
    "weapon",
    "armor",
    "horse_offense",
    "horse_defense",
    "treasure",
];

fn assert_keys(value: &serde_json::Value, expected: &[&str], label: &str) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{label} 不是对象：{value}"));
    let mut got: Vec<String> = obj.keys().cloned().collect();
    got.sort();
    let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "{label} 字段集不符：{value}");
}

// ---------------------------------------------------------------------------
// 夹具：3 人局（周妃 / 卢植 / 路人），手工布置 9 类区
// ---------------------------------------------------------------------------

struct Fixture {
    g: Game,
    hand0: Vec<CardId>,
    equip0: CardId,
    judge1: CardId,
    mark0: CardId,
}

fn fixture() -> Fixture {
    // 全 guest：避免技能触发干扰视图断言（转换技另起夹具）。
    let seats = vec![
        Seat::new("p0", "guest", &[], 4),
        Seat::new("p1", "guest", &[], 4),
        Seat::new("p2", "guest", &[], 4),
    ];
    let mut g = Game::new(seats, cards::deck());
    // 手牌（p0 两张 / p1 一张）
    let hand0: Vec<CardId> = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .take(2)
        .collect();
    for id in &hand0 {
        g.to_hand(id, 0).unwrap();
    }
    let hand1 = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    g.to_hand(&hand1, 1).unwrap();
    // 装备 / 判定区 / 标记 / 弃牌堆 / 移出游戏
    let equip0 = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.def_of(id).slot == Some(Slot::Weapon))
        .unwrap();
    g.to_equip(&equip0, 0, Slot::Weapon).unwrap();
    let judge1 = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| g.def_of(id).tags.delayed)
        .unwrap();
    g.to_judge(&judge1, 1).unwrap();
    let mark0 = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    g.to_mark(&mark0, 0, "kongsheng").unwrap();
    let discard = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    g.to_discard(&discard, "test").unwrap();
    let void = g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    g.to_void(&void).unwrap();
    g.push_log("fixture");
    Fixture {
        g,
        hand0,
        equip0,
        judge1,
        mark0,
    }
}

// ---------------------------------------------------------------------------
// 字段集契约
// ---------------------------------------------------------------------------

#[test]
fn view_contract_root_and_board_shape() {
    let f = fixture();
    let pubv = view::public_state(&f.g);
    assert_keys(&pubv, ROOT_KEYS, "root");
    assert_eq!(pubv["schema_version"], 1);
    assert_eq!(pubv["game_id"], "sanguosha");
    assert_keys(&pubv["public_board"], BOARD_KEYS, "public_board");
    assert_eq!(pubv["special_zones"], serde_json::json!({}));

    let seat = &pubv["public_board"]["seats"][0];
    assert_keys(seat, SEAT_KEYS, "public_board.seats[0]");
    assert_keys(&seat["equips"], EQUIP_SLOTS, "equips");
    // v1 契约：公共视图 player_area = null。
    assert!(pubv["player_area"].is_null(), "公共视图 player_area 应为 null");
    assert_keys(&pubv["public_board"]["log"][0], LOG_KEYS, "log[0]");
}

#[test]
fn view_contract_card_and_pending_shapes() {
    let mut f = fixture();
    let pubv = view::public_state(&f.g);
    assert_keys(&pubv["public_board"]["discard"][0], CARD_KEYS, "discard card");
    assert_keys(
        &pubv["public_board"]["seats"][0]["equips"]["weapon"],
        CARD_KEYS,
        "equip card",
    );
    assert_keys(
        &pubv["public_board"]["seats"][1]["judge_zone"][0],
        CARD_KEYS,
        "judge card",
    );

    // 触发一个 pending（p0 对 p1 使用【杀】，要求 p1 出【闪】）。
    let sha = f
        .hand0
        .iter()
        .find(|id| f.g.def_of(id).tags.sha)
        .cloned()
        .unwrap_or_else(|| {
            let id = f
                .g
                .ids_in(|z| *z == Zone::Draw)
                .into_iter()
                .find(|id| f.g.def_of(id).tags.sha)
                .unwrap();
            f.g.to_hand(&id, 0).unwrap();
            id
        });
    let shan = f
        .g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| f.g.def_of(id).tags.shan)
        .unwrap();
    f.g.to_hand(&shan, 1).unwrap();
    begin_phase(&mut f.g, 0, Phase::Play).unwrap();
    f.g.resolve_use(0, &sha, vec![1]).unwrap();
    assert!(f.g.pending.is_some(), "应等待 p1 出闪");

    let pubv = view::public_state(&f.g);
    let pp = &pubv["public_board"]["pending"];
    assert_keys(pp, PENDING_PUBLIC_KEYS, "public pending");
    assert!(pp.get("options").is_none(), "公共 pending 不得带 options");
    assert!(pp.get("data").is_none(), "公共 pending 不得带 data");

    let p1v = view::player_view(&f.g, 1);
    assert_keys(&p1v["player_area"]["pending"], PENDING_PRIVATE_KEYS, "private pending");
}

#[test]
fn view_contract_presentation_shape() {
    let mut f = fixture();
    begin_phase(&mut f.g, 0, Phase::Play).unwrap();
    let tao = f
        .g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| f.g.def_of(id).tags.tao)
        .unwrap();
    f.g.to_hand(&tao, 0).unwrap();
    f.g.seats[0].hp = 2;
    f.g.resolve_use(0, &tao, vec![]).unwrap();
    let pubv = view::public_state(&f.g);
    let pres = &pubv["public_board"]["presentation"];
    assert!(!pres.as_array().unwrap().is_empty(), "应有一条展示");
    assert_keys(&pres[0], PRESENTATION_KEYS, "presentation[0]");
    assert!(
        pres
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["text"].as_str().unwrap_or("").contains("回复")),
        "应有一条回血展示：{pres}"
    );
}

// ---------------------------------------------------------------------------
// 9 类区投影 + 隐藏信息
// ---------------------------------------------------------------------------

#[test]
fn view_contract_nine_zones_are_projected() {
    let mut f = fixture();
    let pubv = view::public_state(&f.g);
    let board = &pubv["public_board"];
    assert!(board["draw_count"].as_u64().unwrap() > 0, "牌堆只给数量");
    assert_eq!(board["void_count"], 1);
    assert_eq!(board["discard"].as_array().unwrap().len(), 1);
    assert_eq!(board["seats"][0]["hand_count"], 2);
    assert_eq!(
        board["seats"][0]["equips"]["weapon"]["id"],
        f.equip0.as_str()
    );
    assert_eq!(board["seats"][1]["judge_zone"][0]["id"], f.judge1.as_str());
    assert_eq!(
        board["seats"][0]["marks"]["kongsheng"][0]["id"],
        f.mark0.as_str()
    );

    // Processing / Frame 区：公共只给数量。
    let tmp = f
        .g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    f.g.to_processing(&tmp).unwrap();
    let pubv = view::public_state(&f.g);
    assert_eq!(pubv["public_board"]["processing_count"], 1);
    f.g.to_frame(&tmp).unwrap();
    let pubv = view::public_state(&f.g);
    assert_eq!(pubv["public_board"]["frame_count"], 1);
    land(&mut f.g, &tmp, Zone::Discard).unwrap();
    let pubv = view::public_state(&f.g);
    assert_eq!(pubv["public_board"]["frame_count"], 0);
}

#[test]
fn view_contract_hand_and_identity_hidden_from_public() {
    let f = fixture();
    let pubv = view::public_state(&f.g);
    // v1 契约：公共视图 player_area 为 null（最严格的「无私有信息」）。
    assert!(pubv["player_area"].is_null(), "公共视图无 player_area");
    // v1 兼容：非主公身份字段存在但为 null（只有主公/已死亡公开）。
    assert!(
        pubv["public_board"]["seats"][0]["identity"].is_null(),
        "公共座位不得公开非主公身份"
    );
    // 牌堆顺序不暴露：只有 draw_count。
    assert!(
        pubv["public_board"].get("draw").is_none(),
        "不得暴露牌堆顺序"
    );

    let p0v = view::player_view(&f.g, 0);
    let hand = p0v["player_area"]["hand"].as_array().unwrap();
    assert_eq!(hand.len(), 2);
    assert_eq!(hand[0]["id"], f.hand0[0].as_str());
    assert_eq!(p0v["player_area"]["identity"], "rebel");

    let p1v = view::player_view(&f.g, 1);
    let p1hand = p1v["player_area"]["hand"].as_array().unwrap();
    assert_eq!(p1hand.len(), 1);
    assert_ne!(p1hand[0]["id"], f.hand0[0].as_str(), "看不到他人手牌");
}

#[test]
fn view_contract_private_reveal_only_for_viewer() {
    let mut f = fixture();
    let shown = f.hand0[0].clone();
    f.g.reveal_private(1, &shown, "test");
    let p1v = view::player_view(&f.g, 1);
    assert_eq!(
        p1v["player_area"]["private_reveals"][0]["id"],
        shown.as_str()
    );
    let p0v = view::player_view(&f.g, 0);
    assert!(p0v["player_area"]["private_reveals"]
        .as_array()
        .unwrap()
        .is_empty());
    let pubv = view::public_state(&f.g);
    assert!(pubv["public_board"]["revealed"]
        .as_array()
        .unwrap()
        .is_empty(), "私密展示不进公共区");

    // 公共展示 → 进公共区 + card.revealed=true。
    f.g.reveal(&shown, Some(0), "public");
    let pubv = view::public_state(&f.g);
    assert_eq!(pubv["public_board"]["revealed"][0]["id"], shown.as_str());
    assert_eq!(pubv["public_board"]["revealed"][0]["revealed"], true);
}

#[test]
fn view_contract_transforms_and_abolished_slots() {
    let mut f = fixture();
    f.g.seats[0].abolished_slots.insert(Slot::Armor);
    f.g.seats[0].judge_abolished = true;
    let pubv = view::public_state(&f.g);
    let seat = &pubv["public_board"]["seats"][0];
    assert_eq!(seat["abolished_slots"][0], "armor");
    assert_eq!(seat["judge_abolished"], true);

    // 转换技：卢植的贞良 → 公共区出现 yang/yin，且随 `g.yang/toggle_yang` 变化。
    let seats = vec![
        Seat::new("p0", "luzhi", &["zhenliang"], 4),
        Seat::new("p1", "guest", &[], 4),
    ];
    let mut tg = Game::new(seats, cards::deck());
    let pubv = view::public_state(&tg);
    assert_eq!(pubv["public_board"]["seats"][0]["transforms"]["zhenliang"], "yang");
    tg.toggle_yang(0, "zhenliang");
    let pubv = view::public_state(&tg);
    assert_eq!(pubv["public_board"]["seats"][0]["transforms"]["zhenliang"], "yin");
}

#[test]
fn view_contract_judge_zone_lifo_order() {
    let mut f = fixture();
    let second = f
        .g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .find(|id| f.g.def_of(id).tags.delayed)
        .unwrap();
    f.g.to_judge(&second, 1).unwrap();
    let pubv = view::public_state(&f.g);
    let zone = pubv["public_board"]["seats"][1]["judge_zone"]
        .as_array()
        .unwrap();
    assert_eq!(zone.len(), 2);
    // 视图按判定区 LIFO（后置先判）排列：second 在前。
    assert_eq!(zone[0]["id"], second.as_str());
    assert_eq!(zone[1]["id"], f.judge1.as_str());
}

#[test]
fn view_contract_alive_and_card_revealed_flags() {
    let mut f = fixture();
    // I2：存活是显式状态（死亡确认），不由 hp 推导。
    f.g.seats[2].hp = 0;
    f.g.seats[2].alive = false;
    let id = f
        .g
        .ids_in(|z| *z == Zone::Draw)
        .into_iter()
        .next()
        .unwrap();
    f.g.reveal(&id, None, "test");
    let pubv = view::public_state(&f.g);
    assert_eq!(pubv["public_board"]["seats"][2]["alive"], false);
    let found = pubv["public_board"]["revealed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == id.as_str() && c["revealed"] == true);
    assert!(found);
}
