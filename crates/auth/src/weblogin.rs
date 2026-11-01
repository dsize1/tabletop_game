//! Web 跨端权鉴（ADR-0031）：注册码哈希 + 登录会话状态机。
//!
//! 设计要点（与用户确认的口径一致）：
//! - **短效登录会话 = token + 状态 + 4 位验证码 + 目标路由，同生命周期**（同一 Redis 记录、同一 TTL）。
//! - 注册码只存 **Argon2id** 哈希（`web_accounts.passcode_hash`），确认成功后才落库（R1，由 server 调用）。
//! - 4 位验证码必须与**回复人 openid** 比对（R2），因此码 → 会话的索引按 `(group_id, code)` 建。
//! - 确认后由 server 一次性发放正式 token（8h）；本模块只负责状态与一次性语义。
//!
//! 键空间：
//! - `auth:weblogin:{token}`  会话记录（JSON）
//! - `auth:webcode:{group}:{code}` → token（与短效 token 同 TTL）
//! - `auth:webfail:{group}:{openid}`  连续失败计数（30 分钟窗口）
//! - `auth:weblock:{group}:{openid}`  锁定标记（3 次失败 → 30 分钟）

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::AuthError;

/// 短效会话默认 TTL（秒）：10 分钟；阶段转换时在 `1..=此值` 内续期。
pub const WEB_LOGIN_TTL_SECS: u64 = 600;
/// 注册码长度（6 位 `[0-9a-zA-Z]`）。
pub const PASSCODE_LEN: usize = 6;
/// 验证码位数（0-9）。
pub const CODE_LEN: usize = 4;
/// 连续失败上限（达到即锁定）。
pub const MAX_FAILS: u32 = 3;
/// 锁定时长（秒）：30 分钟。
pub const LOCK_SECS: u64 = 30 * 60;
/// 确认换票后保留会话记录的时间（秒）：让二次访问能拿到 `used` 而不是 `expired`。
pub const CONSUMED_GRACE_SECS: u64 = 120;

/// 会话阶段。
pub const STAGE_NEED_REGISTER: &str = "need_register";
pub const STAGE_NEED_LOGIN: &str = "need_login";
pub const STAGE_CODE: &str = "code";
pub const STAGE_CONFIRMED: &str = "confirmed";

/// 会话用途：登录/注册 vs 重置注册码（重置确认后无跳转）。
pub const MODE_LOGIN: &str = "login";
pub const MODE_RESET: &str = "reset";

/// 登录会话记录（Redis JSON）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebLoginRecord {
    pub group_id: String,
    pub openid: String,
    #[serde(default)]
    pub display_name: String,
    /// `game` / `props`
    pub scope: String,
    /// 确认后跳转的站内路由（如 `/tabletop_game/sanguosha/player/xxx`、`/props-admin/`）
    pub target: String,
    /// 关联对局（scope=game 时有值）
    #[serde(default)]
    pub room_id: Option<String>,
    /// `login` / `reset`
    pub mode: String,
    pub stage: String,
    /// 4 位验证码（stage ∈ {code, confirmed} 时有值）
    #[serde(default)]
    pub code: String,
    /// 注册码哈希暂存（**确认成功后才由 server 落库**；R1）
    #[serde(default)]
    pub pending_hash: Option<String>,
    #[serde(default)]
    pub attempts: u32,
    pub created_at: i64,
    #[serde(default)]
    pub confirmed_at: Option<i64>,
    #[serde(default)]
    pub consumed: bool,
}

impl WebLoginRecord {
    pub fn new(
        group_id: &str,
        openid: &str,
        display_name: &str,
        scope: &str,
        target: &str,
        room_id: Option<&str>,
        mode: &str,
        registered: bool,
    ) -> Self {
        Self {
            group_id: group_id.to_string(),
            openid: openid.to_string(),
            display_name: display_name.to_string(),
            scope: scope.to_string(),
            target: target.to_string(),
            room_id: room_id.map(|s| s.to_string()),
            mode: mode.to_string(),
            stage: if registered {
                STAGE_NEED_LOGIN.to_string()
            } else {
                STAGE_NEED_REGISTER.to_string()
            },
            code: String::new(),
            pending_hash: None,
            attempts: 0,
            created_at: now_secs(),
            confirmed_at: None,
            consumed: false,
        }
    }

    /// 确认后正式 token 的资源范围。
    pub fn token_resource(&self) -> String {
        match self.scope.as_str() {
            "props" => "props_admin:all".to_string(),
            _ => format!("game:{}", self.room_id.clone().unwrap_or_default()),
        }
    }
}

/// 群内验证码确认结果。
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmOutcome {
    /// 码有效且回复人 = 会话 openid。
    Confirmed(WebLoginRecord),
    /// 码有效但回复人不是会话主人（不泄露归属）。
    Mismatch,
    /// 码不存在/过期。
    Unknown,
}

/// 注册码格式校验：恰好 6 位 `[0-9a-zA-Z]`。
pub fn is_valid_passcode(passcode: &str) -> bool {
    passcode.len() == PASSCODE_LEN
        && passcode
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase() || c.is_ascii_lowercase())
}

/// 生成注册码哈希（Argon2id，随机盐）。
pub fn hash_passcode(passcode: &str) -> Result<String, AuthError> {
    if !is_valid_passcode(passcode) {
        return Err(AuthError::InvalidPasscode);
    }
    let salt = SaltString::generate(&mut rand::thread_rng());
    let params = Params::new(8 * 1024, 2, 1, None).map_err(|_| AuthError::Hash)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(passcode.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AuthError::Hash)
}

/// 校验注册码（哈希损坏/格式不符一律 false）。
pub fn verify_passcode(hash: &str, passcode: &str) -> bool {
    if !is_valid_passcode(passcode) {
        return false;
    }
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(passcode.as_bytes(), &parsed)
        .is_ok()
}

/// 生成 4 位数字验证码（`0000`–`9999`，保留前导零）。
pub fn gen_code<R: Rng>(rng: &mut R) -> String {
    format!("{:04}", rng.gen_range(0..10_000u32))
}

/// 规范化用户输入的验证码（允许 `#验证码 1234` 的空格写法）。
pub fn normalize_code(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.len() == CODE_LEN && t.chars().all(|c| c.is_ascii_digit()) {
        Some(t.to_string())
    } else {
        None
    }
}

/// Web 跨端登录会话服务（Redis）。
#[derive(Clone)]
pub struct WebLoginService {
    conn: ConnectionManager,
    ttl_secs: u64,
}

impl WebLoginService {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        Self::with_ttl(redis_url, WEB_LOGIN_TTL_SECS).await
    }

    pub async fn with_ttl(redis_url: &str, ttl_secs: u64) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn, ttl_secs })
    }

    pub fn ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    /// 新建会话：返回短效登录 token（记录内已含阶段/目标路由）。
    pub async fn create(&self, rec: &WebLoginRecord) -> Result<(String, u64), AuthError> {
        if rec.openid.trim().is_empty() || rec.group_id.trim().is_empty() {
            return Err(AuthError::EmptyOpenid);
        }
        let token = uuid::Uuid::new_v4().to_string();
        self.save(&token, rec, self.ttl_secs).await?;
        Ok((token, self.ttl_secs))
    }

    /// 读会话（过期/不存在 → None）。
    pub async fn get(&self, token: &str) -> Result<Option<WebLoginRecord>, AuthError> {
        if token.trim().is_empty() {
            return Ok(None);
        }
        let mut conn = self.conn.clone();
        let raw: Option<String> = conn.get(session_key(token)).await?;
        match raw {
            Some(json) => Ok(serde_json::from_str(&json).ok()),
            None => Ok(None),
        }
    }

    /// 写回会话（续期到 `ttl_secs`，夹到 `1..=self.ttl_secs`）。
    pub async fn save(
        &self,
        token: &str,
        rec: &WebLoginRecord,
        ttl_secs: u64,
    ) -> Result<(), AuthError> {
        let ttl = ttl_secs.clamp(1, self.ttl_secs);
        let json = serde_json::to_string(rec).map_err(|_| AuthError::Invalid)?;
        let mut conn = self.conn.clone();
        let _: () = conn.set_ex(session_key(token), json, ttl).await?;
        Ok(())
    }

    /// 删除会话及其验证码索引。
    pub async fn delete(&self, token: &str) -> Result<(), AuthError> {
        let Some(rec) = self.get(token).await? else {
            return Ok(());
        };
        let mut conn = self.conn.clone();
        if !rec.code.is_empty() {
            let _: () = conn.del(code_key(&rec.group_id, &rec.code)).await?;
        }
        let _: () = conn.del(session_key(token)).await?;
        Ok(())
    }

    /// 生成并登记 4 位验证码（群内唯一；`SET NX EX` 抢占，冲突重试）。
    /// 同时把会话阶段推进到 `code` 并续期。
    pub async fn issue_code<R: Rng>(
        &self,
        token: &str,
        mut rec: WebLoginRecord,
        rng: &mut R,
    ) -> Result<(WebLoginRecord, String), AuthError> {
        let mut conn = self.conn.clone();
        for _ in 0..32 {
            let code = gen_code(rng);
            let key = code_key(&rec.group_id, &code);
            let ok: Option<String> = redis::cmd("SET")
                .arg(&key)
                .arg(token)
                .arg("NX")
                .arg("EX")
                .arg(self.ttl_secs)
                .query_async(&mut conn)
                .await?;
            if ok.as_deref() != Some("OK") {
                continue;
            }
            rec.code = code.clone();
            rec.stage = STAGE_CODE.to_string();
            self.save(token, &rec, self.ttl_secs).await?;
            return Ok((rec, code));
        }
        Err(AuthError::Hash)
    }

    /// 群内确认：`(group_id, code)` → 会话；比对 openid。
    pub async fn confirm(
        &self,
        group_id: &str,
        code: &str,
        openid: &str,
    ) -> Result<ConfirmOutcome, AuthError> {
        let mut conn = self.conn.clone();
        let token: Option<String> = conn.get(code_key(group_id, code)).await?;
        let Some(token) = token else {
            return Ok(ConfirmOutcome::Unknown);
        };
        let Some(mut rec) = self.get(&token).await? else {
            let _: () = conn.del(code_key(group_id, code)).await?;
            return Ok(ConfirmOutcome::Unknown);
        };
        if rec.openid != openid {
            return Ok(ConfirmOutcome::Mismatch);
        }
        if rec.stage == STAGE_CONFIRMED {
            return Ok(ConfirmOutcome::Confirmed(rec));
        }
        rec.stage = STAGE_CONFIRMED.to_string();
        rec.confirmed_at = Some(now_secs());
        // 保持原生命周期：按剩余 TTL 写回，不额外延长（"同生命周期"）。
        let ttl: i64 = conn.ttl(session_key(&token)).await?;
        let ttl = if ttl > 0 {
            ttl as u64
        } else {
            self.ttl_secs
        };
        self.save(&token, &rec, ttl).await?;
        let _: () = conn.del(code_key(group_id, code)).await?;
        Ok(ConfirmOutcome::Confirmed(rec))
    }

    /// 标记已换票（短会话进入 `consumed`，保留短暂宽限期以返回 `used`）。
    pub async fn mark_consumed(&self, token: &str, rec: &WebLoginRecord) -> Result<(), AuthError> {
        let mut consumed = rec.clone();
        consumed.consumed = true;
        self.save(token, &consumed, CONSUMED_GRACE_SECS).await
    }

    /// 锁定剩余秒数（未锁定 → None）。
    pub async fn lock_remaining(&self, group_id: &str, openid: &str) -> Option<u64> {
        let mut conn = self.conn.clone();
        let ttl: i64 = conn.ttl(lock_key(group_id, openid)).await.unwrap_or(-2);
        if ttl > 0 {
            Some(ttl as u64)
        } else {
            None
        }
    }

    /// 记一次注册码错误：返回累计次数；达到 `MAX_FAILS` 时设置锁定。
    pub async fn note_failure(&self, group_id: &str, openid: &str) -> u32 {
        let mut conn = self.conn.clone();
        let key = fail_key(group_id, openid);
        let count: Option<u32> = redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .ok();
        let count = count.unwrap_or(1);
        let _: Result<(), _> = conn.expire(&key, LOCK_SECS as i64).await;
        if count >= MAX_FAILS {
            let _: Result<(), _> = conn
                .set_ex(lock_key(group_id, openid), "1", LOCK_SECS)
                .await;
            let _: Result<(), _> = conn.del(&key).await;
        }
        count
    }

    /// 清除失败计数（成功登录/重置后）。
    pub async fn clear_failures(&self, group_id: &str, openid: &str) {
        let mut conn = self.conn.clone();
        let _: Result<(), _> = conn.del(fail_key(group_id, openid)).await;
    }
}

fn session_key(token: &str) -> String {
    format!("auth:weblogin:{token}")
}

fn code_key(group_id: &str, code: &str) -> String {
    format!("auth:webcode:{group_id}:{code}")
}

fn fail_key(group_id: &str, openid: &str) -> String {
    format!("auth:webfail:{group_id}:{openid}")
}

fn lock_key(group_id: &str, openid: &str) -> String {
    format!("auth:weblock:{group_id}:{openid}")
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
    use rand::SeedableRng;

    #[test]
    fn passcode_validation_rules() {
        for ok in ["abc123", "ABC123", "000000", "zZ9zZ9"] {
            assert!(is_valid_passcode(ok), "{ok} 应合法");
        }
        for bad in ["abc12", "abc1234", "abc 123", "abc-12", "中文1234", "", "abcdef "] {
            assert!(!is_valid_passcode(bad), "{bad} 应非法");
        }
    }

    #[test]
    fn hash_is_not_plaintext_and_verifies() {
        let hash = hash_passcode("Ab3dE9").unwrap();
        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(!hash.contains("Ab3dE9"));
        assert!(verify_passcode(&hash, "Ab3dE9"));
        assert!(!verify_passcode(&hash, "Ab3dE8"));
        assert!(!verify_passcode("garbage", "Ab3dE9"));
        assert!(!verify_passcode(&hash, "bad"));
    }

    #[test]
    fn hash_uses_random_salt() {
        let a = hash_passcode("same12").unwrap();
        let b = hash_passcode("same12").unwrap();
        assert_ne!(a, b, "同一注册码两次哈希应不同（随机盐）");
        assert!(verify_passcode(&a, "same12") && verify_passcode(&b, "same12"));
    }

    #[test]
    fn code_generation_is_four_digits() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let c = gen_code(&mut rng);
            assert_eq!(c.len(), CODE_LEN);
            assert!(c.chars().all(|x| x.is_ascii_digit()));
        }
    }

    #[test]
    fn normalize_code_accepts_space_and_rejects_others() {
        assert_eq!(normalize_code(" 0000 ").as_deref(), Some("0000"));
        assert_eq!(normalize_code("1234").as_deref(), Some("1234"));
        assert_eq!(normalize_code("123"), None);
        assert_eq!(normalize_code("12345"), None);
        assert_eq!(normalize_code("12a4"), None);
    }

    #[test]
    fn token_resource_by_scope() {
        let mut rec = WebLoginRecord::new("g", "u", "小明", "game", "/x", Some("room-1"), "login", true);
        assert_eq!(rec.token_resource(), "game:room-1");
        rec.scope = "props".into();
        assert_eq!(rec.token_resource(), "props_admin:all");
    }

    #[test]
    fn new_record_stage_follows_registration() {
        let a = WebLoginRecord::new("g", "u", "n", "game", "/x", Some("r"), "login", false);
        assert_eq!(a.stage, STAGE_NEED_REGISTER);
        let b = WebLoginRecord::new("g", "u", "n", "props", "/props-admin/", None, "login", true);
        assert_eq!(b.stage, STAGE_NEED_LOGIN);
        assert_eq!(b.mode, MODE_LOGIN);
    }

    #[test]
    fn record_serde_roundtrip() {
        let rec = WebLoginRecord::new("g", "u", "n", "game", "/x", Some("r"), MODE_RESET, true);
        let json = serde_json::to_string(&rec).unwrap();
        let back: WebLoginRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
    }

    // ---- 集成测试（需本地 Redis；不可用则软跳过） ----
    async fn try_service() -> Option<WebLoginService> {
        WebLoginService::with_ttl("redis://127.0.0.1:6379", 60)
            .await
            .ok()
    }

    #[tokio::test]
    async fn full_flow_register_code_confirm_consume() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        let mut rec = WebLoginRecord::new(
            &format!("g-{}", uuid::Uuid::new_v4()),
            &format!("u-{}", uuid::Uuid::new_v4()),
            "小明",
            "game",
            "/x",
            Some("room-x"),
            MODE_LOGIN,
            false,
        );
        let (token, _) = svc.create(&rec).await.unwrap();
        rec.pending_hash = Some(hash_passcode("abc123").unwrap());
        let (rec2, code) = svc
            .issue_code(&token, rec.clone(), &mut rand::thread_rng())
            .await
            .unwrap();
        assert_eq!(rec2.stage, STAGE_CODE);

        // 别人拿码回复 → Mismatch，不确认
        let other = format!("other-{}", uuid::Uuid::new_v4());
        assert_eq!(
            svc.confirm(&rec.group_id, &code, &other).await.unwrap(),
            ConfirmOutcome::Mismatch
        );
        // 本人确认
        let out = svc
            .confirm(&rec.group_id, &code, &rec.openid)
            .await
            .unwrap();
        assert!(matches!(out, ConfirmOutcome::Confirmed(_)));
        // 码一次性：再确认 → Unknown
        assert_eq!(
            svc.confirm(&rec.group_id, &code, &rec.openid).await.unwrap(),
            ConfirmOutcome::Unknown
        );
        // 换票后 consumed
        if let ConfirmOutcome::Confirmed(rec3) = out {
            svc.mark_consumed(&token, &rec3).await.unwrap();
            let got = svc.get(&token).await.unwrap().unwrap();
            assert!(got.consumed && got.stage == STAGE_CONFIRMED);
        }
        svc.delete(&token).await.unwrap();
    }

    #[tokio::test]
    async fn failure_counter_locks_after_three() {
        let Some(svc) = try_service().await else {
            eprintln!("跳过：Redis 不可用");
            return;
        };
        let g = format!("g-{}", uuid::Uuid::new_v4());
        let u = format!("u-{}", uuid::Uuid::new_v4());
        assert!(svc.lock_remaining(&g, &u).await.is_none());
        assert_eq!(svc.note_failure(&g, &u).await, 1);
        assert_eq!(svc.note_failure(&g, &u).await, 2);
        assert_eq!(svc.note_failure(&g, &u).await, 3);
        assert!(svc.lock_remaining(&g, &u).await.is_some());
        svc.clear_failures(&g, &u).await;
    }
}
