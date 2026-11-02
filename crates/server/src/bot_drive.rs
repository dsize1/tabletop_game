//! 机器人自动推进的房间锁（P6.3）。
//!
//! 约束：server 是 Web 动作通路的**单写者**，但 HTTP 请求可能并发（两个玩家同时点、
//! 页面轮询触发动作等）。机器人推进必须**按房间串行**，否则会把同一房间推进两次
//! （动作重复/状态回退）。这里用 per-room `tokio::sync::Mutex` + `try_lock`：
//!
//! - 拿到锁 → 正常推进；
//! - 已有推进在跑（锁被占）→ 本次直接跳过（另一个请求会把机器人跑完，页面轮询兜底）。
//!
//! 不排队（避免请求堆积成雪崩）；房间数很少，锁表常驻即可。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// 房间级推进锁表。
#[derive(Default)]
pub struct BotDrive {
    locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl BotDrive {
    pub fn new() -> Self {
        Self::default()
    }

    /// 尝试独占某房间的推进权；已有推进在跑时返回 `None`。
    pub fn try_acquire(&self, room_id: &str) -> Option<OwnedMutexGuard<()>> {
        let lock = {
            let mut map = match self.locks.lock() {
                Ok(m) => m,
                Err(poisoned) => poisoned.into_inner(),
            };
            map.entry(room_id.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        lock.try_lock_owned().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_is_rejected_while_held() {
        let drive = BotDrive::new();
        let first = drive.try_acquire("room-1");
        assert!(first.is_some(), "首次应拿到锁");
        assert!(
            drive.try_acquire("room-1").is_none(),
            "同一房间不应重复推进"
        );
        assert!(
            drive.try_acquire("room-2").is_some(),
            "不同房间互不影响"
        );
        drop(first);
        assert!(
            drive.try_acquire("room-1").is_some(),
            "释放后应可再次获取"
        );
    }
}
