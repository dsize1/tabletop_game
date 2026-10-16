//! 内部事件与公共类型定义
//!
//! 协议约定见 `docs/conventions/message-protocol.md`。

pub mod docs_config;

use serde::{Deserialize, Serialize};

/// 生成全局唯一事件 ID（uuid v4 字符串）
pub fn new_event_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ============ qq.raw 原始事件（bot-adapter 解析用） ============

/// qq.raw Stream 单条消息的 `event` 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqRawEvent {
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub raw: QqRawMessage,
    #[serde(rename = "receivedAt")]
    pub received_at: i64,
}

/// QQ 平台群 @消息的 `raw` 字段（仅声明用到的关键字段，未知字段忽略）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqRawMessage {
    pub id: String,
    pub author: QqAuthor,
    #[serde(rename = "group_openid")]
    pub group_openid: String,
    pub content: String,
    #[serde(rename = "message_type")]
    pub message_type: i32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqAuthor {
    #[serde(rename = "member_openid")]
    pub member_openid: String,
    pub username: String,
}

// ============ internal Stream（bot-adapter → agent-gateway） ============

/// 内部统一消息事件（写入 Redis Stream `internal`）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InternalEvent {
    /// 群 @ 文本消息
    MessageGroupAt(MessageGroupAt),
}

impl InternalEvent {
    pub fn event_id(&self) -> &str {
        match self {
            InternalEvent::MessageGroupAt(m) => &m.event_id,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            InternalEvent::MessageGroupAt(_) => "message_group_at",
        }
    }
}

/// 群 @ 文本消息（bot-adapter 归一化后）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageGroupAt {
    pub event_id: String,
    /// 来源网关，恒为 "qq-gateway"
    pub source: String,
    /// QQ 原始时间戳（RFC3339）
    pub timestamp: String,
    pub group_id: String,
    /// 发送者群内 openid（玩家身份标识，非 QQ 号）
    pub sender_id: String,
    pub sender_name: String,
    /// 平台消息 id（被动回复时作为 msg_id）
    pub message_id: String,
    /// 文本内容（已 trim）
    pub content: String,
}

// ============ qq.push Stream（agent-gateway → qq-gateway） ============

/// 推送指令（写入 Redis Stream `qq.push`）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    pub event_id: String,
    pub group_id: String,
    /// 要发送的群消息文本
    pub content: String,
    /// 被动回复关联的消息 id（有则走被动回复，无则主动发送）
    pub reply_msg_id: Option<String>,
    /// 需要 @ 的玩家 member_openid（可选）
    pub at_member_id: Option<String>,
}

// ============ chat.log Stream（agent-gateway → server） ============

/// 单条对话（写入 Redis Stream `chat.log`，由 server 落 SQLite `chat_history`）。
///
/// 收 **chat + game** 两条分支的用户轮与助手轮（不含 dev），是人物画像蒸馏的数据源
/// （ADR-0007）；`event_id` 幂等去重（at-least-once）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogEvent {
    pub event_id: String,
    /// 用户 openid（画像跨群主键）
    pub openid: String,
    pub group_id: String,
    /// user | assistant
    pub role: String,
    pub content: String,
    /// ISO8601 UTC 写入时刻
    pub created_at: String,
}
