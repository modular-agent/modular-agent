//! Retry with exponential backoff and per-attempt deadlines for LLM HTTP calls.

use std::future::Future;
use std::time::Duration;

use modular_agent_core::AgentError;

/// Upper bound for a single backoff sleep so a huge `Retry-After` value cannot
/// stall a turn indefinitely.
const MAX_DELAY: Duration = Duration::from_secs(60);

/// Per-turn snapshot of the retry/timeout node configs shared by the LLM
/// agents. Snapshotting once per `process()` call keeps a mid-turn config
/// change from altering an in-flight retry loop.
#[derive(Clone, Copy)]
pub(crate) struct RetryPolicy {
    pub(crate) max_retries: u32,
    pub(crate) base_delay: Duration,
    /// Per-attempt deadline; `None` disables it.
    pub(crate) timeout: Option<Duration>,
}

impl RetryPolicy {
    /// Build a policy from the raw `max_retries` / `retry_base_delay_ms` /
    /// `timeout_secs` config integers. Negative values are treated as 0, and
    /// `timeout_secs == 0` disables the deadline.
    pub(crate) fn from_configs(max_retries: i64, base_delay_ms: i64, timeout_secs: i64) -> Self {
        Self {
            max_retries: u32::try_from(max_retries.max(0)).unwrap_or(u32::MAX),
            base_delay: Duration::from_millis(base_delay_ms.max(0) as u64),
            timeout: (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs as u64)),
        }
    }

    /// Run `f` under this policy: every attempt gets the per-attempt deadline,
    /// and retryable failures (including deadline timeouts) are retried with
    /// exponential backoff.
    ///
    /// For streaming calls, wrap only stream establishment with this: once a
    /// chunk has been emitted downstream it cannot be rolled back, so
    /// mid-stream failures must propagate instead of being retried.
    pub(crate) async fn run<T, F, Fut>(&self, f: F) -> Result<T, AgentError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, AgentError>>,
    {
        let timeout = self.timeout;
        with_retry(self.max_retries, self.base_delay, || {
            with_timeout(timeout, f())
        })
        .await
    }
}

/// Retry `f` on retryable errors with exponential backoff, up to
/// `max_retries` retries. A server-provided `Retry-After` takes precedence
/// over the computed backoff; either delay is clipped at 60s. Returns the
/// last error when retries are exhausted.
pub(crate) async fn with_retry<T, F, Fut>(
    max_retries: u32,
    base: Duration,
    f: F,
) -> Result<T, AgentError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, AgentError>>,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Err(e) if e.is_retryable() && attempt < max_retries => {
                let delay = match &e {
                    AgentError::RateLimited {
                        retry_after: Some(d),
                        ..
                    } => *d,
                    _ => base.saturating_mul(2u32.saturating_pow(attempt)),
                };
                tokio::time::sleep(delay.min(MAX_DELAY)).await;
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Apply an optional deadline to `fut`, mapping expiry to
/// `AgentError::Timeout` (retryable, so `with_retry` will retry it).
pub(crate) async fn with_timeout<T, Fut>(
    deadline: Option<Duration>,
    fut: Fut,
) -> Result<T, AgentError>
where
    Fut: Future<Output = Result<T, AgentError>>,
{
    match deadline {
        Some(d) => match tokio::time::timeout(d, fut).await {
            Ok(result) => result,
            Err(_) => Err(AgentError::Timeout(format!(
                "LLM request did not complete within {}s",
                d.as_secs()
            ))),
        },
        None => fut.await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn rate_limited(retry_after: Option<Duration>) -> AgentError {
        AgentError::RateLimited {
            message: "rate limited".into(),
            retry_after,
        }
    }

    #[test]
    fn test_policy_from_configs() {
        let p = RetryPolicy::from_configs(2, 1000, 0);
        assert_eq!(p.max_retries, 2);
        assert_eq!(p.base_delay, Duration::from_millis(1000));
        assert!(p.timeout.is_none());

        let p = RetryPolicy::from_configs(-1, -5, 300);
        assert_eq!(p.max_retries, 0);
        assert_eq!(p.base_delay, Duration::ZERO);
        assert_eq!(p.timeout, Some(Duration::from_secs(300)));
    }

    #[tokio::test]
    async fn test_with_retry_retries_then_succeeds() {
        let attempts = AtomicU32::new(0);
        let result = with_retry(3, Duration::from_millis(1), || async {
            if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                Err(AgentError::Overloaded("busy".into()))
            } else {
                Ok(42)
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_respects_retry_after() {
        let attempts = AtomicU32::new(0);
        let start = std::time::Instant::now();
        // Base delay is huge; if retry_after were ignored the test would stall.
        let result = with_retry(2, Duration::from_secs(30), || async {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(rate_limited(Some(Duration::from_millis(10))))
            } else {
                Ok(())
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_with_retry_exhausts_and_returns_last_error() {
        let attempts = AtomicU32::new(0);
        let result: Result<(), AgentError> = with_retry(2, Duration::from_millis(1), || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(AgentError::Timeout("deadline".into()))
        })
        .await;
        assert!(matches!(result, Err(AgentError::Timeout(_))));
        // Initial attempt + 2 retries.
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_non_retryable_returns_immediately() {
        let attempts = AtomicU32::new(0);
        let result: Result<(), AgentError> = with_retry(5, Duration::from_millis(1), || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(AgentError::InvalidConfig("bad key".into()))
        })
        .await;
        assert!(matches!(result, Err(AgentError::InvalidConfig(_))));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_with_timeout_disabled_passes_through() {
        let result = with_timeout(None, async { Ok::<_, AgentError>(1) }).await;
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_with_timeout_elapsed_maps_to_timeout() {
        let result: Result<(), AgentError> = with_timeout(Some(Duration::from_millis(5)), async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        })
        .await;
        assert!(matches!(result, Err(AgentError::Timeout(_))));
    }
}
