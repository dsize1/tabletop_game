//! 后台消费循环的**自愈**封装（P6.3 线上问题修复）。
//!
//! 背景：Redis 重启/加载数据（`BusyLoadingError`）时，`bot_adapter::run` 等循环会
//! 直接返回错误；原先 `tokio::spawn` 里只打一条 error 就结束 → **消息链路永久断开**
//! （群消息进 `qq.raw` 但再没人消费，表现为「机器人没反应」），只能重启 server。
//!
//! 这里统一包装成「失败 → 指数退避重试，永不放弃」：
//! - 失败只记 `warn`（不刷 error 吓人），并在恢复后自动继续；
//! - 退避 500ms 起、上限 30s；
//! - 循环正常返回（理论上不会）也按退避重启。
//!
//! 注意：`XREADGROUP ... >` 只投递「未被任何消费者取走」的消息，所以重启消费循环
//! 不会丢消息（停机期间的积压会在恢复后被消费）。

use std::future::Future;
use std::time::Duration;

/// 退避基数与上限。
pub const BACKOFF_BASE: Duration = Duration::from_millis(500);
pub const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// 下一次退避时长（翻倍并封顶）。
pub fn next_backoff(current: Duration) -> Duration {
    let doubled = current.saturating_mul(2);
    if doubled > BACKOFF_MAX {
        BACKOFF_MAX
    } else {
        doubled
    }
}

/// 在后台跑一个**永不放弃**的循环：`run` 返回（Ok/Err）都按退避重启。
///
/// `run` 需是 `FnMut() -> Future`（每次重试都新建 future，避免借用冲突）。
pub fn spawn_resilient<N, F, Fut>(name: N, mut run: F)
where
    N: Into<String> + Send + 'static,
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send,
{
    let name = name.into();
    tokio::spawn(async move {
        let mut backoff = BACKOFF_BASE;
        loop {
            match run().await {
                Ok(()) => {
                    tracing::warn!("{name} 循环正常返回，将按退避重启");
                    backoff = BACKOFF_BASE;
                }
                Err(e) => {
                    tracing::warn!("{name} 循环中断（{e:#}），{}ms 后重试", backoff.as_millis());
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = next_backoff(backoff);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(next_backoff(BACKOFF_BASE), Duration::from_secs(1));
        assert_eq!(next_backoff(Duration::from_secs(1)), Duration::from_secs(2));
        assert_eq!(next_backoff(Duration::from_secs(30)), BACKOFF_MAX);
        assert_eq!(next_backoff(Duration::from_secs(25)), BACKOFF_MAX);
    }
}
