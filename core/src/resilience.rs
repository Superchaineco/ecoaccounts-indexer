use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, debug};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}
impl Default for RetryConfig {
    fn default() -> Self {
        Self { max_retries: 5, initial_delay_ms: 500, max_delay_ms: 30_000, backoff_multiplier: 2.0 }
    }
}

pub async fn with_retry<T, E, F, Fut>(
    config: &RetryConfig,
    op_name: &str,
    mut op: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = config.initial_delay_ms;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match op().await {
            Ok(val) => {
                if attempt > 1 { info!(op = op_name, attempt, "retry success"); }
                return Ok(val);
            }
            Err(e) => {
                let retryable = is_retryable_error(&e.to_string());
                if attempt >= config.max_retries || !retryable {
                    warn!(op = op_name, attempt, retryable, error = %e, "retry failed");
                    return Err(e);
                }
                warn!(op = op_name, attempt, delay, error = %e, "retrying");
                sleep(Duration::from_millis(delay)).await;
                delay = ((delay as f64) * config.backoff_multiplier) as u64;
                delay = delay.min(config.max_delay_ms);
            }
        }
    }
}

fn is_retryable_error(e: &str) -> bool {
    let e = e.to_lowercase();
    e.contains("500") || e.contains("502") || e.contains("503") || e.contains("504") || e.contains("429")
        || e.contains("rate limit") || e.contains("too many requests") || e.contains("request timed out")
        || e.contains("timeout") || e.contains("temporary") || e.contains("retry") || e.contains("internal error")
        || e.contains("connection refused") || e.contains("connection reset") || e.contains("broken pipe") || e.contains("network")
}

#[derive(Debug)]
pub struct AdaptiveChunkManager {
    current: AtomicU64,
    min: u64,
    max: u64,
    initial: u64,
    growth_threshold: u32,
    consecutive_successes: AtomicU64,
}
impl AdaptiveChunkManager {
    pub fn new(initial: u64, min: u64, max: u64) -> Arc<Self> {
        Arc::new(Self {
            current: AtomicU64::new(initial),
            min,
            max,
            initial,
            growth_threshold: 5,
            consecutive_successes: AtomicU64::new(0),
        })
    }
    pub fn get(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }
    pub fn on_success(&self) {
        let s = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;
        if s >= self.growth_threshold as u64 {
            let old = self.current.load(Ordering::Relaxed);
            let new = ((old as f64) * 1.25) as u64;
            let new = new.min(self.max);
            if new > old {
                self.current.store(new, Ordering::Relaxed);
                self.consecutive_successes.store(0, Ordering::Relaxed);
                info!(old_chunk = old, new_chunk = new, "chunk up");
            }
        }
    }
    pub fn on_rpc_error(&self, error: &str) {
        if is_chunk_size_error(error) {
            let old = self.current.load(Ordering::Relaxed);
            let new = (old / 2).max(self.min);
            if new < old {
                self.current.store(new, Ordering::Relaxed);
                self.consecutive_successes.store(0, Ordering::Relaxed);
                warn!(old_chunk = old, new_chunk = new, "chunk down");
            }
        }
    }
    pub fn reset(&self) {
        self.current.store(self.initial, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        debug!(chunk = self.initial, "chunk reset");
    }
}

fn is_chunk_size_error(e: &str) -> bool {
    let e = e.to_lowercase();
    e.contains("compute limit") || e.contains("block range") || e.contains("query timeout")
        || e.contains("response size") || e.contains("exceeded") || e.contains("too large")
        || e.contains("limit exceeded") || e.contains("timed out") || e.contains("timeout")
}

