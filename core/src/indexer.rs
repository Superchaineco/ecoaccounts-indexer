use crate::strategies::{
    ChunkProcessor, IndexedRangeDecorator, Stats, StrategyConfig
    // VaultsTransactionsCompoundProcessor,
};
use alloy::providers::Provider;
use eyre::{Result, ensure};
use futures_util::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use sqlx::PgPool;
use tracing::{error, info};

pub async fn run_indexer<P>(
    provider: P,
    db: &PgPool,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
    strategies: Vec<StrategyConfig<P>>,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    ensure!(from_block <= to_block, "from_block > to_block");
    ensure!(chunk_size > 0, "chunk_size must be > 0");
    let total = to_block.saturating_sub(from_block).saturating_add(1);

    info!(
        from = from_block,
        to = to_block,
        total,
        chunk_size,
        "starting run_indexer"
    );

    let bar = ProgressBar::new(total.into());
    bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {percent}% | Block {pos}/{len} | ETA {eta}",
        )?
        .progress_chars("=>-"),
    );

    let mut cur = from_block;
    while cur <= to_block {
        let start = cur;
        let end = (start + chunk_size - 1).min(to_block);

        let chunk_size_actual = end - start + 1;
        info!(start, end, chunk_size_actual, "processing chunk");

        let tasks: Vec<_> = strategies
            .iter()
            .map(|config| {
                let provider = provider.clone();
                let db = db.clone();
                let start = start;
                let end = end;
                let config = config.clone();
                tokio::spawn(async move {

                    if start.max(config.from_block) > end {
                        return Ok(Stats::default());
                    }
                    let processor = IndexedRangeDecorator::new(
                        config.processor.clone(),
                        config.name,
                        config.force_reindex,
                    );

                    processor.process(provider, &db, start, end).await
                })
            })
            .collect();

        let results = join_all(tasks).await;
        for result in results {
            match result {
                Ok(Ok(stats)) => {
                    info!(strategy = ?stats, logs_found = stats.logs_found, rows_written = stats.rows_written, "strategy completed")
                }
                Ok(Err(e)) => error!("Strategy failed: {}", e),
                Err(e) => error!("Task panicked: {}", e),
            }
        }

        bar.inc(chunk_size_actual as u64);
        cur = end.saturating_add(1);
    }

    bar.finish_with_message("✅ Sync completed.");
    info!("run_indexer finished");
    Ok(())
}

pub async fn run_indexer_and_follow<P>(
    http_provider: P,
    db: &PgPool,
    strategies: Vec<StrategyConfig<P>>,
    chunk_size: u64,
    confirmations: u64,
    poll_interval_secs: u64,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    eyre::ensure!(chunk_size > 0, "chunk_size must be > 0");

    // Inicializar el último bloque indexado (usando el mínimo from_block de las estrategias)
    let min_from_block = strategies.iter().map(|c| c.from_block).min().unwrap_or(0);
    let mut last_indexed = min_from_block;

    loop {
        // Obtener el head actual y calcular safe_head
        let head = http_provider.get_block_number().await? as u64;
        let safe_head = head.saturating_sub(confirmations);

        // Si hay nuevos bloques para indexar (desde last_indexed + 1 hasta safe_head)
        if last_indexed < safe_head {
            let from = last_indexed.saturating_add(1);
            let to = safe_head;
            info!(from, to, "processing range (historical or live)");

            // Ejecutar todas las estrategias en paralelo
            run_indexer(
                http_provider.clone(),
                db,
                from,
                to,
                chunk_size,
                strategies.clone(),
            )
            .await?;

            // Actualizar el último indexado
            last_indexed = to;
        } else {
            info!("no new blocks to index, waiting...");
        }

        // Esperar antes del próximo poll
        tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await;
    }
}
