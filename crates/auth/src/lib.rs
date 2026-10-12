//! 短期 token 签发/校验（Redis 存储）。
//!
//! 语义（与用户确认的实现约定一致）：
//! - 存 Redis，优先查询 Redis；Redis 失效/不存在时视为无效，由调用方重新签发。
//! - 签发时校验 openid（非空）。
//! - TTL 可配，默认 8 小时（`DEFAULT_TTL_SECS`）；局终态（游戏结束）调用 `revoke_resource` 清空该资源全部 token。
//! - token 绑定 `openid` + 资源范围 `resource`（如 `dev_task:{sessionId}`、`game:{gameId}`）。

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

/// 默认 token TTL：8 小时（秒）。
pub const DEFAULT_TTL_SECS: u64 = 8 * 3600;

/// token 在 Redis 中的记录（value 为 JSON）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenRecord {
    pub openid: String,
    pub resource: String,
    pub issued_at: i64,
}

/// 签发结果（调用方据此拼短链）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssuedToken {
    pub token: String,
    pub expires_in_secs: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("token 无效或已过期")]
    Invalid,
    #[error("openid 为空")]
    EmptyOpenid,
    #[error("资源范围为空")]
    EmptyResource,
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
}

/// 鉴权服务（Clone 共享连接，供 server 注入 AppState）。
#[derive(Clone)]
pub struct AuthService {
    conn: ConnectionManager,
    ttl_secs: u64,
}

impl AuthService {
    pub async fn new(redis_url: &str, ttl_secs: u64) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn, ttl_secs })
    }

    /// 签发：uuid v4 token，绑定 openid + 资源范围，TTL 可配。
    /// 同时维护 `auth:resource:{resource}` 索引，便于局终态按资源清空。
    pub async fn issue(&self, openid: &str, resource: &str) -> Result<IssuedToken, AuthError> {
        if openid.trim().is_empty() {
            return Err(AuthError::EmptyOpenid);
        }
        if resource.trim().is_empty() {
            return Err(AuthError::EmptyResource);
        }
        let token = uuid::Uuid::new_v4().to_string();
        let record = TokenRecord {
            openid: openid.to_string(),
            resource: resource.to_string(),
            issued_at: now_secs(),
        };
        let json = serde_json::to_string(&record).map_err(|_| AuthError::Invalid)?;

        let mut conn = self.conn.clone();
        let _: () = conn.set_ex(token_key(&token), &json, self.ttl_secs).await?;
        let _: () = conn.sadd(resource_key(resource), &token).await?;
        let _: () = conn
            .expire(resource_key(resource), self.ttl_secs as i64)
            .await?;

        tracing::debug!(%token, %openid, %resource, ttl = self.ttl_secs, "签发短期 token");
        Ok(IssuedToken {
            token,
            expires_in_secs: self.ttl_secs,
        })
    }

    /// 校验：优先查 Redis；不存在/过期/记录损坏 → `AuthError::Invalid`。
    pub async fn verify(&self, token: &str) -> Result<TokenRecord, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::Invalid);
        }
        let mut conn = self.conn.clone();
        let raw: Option<String> = conn.get(token_key(token)).await?;
        match raw {
            Some(json) => parse_record(&json),
            None => Err(AuthError::Invalid),
        }
    }

    /// 删除单个 token（注销/单点失效）。
    pub async fn revoke(&self, token: &str) -> Result<(), AuthError> {
        let mut conn = self.conn.clone();
        if let Some(raw) = conn.get::<_, Option<String>>(token_key(token)).await? {
            if let Ok(rec) = parse_record(&raw) {
                let _: () = conn.srem(resource_key(&rec.resource), token).await?;
            }
        }
        let _: () = conn.del(token_key(token)).await?;
        Ok(())
    }

    /// 按资源清空全部 token（局终态：游戏结束时清除该资源下所有 token）。
    pub async fn revoke_resource(&self, resource: &str) -> Result<(), AuthError> {
        let mut conn = self.conn.clone();
        let tokens: Vec<String> = conn.smembers(resource_key(resource)).await?;
        for t in &tokens {
            let _: () = conn.del(token_key(t)).await?;
        }
        let _: () = conn.del(resource_key(resource)).await?;
        tracing::debug!(%resource, count = tokens.len(), "局终态清空资源 token");
        Ok(())
    }
}

/// 解析 Redis 中的 token 记录；损坏视为无效（不区分是「过期」还是「伪造」）。
fn parse_record(json: &str) -> Result<TokenRecord, AuthError> {
    serde_json::from_str::<TokenRecord>(json).map_err(|_| AuthError::Invalid)
}

fn token_key(token: &str) -> String {
    format!("auth:token:{token}")
}

fn resource_key(resource: &str) -> String {
    format!("auth:resource:{resource}")
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_record_serde_roundtrip() {
        let rec = TokenRecord {
            openid: "openid-1".into(),
            resource: "dev_task:abc-123".into(),
            issued_at: 1700000000,
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back = parse_record(&json).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn parse_record_rejects_garbage() {
        assert!(matches!(parse_record("not-json"), Err(AuthError::Invalid)));
        assert!(matches!(parse_record(""), Err(AuthError::Invalid)));
    }

    #[test]
    fn key_formats() {
        assert_eq!(token_key("t-1"), "auth:token:t-1");
        assert_eq!(resource_key("dev_task:x"), "auth:resource:dev_task:x");
    }

    #[test]
    fn issued_token_fields() {
        let t = IssuedToken {
            token: "abc".into(),
            expires_in_secs: DEFAULT_TTL_SECS,
        };
        assert_eq!(t.expires_in_secs, 8 * 3600);
    }

    // ---- 集成测试（需本地 Redis；Redis 不可用则软跳过） ----

    async fn try_service() -> Option<AuthService> {
        match AuthService::new("redis://127.0.0.1:6379", 60).await {
            Ok(svc) => Some(svc),
            Err(_) => None,
        }
    }

    #[tokio::test]
    async fn issue_then_verify_roundtrip() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        let issued = svc.issue("openid-9", "dev_task:test-1").await.unwrap();
        assert_eq!(issued.token.len(), 36);
        let rec = svc.verify(&issued.token).await.unwrap();
        assert_eq!(rec.openid, "openid-9");
        assert_eq!(rec.resource, "dev_task:test-1");
        svc.revoke(&issued.token).await.unwrap();
    }

    #[tokio::test]
    async fn verify_rejects_unknown_and_revoked() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        // 伪造/不存在
        assert!(matches!(
            svc.verify("no-such-token").await,
            Err(AuthError::Invalid)
        ));
        // 签发后撤销
        let issued = svc.issue("openid-9", "dev_task:test-2").await.unwrap();
        svc.revoke(&issued.token).await.unwrap();
        assert!(matches!(
            svc.verify(&issued.token).await,
            Err(AuthError::Invalid)
        ));
    }

    #[tokio::test]
    async fn revoke_resource_clears_all() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        let a = svc.issue("openid-1", "game:room-x").await.unwrap();
        let b = svc.issue("openid-2", "game:room-x").await.unwrap();
        svc.revoke_resource("game:room-x").await.unwrap();
        assert!(matches!(svc.verify(&a.token).await, Err(AuthError::Invalid)));
        assert!(matches!(svc.verify(&b.token).await, Err(AuthError::Invalid)));
    }

    #[tokio::test]
    async fn issue_rejects_empty_openid() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        assert!(matches!(
            svc.issue("", "dev_task:x").await,
            Err(AuthError::EmptyOpenid)
        ));
        assert!(matches!(
            svc.issue("openid", "").await,
            Err(AuthError::EmptyResource)
        ));
    }
}
