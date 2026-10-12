//! 推送模块：写 Redis Stream `qq.push`（qq-gateway 消费后发群消息）。

use redis::aio::ConnectionManager;
use redis::streams::StreamMaxlen;
use redis::AsyncCommands;
use shared::PushEvent;
use tracing::debug;

const PUSH_STREAM: &str = "qq.push";
const MAXLEN: usize = 1000;

/// 为空 event_id 补全为 uuid v4（纯函数，便于单测）。
pub(crate) fn ensure_event_id(mut event: PushEvent) -> PushEvent {
    if event.event_id.is_empty() {
        event.event_id = shared::new_event_id();
    }
    event
}

/// 出站推送器（可 clone 共享连接）。
pub struct Pusher {
    conn: ConnectionManager,
}

impl Pusher {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    /// 推送一条群消息指令到 `qq.push`。
    pub async fn push(&mut self, event: PushEvent) -> anyhow::Result<()> {
        let event = ensure_event_id(event);
        let json = serde_json::to_string(&event)?;
        self.conn
            .xadd_maxlen::<_, _, _, _, ()>(
                PUSH_STREAM,
                StreamMaxlen::Approx(MAXLEN),
                "*",
                &[("event", json.as_str())],
            )
            .await?;
        debug!("→ qq.push {} [group={}]", event.event_id, event.group_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PushEvent {
        PushEvent {
            event_id: String::new(),
            group_id: "mock-group-1".into(),
            content: "主动推送测试".into(),
            reply_msg_id: None,
            at_member_id: Some("mock-openid-1".into()),
        }
    }

    #[test]
    fn ensure_event_id_fills_empty() {
        let e = ensure_event_id(sample());
        assert!(!e.event_id.is_empty());
        assert_eq!(e.event_id.len(), 36); // uuid v4 字符串长度
    }

    #[test]
    fn ensure_event_id_keeps_existing() {
        let mut e = sample();
        e.event_id = "fixed-id".into();
        let e = ensure_event_id(e);
        assert_eq!(e.event_id, "fixed-id");
    }

    #[test]
    fn push_event_serde_roundtrip_proactive() {
        let mut e = sample();
        e.event_id = "evt-1".into();
        e.reply_msg_id = None; // 主动推送：无 reply_msg_id
        let json = serde_json::to_string(&e).unwrap();
        let back: PushEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_id, "evt-1");
        assert_eq!(back.group_id, "mock-group-1");
        assert_eq!(back.content, "主动推送测试");
        assert!(back.reply_msg_id.is_none());
        assert_eq!(back.at_member_id.as_deref(), Some("mock-openid-1"));
    }
}
