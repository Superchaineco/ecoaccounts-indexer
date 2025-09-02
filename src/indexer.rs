use crate::strategies::process_super_account_created_chunk;
use alloy::{primitives::Address, providers::Provider};
use eyre::{Result, ensure};
use indicatif::{ProgressBar, ProgressStyle};
use sqlx::PgPool;
use tracing::{info};

pub async fn run_indexer<P>(
    provider: P,
    db: &PgPool,
    contract_addr: Address,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    ensure!(from_block <= to_block, "from_block > to_block");
    ensure!(chunk_size > 0, "chunk_size must be > 0");
    let total = to_block.saturating_sub(from_block).saturating_add(1);
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

        process_super_account_created_chunk(provider.clone(), db, contract_addr, start, end)
            .await?;

        bar.inc((end - start + 1) as u64);
        cur = end.saturating_add(1);
    }

    bar.finish_with_message("✅ Sync completed.");
    Ok(())
}

pub async fn run_indexer_and_follow<P>(
    http_provider: P,
    db: &PgPool,
    contract_addr: alloy::primitives::Address,
    from_block: u64,
    chunk_size: u64,
    confirmations: u64,
    poll_interval_secs: u64,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    eyre::ensure!(chunk_size > 0, "chunk_size must be > 0");

    let mut head = http_provider.get_block_number().await? as u64;
    let mut safe_head = head.saturating_sub(confirmations);

    if from_block <= safe_head {
        info!(from = from_block, to = safe_head, "historical sync start");
        run_indexer(
            http_provider.clone(),
            db,
            contract_addr,
            from_block,
            safe_head,
            chunk_size,
        )
        .await?;
        info!("historical sync done");
    } else {
        info!(
            "nothing to do in historical phase: from_block ({}) > safe_head ({})",
            from_block, safe_head
        );
    }

    let mut cursor = safe_head.saturating_add(1);
    loop {
        head = http_provider.get_block_number().await? as u64;
        safe_head = head.saturating_sub(confirmations);

        if cursor <= safe_head {
            let to = safe_head;
            info!(from = cursor, to, "catch-up polling");
            run_indexer(
                http_provider.clone(),
                db,
                contract_addr,
                cursor,
                to,
                chunk_size,
            )
            .await?;
            cursor = to.saturating_add(1);
            continue;
        }

        break;
    }

    live_polling_loop(
        http_provider,
        db,
        contract_addr,
        confirmations,
        poll_interval_secs,
        chunk_size,
    )
    .await
}

async fn live_polling_loop<P>(
    provider: P,
    db: &PgPool,
    contract_addr: alloy::primitives::Address,
    confirmations: u64,
    poll_interval_secs: u64,
    chunk_size: u64,
) -> Result<()>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let mut last_synced = provider
        .get_block_number()
        .await?
        .saturating_sub(confirmations);

    loop {
        let head = provider.get_block_number().await?;
        let safe_head = head.saturating_sub(confirmations);

        if last_synced < safe_head {
            let from = last_synced.saturating_add(1);
            let to = safe_head;
            info!(from, to, "polling live range");
            run_indexer(provider.clone(), db, contract_addr, from, to, chunk_size).await?;
            last_synced = to;
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await;
        }
    }
}
