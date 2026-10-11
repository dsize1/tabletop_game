//! 推送模块：写 Redis Stream `qq.push`（qq-gateway 消费后发群消息）。

use redis::aio::ConnectionManager;
use redis::streams::StreamMaxlen;
use redis::AsyncCommands;
use shared::PushEvent;
use tracing::debug;

const PUSH_STREAM: &str = "qq.push";
const MAXLEN: usize = 1000;

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
    pub async fn push(&mut self, mut event: PushEvent) -> anyhow::Result<()> {
        if event.event_id.is_empty() {
            event.event_id = shared::new_event_id();
        }
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
