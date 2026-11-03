//! Web 跨端权鉴 HTTP API（ADR-0031）。
//!
//! 角色划分：
//! - `agent-gateway` 调 **`/internal/auth/web/link`** 申请短效登录链接，调 **`/internal/auth/web/verify`** 回执群内验证码结果。
//! - `apps/web`（登录页）调 `/auth/web/status|register|login|confirm`，确认成功后拿 8h token 写 Cookie。
//! - `/internal/*` 仅本机（server 监听 127.0.0.1，不经 web/tunnel）。
//!
//! 关键语义（用户确认口径）：
//! - 短效会话 = token + 状态 + 4 位码 + 目标路由，**同生命周期**；
//! - 注册码**只在群内验证码确认成功后**才落库（R1）；
//! - 确认后一次性换票（`consumed`），二次访问返回 `used`；
//! - 注册码错误 3 次锁定 30 分钟。

use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use auth::weblogin::{
    hash_passcode, is_valid_passcode, normalize_code, ConfirmOutcome, WebLoginRecord,
    STAGE_CODE, STAGE_CONFIRMED, STAGE_NEED_LOGIN, STAGE_NEED_REGISTER,
};

use crate::AppState;

type ApiError = (StatusCode, Json<Value>);

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

/// 统一的注册码错误文案：不区分「未注册 / 注册码错误」。
const PASSCODE_REJECTED: &str = "注册码不正确";

/// 中台权限范围（ADR-0035）：
/// - Admin：开发/中台白名单，全部菜单；
/// - Workbench：测试玩家白名单，仅工作台菜单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropsScope {
    Admin,
    Workbench,
}

impl PropsScope {
    pub(crate) fn code(self) -> &'static str {
        match self {
            PropsScope::Admin => "admin",
            PropsScope::Workbench => "workbench",
        }
    }

    pub(crate) fn menus(self) -> Vec<&'static str> {
        match self {
            PropsScope::Admin => vec![
                "dashboard",
                "packs",
                "identities",
                "identity-setup",
                "card-defs",
                "cards",
                "card-relation",
                "generals",
                "skills",
                "general-skills",
                "workbench",
                "import",
            ],
            PropsScope::Workbench => vec!["workbench"],
        }
    }
}

/// 解析 openid 的中台权限范围；`None` = 无中台权限。
///
/// 优先级：DEV_WHITELIST / PROPS_ADMIN_WHITELIST（Admin）> TEST_PLAYER_WHITELIST（Workbench）。
pub(crate) fn props_scope(state: &AppState, openid: &str) -> Option<PropsScope> {
    if state.props_whitelist.contains(openid) || state.dev_whitelist.contains(openid) {
        return Some(PropsScope::Admin);
    }
    if state.test_player_whitelist.contains(openid) {
        return Some(PropsScope::Workbench);
    }
    None
}

fn is_props_whitelisted(state: &AppState, openid: &str) -> bool {
    props_scope(state, openid).is_some()
}

// ==================== 网关侧（internal） ====================

#[derive(Debug, Deserialize)]
pub struct LinkRequest {
    pub group_id: String,
    pub openid: String,
    #[serde(default)]
    pub display_name: String,
    /// `game` / `props`
    pub scope: String,
    /// 确认后跳转的站内路由（web 侧只允许相对路径）
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub room_id: Option<String>,
    /// `login` / `reset`
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LinkResponse {
    pub token: String,
    pub expires_in_secs: u64,
    pub stage: String,
    pub registered: bool,
}

/// 申请短效登录链接（网关调用）。scope=props 时要求白名单。
pub async fn link(
    State(state): State<AppState>,
    Json(req): Json<LinkRequest>,
) -> Result<Json<LinkResponse>, ApiError> {
    if req.group_id.trim().is_empty() || req.openid.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "缺少 group_id / openid"));
    }
    let scope = req.scope.as_str();
    if scope != "game" && scope != "props" {
        return Err(err(StatusCode::BAD_REQUEST, "scope 只能是 game / props"));
    }
    if scope == "props" && !is_props_whitelisted(&state, &req.openid) {
        return Err(err(StatusCode::FORBIDDEN, "无中台权限"));
    }
    let mode = match req.mode.as_deref() {
        None | Some("") | Some("login") => auth::weblogin::MODE_LOGIN,
        Some("reset") => auth::weblogin::MODE_RESET,
        Some(other) => return Err(err(StatusCode::BAD_REQUEST, format!("未知 mode：{other}"))),
    };
    let target = sanitize_target(&req.target);
    if target.is_none() {
        return Err(err(StatusCode::BAD_REQUEST, "target 必须是站内相对路径"));
    }
    let mut target = target.unwrap();
    // 测试玩家只有工作台权限：即使网关/旧链接给了别的 target，也强制落到工作台。
    if scope == "props"
        && matches!(props_scope(&state, &req.openid), Some(PropsScope::Workbench))
    {
        target = "/props-admin/workbench".to_string();
    }

    let account = state
        .storage
        .get_web_account(&req.group_id, &req.openid)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取账号失败"))?;
    let registered = account.is_some();
    if mode == auth::weblogin::MODE_RESET && !registered {
        return Err(err(StatusCode::BAD_REQUEST, "尚未注册，无需重置"));
    }
    if scope == "game" && req.room_id.is_none() {
        return Err(err(StatusCode::BAD_REQUEST, "scope=game 需要 room_id"));
    }

    let mut rec = WebLoginRecord::new(
        &req.group_id,
        &req.openid,
        &req.display_name,
        scope,
        &target,
        req.room_id.as_deref(),
        mode,
        registered,
    );
    if mode == auth::weblogin::MODE_RESET {
        // 重置：直接进入「设置新注册码」表单（仍复用 need_register 视图）。
        rec.stage = STAGE_NEED_REGISTER.to_string();
    }

    let (token, ttl) = state
        .weblogin
        .create(&rec)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "创建登录会话失败"))?;
    Ok(Json(LinkResponse {
        token,
        expires_in_secs: ttl,
        stage: rec.stage,
        registered,
    }))
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub group_id: String,
    pub code: String,
    pub openid: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    /// `confirmed` / `mismatch` / `unknown`
    pub result: String,
}

/// 群内 `#验证码#NNNN` 判定：码 + 回复人 openid。
pub async fn verify(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, ApiError> {
    let Some(code) = normalize_code(&req.code) else {
        return Ok(Json(VerifyResponse {
            result: "unknown".into(),
        }));
    };
    match state
        .weblogin
        .confirm(&req.group_id, &code, &req.openid)
        .await
    {
        Ok(ConfirmOutcome::Confirmed(_)) => Ok(Json(VerifyResponse {
            result: "confirmed".into(),
        })),
        Ok(ConfirmOutcome::Mismatch) => Ok(Json(VerifyResponse {
            result: "mismatch".into(),
        })),
        Ok(ConfirmOutcome::Unknown) => Ok(Json(VerifyResponse {
            result: "unknown".into(),
        })),
        Err(_) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, "校验失败")),
    }
}

// ==================== 登录页（web 调用） ====================

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
    /// 可选：本次长轮询的最长等待秒数（0 = 立即返回当前状态；用于测试/页面首屏探测）。
    #[serde(default)]
    pub wait: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub stage: String,
    pub consumed: bool,
    pub registered: bool,
    pub openid: String,
    pub group_id: String,
    pub display_name: String,
    pub scope: String,
    pub target: String,
    pub mode: String,
    pub room_id: Option<String>,
    /// 验证码（stage=code 时返回，供页面刷新后继续展示）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// 开局信息（scope=game）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<Value>,
    pub attempts_left: u32,
    /// 注册码锁定剩余秒数（未锁定不返回）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_secs: Option<u64>,
    pub expires_in_secs: u64,
}

/// 会话状态：页面进入/刷新都调它（同时可判定「是否已确认 / 是否已换票」）。
pub async fn status(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
) -> Result<Json<StatusResponse>, ApiError> {
    let Some(rec) = state
        .weblogin
        .get(&q.token)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取会话失败"))?
    else {
        return Err(err(StatusCode::UNAUTHORIZED, "链接已失效，请在群内重新获取"));
    };
    let account = state
        .storage
        .get_web_account(&rec.group_id, &rec.openid)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取账号失败"))?;
    let registered = account.is_some();
    let locked_secs = state.weblogin.lock_remaining(&rec.group_id, &rec.openid).await;
    let game = rec
        .room_id
        .as_deref()
        .and_then(|room| state.games.state_summary(room).ok())
        .map(|summary| json!({ "room_id": rec.room_id, "summary": summary }));

    Ok(Json(StatusResponse {
        stage: rec.stage.clone(),
        consumed: rec.consumed,
        registered,
        openid: rec.openid.clone(),
        group_id: rec.group_id.clone(),
        display_name: rec.display_name.clone(),
        scope: rec.scope.clone(),
        target: rec.target.clone(),
        mode: rec.mode.clone(),
        room_id: rec.room_id.clone(),
        code: if rec.stage == STAGE_CODE || rec.stage == STAGE_CONFIRMED {
            Some(rec.code.clone())
        } else {
            None
        },
        game,
        attempts_left: auth::weblogin::MAX_FAILS.saturating_sub(rec.attempts),
        locked_secs,
        expires_in_secs: state.weblogin.ttl_secs(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct PasscodeRequest {
    pub token: String,
    pub passcode: String,
    /// 第二次输入（注册/重置时必填，前后端都要一致）
    #[serde(default)]
    pub passcode_confirm: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CodeResponse {
    pub stage: String,
    pub code: String,
    pub expires_in_secs: u64,
}

/// 注册（或重置）注册码：**只暂存哈希**，群内确认成功后才落库（R1）。
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<PasscodeRequest>,
) -> Result<Json<CodeResponse>, ApiError> {
    let Some(mut rec) = state
        .weblogin
        .get(&req.token)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取会话失败"))?
    else {
        return Err(err(StatusCode::UNAUTHORIZED, "链接已失效，请在群内重新获取"));
    };
    let is_reset = rec.mode == auth::weblogin::MODE_RESET;
    if !is_reset && rec.stage != STAGE_NEED_REGISTER {
        return Err(err(StatusCode::BAD_REQUEST, "当前不需要注册"));
    }
    if let Some(locked) = state.weblogin.lock_remaining(&rec.group_id, &rec.openid).await {
        return Err(err(
            StatusCode::LOCKED,
            format!("注册码错误次数过多，请 {locked} 秒后再试"),
        ));
    }
    if !is_valid_passcode(&req.passcode) {
        return Err(err(StatusCode::BAD_REQUEST, "注册码需为 6 位 0-9a-zA-Z"));
    }
    if req.passcode_confirm.as_deref() != Some(req.passcode.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "两次输入的注册码不一致"));
    }
    let hash = hash_passcode(&req.passcode)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "注册码处理失败"))?;
    rec.pending_hash = Some(hash);
    let (_, code) = state
        .weblogin
        .issue_code(&req.token, rec, &mut rand::rngs::StdRng::from_entropy())
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "生成验证码失败"))?;
    Ok(Json(CodeResponse {
        stage: STAGE_CODE.to_string(),
        code,
        expires_in_secs: state.weblogin.ttl_secs(),
    }))
}

/// 用既有注册码登录 → 生成 4 位验证码。
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<PasscodeRequest>,
) -> Result<Json<CodeResponse>, ApiError> {
    let Some(mut rec) = state
        .weblogin
        .get(&req.token)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取会话失败"))?
    else {
        return Err(err(StatusCode::UNAUTHORIZED, "链接已失效，请在群内重新获取"));
    };
    if rec.stage != STAGE_NEED_LOGIN {
        return Err(err(StatusCode::BAD_REQUEST, "当前不是登录阶段"));
    }
    if let Some(locked) = state.weblogin.lock_remaining(&rec.group_id, &rec.openid).await {
        return Err(err(
            StatusCode::LOCKED,
            format!("注册码错误次数过多，请 {locked} 秒后再试"),
        ));
    }
    let account = state
        .storage
        .get_web_account(&rec.group_id, &rec.openid)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取账号失败"))?
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, PASSCODE_REJECTED))?;
    if !auth::weblogin::verify_passcode(&account.passcode_hash, &req.passcode) {
        let fails = state
            .weblogin
            .note_failure(&rec.group_id, &rec.openid)
            .await;
        rec.attempts = fails;
        let _ = state
            .weblogin
            .save(&req.token, &rec, state.weblogin.ttl_secs())
            .await;
        let left = auth::weblogin::MAX_FAILS.saturating_sub(fails);
        let msg = if left == 0 {
            "注册码错误次数过多，账号已锁定 30 分钟".to_string()
        } else {
            format!("{PASSCODE_REJECTED}（还可尝试 {left} 次）")
        };
        return Err(err(StatusCode::UNAUTHORIZED, msg));
    }
    state
        .weblogin
        .clear_failures(&rec.group_id, &rec.openid)
        .await;
    let (_, code) = state
        .weblogin
        .issue_code(&req.token, rec, &mut rand::rngs::StdRng::from_entropy())
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "生成验证码失败"))?;
    Ok(Json(CodeResponse {
        stage: STAGE_CODE.to_string(),
        code,
        expires_in_secs: state.weblogin.ttl_secs(),
    }))
}

/// 长轮询确认（≤ `WEB_CONFIRM_POLL_SECS`，默认 20s）。
///
/// 返回：
/// - `pending`：还没在群里确认；
/// - `confirmed`：返回一次性 8h token（并把 pending 的注册码落库）；
/// - `used`：已换过票（页面若已有有效 Cookie 可直接跳转）；
/// - `expired`：链接过期。
pub async fn confirm(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
) -> Result<Json<Value>, ApiError> {
    let poll_secs = q
        .wait
        .or_else(|| {
            std::env::var("WEB_CONFIRM_POLL_SECS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
        })
        .unwrap_or(20)
        .min(60);
    let deadline = std::time::Instant::now() + Duration::from_secs(poll_secs);
    loop {
        let Some(rec) = state
            .weblogin
            .get(&q.token)
            .await
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "读取会话失败"))?
        else {
            return Ok(Json(json!({ "status": "expired" })));
        };
        if rec.consumed {
            return Ok(Json(json!({
                "status": "used",
                "target": rec.target,
                "openid": rec.openid,
                "group_id": rec.group_id,
            })));
        }
        if rec.stage == STAGE_CONFIRMED {
            return Ok(Json(finish_confirm(&state, &q.token, rec).await?));
        }
        if std::time::Instant::now() >= deadline {
            return Ok(Json(json!({ "status": "pending" })));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// 确认通过后的收尾：落库注册码（R1）→ 签 8h token → 会话置 consumed。
async fn finish_confirm(
    state: &AppState,
    token: &str,
    rec: WebLoginRecord,
) -> Result<Value, ApiError> {
    if let Some(hash) = rec.pending_hash.as_deref() {
        state
            .storage
            .upsert_web_account(&rec.group_id, &rec.openid, hash, "argon2id")
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "保存注册码失败"))?;
    }
    state
        .storage
        .touch_web_login(&rec.group_id, &rec.openid)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "更新登录时间失败"))?;
    // 重置注册码：确认即生效，但**不换票、不跳转**（用户 2026-11-01 口径）。
    if rec.mode == auth::weblogin::MODE_RESET {
        state
            .weblogin
            .mark_consumed(token, &rec)
            .await
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "更新会话失败"))?;
        state
            .weblogin
            .clear_failures(&rec.group_id, &rec.openid)
            .await;
        tracing::info!(group = %rec.group_id, openid = %rec.openid, "注册码已重置（群内确认成功）");
        return Ok(json!({
            "status": "confirmed",
            "mode": rec.mode,
            "openid": rec.openid,
            "group_id": rec.group_id,
            "target": "",
        }));
    }
    let resource = rec.token_resource();
    let issued = state
        .auth
        .issue(&rec.openid, &resource)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "签发 token 失败"))?;
    state
        .weblogin
        .mark_consumed(token, &rec)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "更新会话失败"))?;
    state
        .weblogin
        .clear_failures(&rec.group_id, &rec.openid)
        .await;
    tracing::info!(
        group = %rec.group_id,
        openid = %rec.openid,
        scope = %rec.scope,
        mode = %rec.mode,
        "Web 跨端登录已确认并签发 token"
    );
    Ok(json!({
        "status": "confirmed",
        "mode": rec.mode,
        "token": issued.token,
        "expires_in_secs": issued.expires_in_secs,
        "resource": resource,
        "openid": rec.openid,
        "group_id": rec.group_id,
        "target": rec.target,
        "cookie": if rec.scope == "props" { "tg_props" } else { "tg_game" },
    }))
}

#[derive(Debug, Deserialize)]
pub struct WhoamiQuery {
    pub token: String,
}

/// 会话自检：web 用它决定是否放行 `/props-admin`（白名单既是门槛也是准入）。
pub async fn whoami(
    State(state): State<AppState>,
    Query(q): Query<WhoamiQuery>,
) -> Result<Json<Value>, ApiError> {
    let rec = state
        .auth
        .verify(&q.token)
        .await
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "token 无效或已过期"))?;
    let scope = props_scope(&state, &rec.openid);
    Ok(Json(json!({
        "openid": rec.openid,
        "resource": rec.resource,
        "props_whitelisted": scope.is_some(),
        "props_scope": scope.map(PropsScope::code),
        "props_menus": scope.map(PropsScope::menus).unwrap_or_default(),
    })))
}

// ==================== QQ 扫码登录预留（默认关闭，ADR-0031 §4） ====================

/// `WEB_QQ_LOGIN_ENABLED=1` 才考虑启用；当前仅占位，返回明确错误。
fn qq_login_enabled() -> bool {
    std::env::var("WEB_QQ_LOGIN_ENABLED").ok().as_deref() == Some("1")
}

/// `GET /auth/web/qq/start`：发起 QQ 互联授权（需备案域名 + 互联应用审核 + union_openid 打通）。
pub async fn qq_start() -> Result<Json<Value>, ApiError> {
    if !qq_login_enabled() {
        return Err(err(
            StatusCode::SERVICE_UNAVAILABLE,
            "QQ 扫码登录未启用（需备案域名 / 互联应用审核 / 平台运营打通 union_openid）",
        ));
    }
    Err(err(
        StatusCode::NOT_IMPLEMENTED,
        "QQ 扫码登录骨架已预留，待外部审批完成后接入",
    ))
}

/// `GET /auth/web/qq/callback`：授权回调（同上）。
pub async fn qq_callback() -> Result<Json<Value>, ApiError> {
    if !qq_login_enabled() {
        return Err(err(StatusCode::SERVICE_UNAVAILABLE, "QQ 扫码登录未启用"));
    }
    Err(err(StatusCode::NOT_IMPLEMENTED, "QQ 扫码登录回调待接入"))
}

/// 仅允许站内相对路径（防开放重定向）。
fn sanitize_target(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || !t.starts_with('/') || t.starts_with("//") || t.contains("://") {
        return None;
    }
    if t.chars().any(|c| c == '\n' || c == '\r' || c.is_control()) {
        return None;
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_must_be_relative() {
        assert_eq!(sanitize_target("/tabletop_game/x").as_deref(), Some("/tabletop_game/x"));
        assert_eq!(sanitize_target("/props-admin/").as_deref(), Some("/props-admin/"));
        for bad in ["https://evil.com", "//evil.com", "javascript:alert(1)", "", "  ", "/a\n/b"] {
            assert!(sanitize_target(bad).is_none(), "{bad} 应被拒绝");
        }
    }
}
