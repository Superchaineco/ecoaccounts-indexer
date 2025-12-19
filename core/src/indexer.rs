use crate::api::{router_with_dashboard, App, IndexState, Status};
use crate::resilience::{AdaptiveChunkManager, RetryConfig, with_retry};
use crate::strategies::{ChunkProcessor, IndexedRangeDecorator, Stats, StrategyConfig};
use alloy::providers::Provider;
use eyre::{Result, ensure};
use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

// ============================================================================
// Core indexer
// ============================================================================

/// Configuration for running the indexer with resilience.
#[derive(Clone)]
pub struct IndexerConfig {
    pub retry: RetryConfig,
    pub chunk_manager: Arc<AdaptiveChunkManager>,
}

impl IndexerConfig {
    pub fn new(initial_chunk_size: u64) -> Self {
        Self {
            retry: RetryConfig::default(),
            chunk_manager: AdaptiveChunkManager::new(
                initial_chunk_size,
                100,                      // min: 100 blocks
                initial_chunk_size * 2,   // max: 2x initial
            ),
        }
    }
}

pub async fn run_indexer<P>(
    provider: P,
    db: &PgPool,
    from: u64,
    to: u64,
    config: &IndexerConfig,
    strategies: Vec<StrategyConfig<P>>,
    app: Option<Arc<App>>,
) -> Result<u64>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    ensure!(from <= to, "from > to");
    let total = to - from + 1;
    let initial_chunk = config.chunk_manager.get();
    info!(from, to, total, chunk_size = initial_chunk, "starting indexer with resilience");

    let bar = ProgressBar::new(total);
    bar.set_style(ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {percent}% | Block {pos}/{len} | Chunk: {msg} | ETA {eta}"
    )?.progress_chars("=>-"));

    let mut cur = from;
    while cur <= to {
        // Check if should stop (pause or new reindex request)
        if let Some(ref a) = app {
            if a.should_interrupt().await {
                bar.finish_with_message("⏸️ Interrupted");
                warn!(block = cur, "interrupted");
                let mut s = a.state.write().await;
                if let Some(ref mut idx) = s.index {
                    idx.current = cur;
                }
                return Ok(cur);
            }
            // Update current position
            let mut s = a.state.write().await;
            if let Some(ref mut idx) = s.index {
                idx.current = cur;
            }
        }

        // Use adaptive chunk size
        let chunk_size = config.chunk_manager.get();
        bar.set_message(format!("{}", chunk_size));

        let end = (cur + chunk_size - 1).min(to);
        debug!(start = cur, end, chunk_size, "processing chunk");

        let tasks: Vec<_> = strategies.iter().map(|cfg| {
            let p = provider.clone();
            let d = db.clone();
            let c = cfg.clone();
            let (s, e) = (cur, end);
            let retry_config = config.retry.clone();
            let chunk_manager = config.chunk_manager.clone();

            tokio::spawn(async move {
                if s.max(c.from_block) > e {
                    return Ok(Stats::default());
                }

                let strategy_name = c.name;
                let processor = IndexedRangeDecorator::new(c.processor.clone(), c.name, c.force_reindex);

                // Execute with retry
                let result = with_retry(&retry_config, strategy_name, || {
                    let proc = processor.clone();
                    let prov = p.clone();
                    let database = d.clone();
                    async move {
                        proc.process(prov, &database, s, e).await
                    }
                }).await;

                match &result {
                    Ok(_) => chunk_manager.on_success(),
                    Err(e) => chunk_manager.on_rpc_error(&e.to_string()),
                }

                result
            })
        }).collect();

        let mut had_error = false;
        for r in join_all(tasks).await {
            match r {
                Ok(Ok(s)) => {
                    if s.logs_found > 0 || s.rows_written > 0 {
                        info!(logs = s.logs_found, rows = s.rows_written, "strategy completed");
                    }
                }
                Ok(Err(e)) => {
                    error!("strategy error: {e}");
                    had_error = true;
                }
                Err(e) => {
                    error!("task panic: {e}");
                    had_error = true;
                }
            }
        }

        // If there were errors, we still continue but the chunk may have been reduced
        if had_error {
            debug!(
                chunk_size = config.chunk_manager.get(),
                "continuing after errors (chunk may have been adjusted)"
            );
        }

        bar.inc(end - cur + 1);
        cur = end + 1;
    }

    bar.finish_with_message("✅ Done");
    info!("indexer finished");
    Ok(to)
}

// ============================================================================
// Main loop with API
// ============================================================================

pub async fn run_indexer_and_follow<P>(
    provider: P,
    db: &PgPool,
    strategies: Vec<StrategyConfig<P>>,
    chunk_size: u64,
    confirmations: u64,
    poll_secs: u64,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let port: u16 = std::env::var("API_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let api_key = std::env::var("API_KEY").unwrap_or_else(|_| "changeme".into());

    let app = App::new(api_key);

    // Create resilient indexer configuration
    let config = IndexerConfig::new(chunk_size);
    info!(
        chunk_size = config.chunk_manager.get(),
        min = 100,
        max = chunk_size * 2,
        "initialized adaptive chunk manager"
    );

    // Check for dashboard path
    let dashboard_path = std::env::var("DASHBOARD_PATH")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Try to find dashboard/dist relative to current dir
            let candidates = [
                PathBuf::from("dashboard/dist"),
                PathBuf::from("../dashboard/dist"),
            ];
            candidates.into_iter().find(|p| p.exists())
        });

    // Start API server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    if dashboard_path.is_some() {
        info!("API: http://0.0.0.0:{port} (endpoints: /api/*, /dashboard)");
    } else {
        info!("API: http://0.0.0.0:{port} (endpoints: /status, /pause, /resume, /reindex)");
        info!("Dashboard not found. Set DASHBOARD_PATH or build dashboard with 'npm run build'");
    }
    let r = router_with_dashboard(app.clone(), dashboard_path);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tokio::spawn(async move { axum::serve(listener, r).await.ok(); });

    let mut last = strategies.iter().map(|c| c.from_block).min().unwrap_or(0);

    loop {
        // Wait while paused (but not if there's a pending reindex)
        while app.is_paused() && app.state.read().await.pending_reindex.is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Check for pending reindex - takes priority
        let pending_reindex = {
            let mut s = app.state.write().await;
            s.pending_reindex.take()
        };
        
        if let Some(reindex_req) = pending_reindex {
            // Clear any current index state and start reindex
            {
                let mut s = app.state.write().await;
                s.index = Some(reindex_req.clone());
                s.status = Status::Reindexing;
            }
            app.set_paused(false);

            let strats: Vec<_> = match &reindex_req.strategy {
                Some(n) => strategies.iter().filter(|s| s.name == n.as_str()).cloned().collect(),
                None => strategies.to_vec(),
            };

            if strats.is_empty() {
                warn!("no matching strategies for reindex");
            } else {
                let head = provider.get_block_number().await? as u64;
                let from = if reindex_req.from > 0 { reindex_req.from }
                          else { strats.iter().map(|s| s.from_block).min().unwrap_or(0) };
                let to = if reindex_req.to > 0 { reindex_req.to } 
                        else { last.max(head.saturating_sub(confirmations)) };

                // Update state with calculated from/to values
                {
                    let mut s = app.state.write().await;
                    if let Some(ref mut idx) = s.index {
                        idx.from = from;
                        idx.to = to;
                        idx.current = from;
                    }
                }

                if from <= to {
                    info!("╔══════════════════════════════════════════════════════════════╗");
                    info!("║                    🔄 REINDEX STARTED                        ║");
                    info!("╠══════════════════════════════════════════════════════════════╣");
                    info!("║  From Block: {:>15}                                ║", from);
                    info!("║  To Block:   {:>15}                                ║", to);
                    info!("║  Strategy:   {:?}", reindex_req.strategy.as_deref().unwrap_or("ALL"));
                    info!("╚══════════════════════════════════════════════════════════════╝");
                    for mut strat in strats {
                        strat.force_reindex = true;
                        match run_indexer(provider.clone(), db, from, to, &config, vec![strat], Some(app.clone())).await {
                            Ok(_) => {}
                            Err(e) => error!("reindex error: {e}"),
                        }
                        // Check if interrupted (pause or another reindex)
                        if app.should_interrupt().await { break; }
                    }
                }
            }

            // Clear reindex if completed (not interrupted)
            if !app.should_interrupt().await {
                let mut s = app.state.write().await;
                s.index = None;
                s.status = Status::Running;
                info!("╔══════════════════════════════════════════════════════════════╗");
                info!("║                    ✅ REINDEX COMPLETED                      ║");
                info!("╚══════════════════════════════════════════════════════════════╝");
            }
            continue;
        }

        // Resume existing index if paused mid-way
        let existing_index = app.state.read().await.index.clone();
        if let Some(idx) = existing_index {
            if !idx.is_reindex && idx.current > 0 && idx.current < idx.to {
                // Resume from where we left off
                info!(from = idx.current, to = idx.to, "resuming indexing");
                
                match run_indexer(provider.clone(), db, idx.current, idx.to, &config, strategies.clone(), Some(app.clone())).await {
                    Ok(processed) => {
                        if !app.should_interrupt().await {
                            last = processed;
                            let mut s = app.state.write().await;
                            s.last_block = last;
                            s.index = None;
                        }
                    }
                    Err(e) => error!("indexer error: {e}"),
                }
                continue;
            }
        }

        // Normal indexing: follow chain head
        let head = provider.get_block_number().await? as u64;
        let safe = head.saturating_sub(confirmations);
        
        {
            let mut s = app.state.write().await;
            s.head = head;
            s.last_block = last;
        }

        if last < safe {
            let from = last + 1;
            info!(from, to = safe, "processing");
            
            // Set normal index state
            {
                let mut s = app.state.write().await;
                s.index = Some(IndexState {
                    from,
                    to: safe,
                    current: from,
                    strategy: None,
                    is_reindex: false,
                });
            }

            match run_indexer(provider.clone(), db, from, safe, &config, strategies.clone(), Some(app.clone())).await {
                Ok(processed) => {
                    if !app.should_interrupt().await {
                        last = processed;
                        let mut s = app.state.write().await;
                        s.last_block = last;
                        s.index = None;
                    }
                }
                Err(e) => error!("indexer error: {e}"),
            }
        } else {
            // Clear index state when idle
            app.state.write().await.index = None;
        }

        tokio::time::sleep(std::time::Duration::from_secs(poll_secs)).await;
    }
}
