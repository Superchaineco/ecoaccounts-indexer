use crate::strategies::process_super_account_created_chunk;
use alloy::{primitives::Address, providers::Provider};
use eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use sqlx::PgPool;

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
    let total = to_block - from_block + 1;
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
