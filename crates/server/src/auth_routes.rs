//! auth HTTP API：短期 token 签发 / 校验。
//!
//! - `POST /auth/issue`：签发（绑定 openid + 资源范围）。
//! - `POST /auth/verify`：校验（无效/过期 → 401）。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct IssueRequest {
    pub openid: String,
    pub resource: String,
    /// 可选：短时效（秒）。缺省用 auth 实例默认 TTL（8 小时）。
    /// 用于「加密选项页」：5 分钟即可，过期后玩家发「#恢复」重新签发。
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct IssueResponse {
    pub token: String,
    pub expires_in_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub openid: String,
    pub resource: String,
}

/// 签发短期 token。openid / resource 为空 → 400。
pub async fn issue(
    State(state): State<AppState>,
    Json(req): Json<IssueRequest>,
) -> Result<Json<IssueResponse>, StatusCode> {
    let issued = match req.ttl_secs {
        Some(t) => state.auth.issue_with_ttl(&req.openid, &req.resource, t).await,
        None => state.auth.issue(&req.openid, &req.resource).await,
    };
    match issued {
        Ok(t) => Ok(Json(IssueResponse {
            token: t.token,
            expires_in_secs: t.expires_in_secs,
        })),
        Err(auth::AuthError::EmptyOpenid | auth::AuthError::EmptyResource) => {
            Err(StatusCode::BAD_REQUEST)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// 校验 token。无效/过期/伪造 → 401。
pub async fn verify(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, StatusCode> {
    match state.auth.verify(&req.token).await {
        Ok(rec) => Ok(Json(VerifyResponse {
            valid: true,
            openid: rec.openid,
            resource: rec.resource,
        })),
        Err(auth::AuthError::Invalid) => Err(StatusCode::UNAUTHORIZED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
