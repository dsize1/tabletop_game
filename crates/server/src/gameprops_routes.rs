//! 道具中台内部 API（P1）：/internal/gameprops/*。
//!
//! 单写者：只经 server 写 SQLite；SPA 不直连库。
//! 鉴权：`PROPS_ADMIN_TOKEN` 存在时要求 `X-Props-Admin-Token`；未设置时仅限本机
//! （server 只监听 127.0.0.1）。完整的游戏权鉴在 P8。

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

pub(crate) type ApiError = (StatusCode, Json<Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

/// 共享 token（web → server 内部凭证；未配置 = 本机开发放行）。
fn shared_token_ok(state: &AppState, headers: &HeaderMap) -> bool {
    let expected = state.props_admin_token.as_deref().filter(|s| !s.is_empty());
    match expected {
        Some(expected) => {
            headers
                .get("x-props-admin-token")
                .and_then(|v| v.to_str().ok())
                == Some(expected)
        }
        None => true,
    }
}

fn header_actor(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-props-actor")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// 严格管理员准入（ADR-0031 / ADR-0035）：
/// - 有 `x-props-actor` 时**只按 actor 的权限范围**判定（即使同时带了共享 token）；
///   Admin 放行，Workbench / 无权限拒绝（防止测试玩家借内部 token 越权）。
/// - 无 actor 时走共享 token / 本机开发放行（脚本、agent-gateway 内部调用）。
pub(crate) fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if let Some(actor) = header_actor(headers) {
        return match crate::web_auth_routes::props_scope(state, actor) {
            Some(crate::web_auth_routes::PropsScope::Admin) => Ok(()),
            Some(crate::web_auth_routes::PropsScope::Workbench) => Err(err(
                StatusCode::FORBIDDEN,
                "当前账号仅有工作台权限",
            )),
            None => Err(err(StatusCode::UNAUTHORIZED, "无道具中台权限")),
        };
    }
    if shared_token_ok(state, headers) {
        Ok(())
    } else {
        Err(err(StatusCode::UNAUTHORIZED, "缺少或错误的道具中台凭证"))
    }
}

/// 工作台准入：Admin 或 Workbench 均可；返回实际范围。
pub(crate) fn require_workbench(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::web_auth_routes::PropsScope, ApiError> {
    if let Some(actor) = header_actor(headers) {
        return crate::web_auth_routes::props_scope(state, actor)
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "无道具中台权限"));
    }
    if shared_token_ok(state, headers) {
        Ok(crate::web_auth_routes::PropsScope::Admin)
    } else {
        Err(err(StatusCode::UNAUTHORIZED, "缺少或错误的道具中台凭证"))
    }
}

/// 当前会话信息（SPA 用来裁剪菜单 / 路由守卫）。
pub async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let scope = require_workbench(&state, &headers)?;
    Ok(Json(json!({
        "openid": actor(&headers),
        "scope": scope.code(),
        "menus": scope.menus(),
    })))
}

pub(crate) fn actor(headers: &HeaderMap) -> String {
    headers
        .get("x-props-actor")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("admin")
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub table: String,
    pub entity_code: String,
    pub to_status: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// `GET /internal/gameprops/summary`
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    state
        .storage
        .gameprops_summary()
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("汇总失败: {e}")))
}

/// `GET /internal/gameprops/{table}?status=&q=&limit=&offset=`
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(table): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let rows = state
        .storage
        .list_gameprops(
            &table,
            q.status.as_deref(),
            q.q.as_deref(),
            q.limit.unwrap_or(100),
            q.offset.unwrap_or(0),
        )
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?;
    Ok(Json(json!({ "table": table, "rows": rows })))
}

/// `POST /internal/gameprops/{table}`：upsert 一行（JSON 对象）。
pub async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(table): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let obj = body
        .as_object()
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "请求体必须是 JSON 对象"))?;
    let row = state
        .storage
        .upsert_gameprops(&table, obj)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("写入失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

/// `POST /internal/gameprops/status`：生命周期流转 + 审核日志。
pub async fn set_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<StatusBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    const STATUSES: &[&str] = &["draft", "testing", "review", "live", "offline"];
    if !STATUSES.contains(&body.to_status.as_str()) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("非法状态 {}", body.to_status),
        ));
    }
    let row = state
        .storage
        .set_gameprops_status(
            &body.table,
            &body.entity_code,
            &body.to_status,
            &actor(&headers),
            body.note.as_deref(),
        )
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("状态流转失败: {e}")))?;
    Ok(Json(json!({ "row": row })))
}

/// `POST /internal/gameprops/seed`：初始化身份/配比/包（幂等）。
pub async fn seed(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let result = state.storage.seed_sgs_defaults().map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("初始化失败: {e}"),
        )
    })?;
    Ok(Json(json!({ "seeded": result })))
}

/// `POST /internal/gameprops/import/heroes`：读取 heroes.json 并幂等导入。
pub async fn import_heroes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let path = state.props_data_dir.join("heroes.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取 {} 失败: {e}", path.display()),
        )
    })?;
    let payload: Value = serde_json::from_str(&text).map_err(|e| {
        err(
            StatusCode::BAD_REQUEST,
            format!("heroes.json 解析失败: {e}"),
        )
    })?;
    let result = state
        .storage
        .import_sgs_heroes(&payload)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("导入失败: {e}")))?;
    Ok(Json(
        json!({ "imported": result, "source": path.display().to_string() }),
    ))
}

/// 从**引擎静态数据**同步牌定义 / 物理牌 / 武将 / 技能 / 关联（中台可见「已制作」）。
///
/// 口径（2026-11-14 起，ADR-0043）：引擎 **V3**（`crates/engines/sanguosha3`）是唯一真相：
/// - 牌定义/物理牌来自生成物 `cards_generated::{DEFS, DECK}`（160 张）；
/// - 武将来自生成物 `generals_generated::GENERALS`（232 名录：**147 live + 85 offline** + 0 review）；
///   重名（界化）原版由生成器标 `Offline`（与引擎选将池同口径）；
/// - 技能来自 V3 注册表 `kernel::skills::all()`（按 B0–B20 批次接入，**只登记已实现**技能）。
/// 官网原始数据（`heroes.json`，696 候选）仍走 `/import/heroes`（draft），两者互补。
/// 幂等：牌定义/武将/技能按 code，物理牌按保留 id 段重写。
pub async fn import_engine(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    use sanguosha3::cards_generated::{DECK, DEFS};
    use sanguosha3::generals_generated::{Status, GENERALS};
    use sanguosha3::kernel::cards::{Category, TargetMode};
    use sanguosha3::kernel::Slot;

    require_admin(&state, &headers)?;

    fn upsert(
        state: &AppState,
        table: &str,
        row: serde_json::Map<String, Value>,
    ) -> Result<Value, ApiError> {
        state.storage.upsert_gameprops(table, &row).map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("写入 {table} 失败: {e}"),
            )
        })
    }

    fn pack_id(
        state: &AppState,
        kind: &str,
        code: &str,
        name: &str,
        sort: i64,
    ) -> Result<i64, ApiError> {
        let mut row = serde_json::Map::new();
        row.insert("game_id".into(), json!("sanguosha"));
        row.insert("kind".into(), json!(kind));
        row.insert("code".into(), json!(code));
        row.insert("name".into(), json!(name));
        row.insert("sort_order".into(), json!(sort));
        row.insert("enabled".into(), json!(1));
        let out = upsert(state, "prop_packs", row)?;
        Ok(out.get("id").and_then(Value::as_i64).unwrap_or(0))
    }

    // 1) 包（引擎 key 与中台 code 保持一致）
    let general_packs: [(&str, &str, i64); 4] = [
        ("standard", "标准", 10),
        ("wind_fire_forest_mountain", "风林火山", 20),
        ("yijiang", "一将成名", 30),
        ("jiexian", "界限突破", 40),
    ];
    let mut gpack_ids: std::collections::HashMap<&str, i64> = Default::default();
    for (code, name, sort) in general_packs {
        gpack_ids.insert(code, pack_id(&state, "general", code, name, sort)?);
    }
    let card_packs: [(&str, &str, i64); 2] =
        [("standard", "标准牌堆", 10), ("junzheng", "军争牌堆", 20)];
    let mut cpack_ids: std::collections::HashMap<&str, i64> = Default::default();
    for (code, name, sort) in card_packs {
        cpack_ids.insert(code, pack_id(&state, "card", code, name, sort)?);
    }

    // 2) 牌定义（V3 生成物 44 条，含 木牛流马）
    let mut card_defs = 0usize;
    for r in DEFS {
        let category = match r.category {
            Category::Basic => "basic",
            Category::Trick => "trick",
            Category::DelayedTrick => "delayed_trick",
            Category::Equip => match r.slot {
                Some(Slot::Weapon) => "weapon",
                Some(Slot::Armor) => "armor",
                Some(Slot::Treasure) => "treasure",
                _ => "horse",
            },
        };
        let equip_slot = match r.slot {
            Some(Slot::Weapon) => Some("weapon"),
            Some(Slot::Armor) => Some("armor"),
            Some(Slot::HorseOffense) => Some("horse_offense"),
            Some(Slot::HorseDefense) => Some("horse_defense"),
            Some(Slot::Treasure) => Some("treasure"),
            None => None,
        };
        // 中台约束：`range_type` 只对坐骑有值（武器只记 `range_value`）。
        let (range_value, range_type) = match r.slot {
            Some(Slot::HorseOffense) => (r.horse, Some("attack")),
            Some(Slot::HorseDefense) => (r.horse, Some("defense")),
            Some(Slot::Weapon) => (r.weapon_range, None),
            _ => (None, None),
        };
        let target_mode = match r.target_mode {
            TargetMode::AllAlive => Some("aoe"),
            TargetMode::Chosen | TargetMode::Single => Some("single"),
            TargetMode::SelfOnly | TargetMode::None => None,
        };
        let mut row = serde_json::Map::new();
        row.insert("code".into(), json!(r.code));
        row.insert("name".into(), json!(r.name));
        row.insert("category".into(), json!(category));
        row.insert("description".into(), json!(""));
        row.insert("range_value".into(), json!(range_value));
        row.insert("target_mode".into(), json!(target_mode));
        row.insert("equip_slot".into(), json!(equip_slot));
        row.insert("range_type".into(), json!(range_type));
        row.insert("status".into(), json!("live"));
        upsert(&state, "sgs_card_defs", row)?;
        card_defs += 1;
    }

    // 3) 物理牌（V3 生成物 160 张；保留 id 段重写）
    const ENGINE_CARD_ID_BASE: i64 = 1_000_000;
    state
        .storage
        .exec_raw(&format!("DELETE FROM sgs_cards WHERE id >= {ENGINE_CARD_ID_BASE}"))
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("清理旧同步牌失败: {e}")))?;
    let mut cards = 0usize;
    for c in DECK {
        let Some(pid) = cpack_ids.get(c.pack) else {
            continue;
        };
        let def_code = sanguosha3::kernel::cards::code_of(c.name).unwrap_or(c.name);
        let suit = match c.suit {
            sanguosha3::kernel::Suit::Spade => "spade",
            sanguosha3::kernel::Suit::Heart => "heart",
            sanguosha3::kernel::Suit::Club => "club",
            sanguosha3::kernel::Suit::Diamond => "diamond",
        };
        let mut row = serde_json::Map::new();
        row.insert("id".into(), json!(ENGINE_CARD_ID_BASE + cards as i64));
        row.insert("pack_id".into(), json!(pid));
        row.insert("def_code".into(), json!(def_code));
        row.insert("suit".into(), json!(suit));
        row.insert("rank".into(), json!(c.rank));
        row.insert("status".into(), json!("live"));
        upsert(&state, "sgs_cards", row)?;
        cards += 1;
    }

    // 4) 武将 + 技能 + 关联（V3 生成物名录 + V3 技能注册表）
    let mut generals = 0usize;
    let mut skills_n = 0usize;
    let mut links = 0usize;
    // 技能中文名 → code（V3 注册表 = 已实现技能的唯一来源；未接入的技能不登记）。
    let mut skill_codes: std::collections::HashMap<&'static str, &'static str> = Default::default();
    for def in sanguosha3::kernel::skills::all() {
        skill_codes.insert(def.name, def.code);
    }
    for g in GENERALS {
        let Some(pid) = gpack_ids.get(g.pack) else {
            continue;
        };
        let status = match g.status {
            Status::Live => "live",
            Status::Offline => "offline",
            Status::Review => "review",
        };
        let mut row = serde_json::Map::new();
        row.insert("code".into(), json!(g.code));
        row.insert(
            "external_id".into(),
            json!(format!("sanguosha.com:hero:{}", g.gid)),
        );
        row.insert("name".into(), json!(g.name));
        row.insert("kingdom".into(), json!(g.kingdom));
        row.insert("gender".into(), json!(if g.male { "male" } else { "female" }));
        row.insert("pack_id".into(), json!(pid));
        row.insert("max_hp".into(), json!(g.max_hp));
        row.insert("initial_hp".into(), json!(0));
        row.insert("description".into(), json!(""));
        row.insert("biography".into(), json!(""));
        row.insert("status".into(), json!(status));
        upsert(&state, "sgs_generals", row)?;
        generals += 1;

        for (idx, name) in g.skills.iter().enumerate() {
            // 只登记 V3 注册表里**已实现**的技能（未接入批次的不进中台「已制作」）。
            let Some(code) = skill_codes.get(name).copied() else {
                continue;
            };
            if let Some(def) = sanguosha3::kernel::skills::def(code) {
                let kind = match def.kind {
                    sanguosha3::kernel::skills::SkillKind::Normal => "normal",
                    sanguosha3::kernel::skills::SkillKind::Locked => "locked",
                    sanguosha3::kernel::skills::SkillKind::Limited => "limited",
                    sanguosha3::kernel::skills::SkillKind::Awaken => "awaken",
                    sanguosha3::kernel::skills::SkillKind::Lord => "lord",
                    sanguosha3::kernel::skills::SkillKind::Equip => "equip",
                };
                let mut srow = serde_json::Map::new();
                srow.insert("code".into(), json!(def.code));
                srow.insert("name".into(), json!(def.name));
                srow.insert("type".into(), json!(kind));
                srow.insert("description".into(), json!(""));
                srow.insert("status".into(), json!("live"));
                upsert(&state, "sgs_skills", srow)?;
                skills_n += 1;
            }

            let mut link = serde_json::Map::new();
            link.insert("general_code".into(), json!(g.code));
            link.insert("skill_code".into(), json!(code));
            link.insert("sort_order".into(), json!(idx as i64));
            upsert(&state, "sgs_general_skills", link)?;
            links += 1;
        }
    }

    Ok(Json(json!({
        "imported": {
            "card_defs": card_defs,
            "cards": cards,
            "generals": generals,
            "skills": skills_n,
            "general_skills": links,
        },
        "source": "engine v3: crates/engines/sanguosha3/{cards_generated,generals_generated,kernel::skills}",
    })))
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 兼容：`GET /internal/gameprops/{table}/{code}` 单行查询（便于关系页）。
pub async fn get_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((table, code)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    if !storage::gameprops::is_table(&table) {
        return Err(err(StatusCode::BAD_REQUEST, format!("未知表 {table}")));
    }
    let rows = state
        .storage
        .list_gameprops(&table, None, Some(&code), 1, 0)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("查询失败: {e}")))?;
    let row = rows.into_iter().next().unwrap_or(Value::Null);
    Ok(Json(json!({ "row": row })))
}
