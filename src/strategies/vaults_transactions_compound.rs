use std::collections::HashSet;

use alloy::{eips::BlockNumberOrTag, primitives::address};
use chrono::{TimeZone, Utc};
use eyre::Result;
use sqlx::{PgPool, QueryBuilder, query_scalar};
use async_trait::async_trait;

use crate::{contracts::Comet, strategies::{Stats, ChunkProcessor}};

#[derive(Clone, Copy, Debug)]
enum Direction {
    In,
    Out,
}
impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
        }
    }
}

pub struct VaultsTransactionsCompoundProcessor;

#[async_trait]
impl<P: alloy::providers::Provider + Clone + Send + Sync + 'static> ChunkProcessor<P> for VaultsTransactionsCompoundProcessor {
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats> {
        process_vaults_transactions_chunk(provider, db, from, to).await
    }
}

pub async fn process_vaults_transactions_chunk<P>(
    provider: P,
    db: &PgPool,
    from: u64,
    to: u64,
) -> Result<Stats>
where
    P: alloy::providers::Provider + Clone + Send + Sync + 'static,
{
    let comet_addr = address!("0xE36A30D249f7761327fd973001A32010b521b6Fd");
    let contract = Comet::new(comet_addr, provider.clone());

    let t0 = std::time::Instant::now();

    tracing::info!(from = from, to = to, "processing event range");

    let logs = contract
        .Supply_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()))
        .query()
        .await?;

    if logs.is_empty() {
        tracing::info!(from = from, to = to, "no logs found in range");
        return Ok(Stats::default());
    }

    let mut dsts: Vec<String> = logs
        .iter()
        .map(|(ev, _)| format!("{:#x}", ev.dst).to_lowercase())
        .collect();
    dsts.sort_unstable();
    dsts.dedup();

    // 2) Pide a la DB cuáles 'dst' existen como super_accounts.account
    let existing: Vec<String> = query_scalar!(
        r#"SELECT account FROM super_accounts WHERE lower(account) = ANY($1::text[])"#,
        &dsts
    )
    .fetch_all(db)
    .await?;

    tracing::info!(matches = existing.len(), "super_accounts matches");

    let existing_set: HashSet<String> = existing.into_iter().map(|s| s.to_lowercase()).collect();

    let filtered_logs: Vec<_> = logs
        .into_iter()
        .filter(|(ev, _)| {
            let d = format!("{:#x}", ev.dst).to_lowercase();
            existing_set.contains(&d)
        })
        .collect();

    if filtered_logs.is_empty() {
        tracing::info!(from = from, to = to, "no valid logs found in range");
        return Ok(Stats::default());
    }

    struct Row {
        account_hex: String, // TEXT "0x..."
        token_hex: String,
        amount: sqlx::types::BigDecimal,
        direction: Direction,
        txhash_hex: String,
        txblock: i64,
        block_time: chrono::DateTime<chrono::Utc>,
    }

    let mut rows: Vec<Row> = Vec::with_capacity(filtered_logs.len());

    for (event, raw_log) in filtered_logs {
        rows.push(Row {
            account_hex: event.from.to_string(),
            token_hex: "0x4200000000000000000000000000000000000006".to_string(), // WETH
            amount: event.amount.to_string().parse()?,
            direction: Direction::In.into(),
            txhash_hex: raw_log
                .transaction_hash
                .map(|h| format!("{:#x}", h))
                .unwrap_or_default(),
            txblock: raw_log.block_number.map(|b| b as i64).unwrap_or_default(),
            block_time: raw_log
                .block_timestamp
                .map(|ts| Utc.timestamp_opt(ts as i64, 0).unwrap())
                .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap()),
        })
    }
    let mut qb = QueryBuilder::new(
        "INSERT INTO vaults_transactions (
            account, token, amount, direction, tx_hash, tx_block, block_time
        ) ",
    );
    qb.push_values(rows.iter(), |mut b, row| {
        b.push_bind(&row.account_hex)
            .push_bind(&row.token_hex)
            .push_bind(&row.amount)
            .push_bind(row.direction.as_str())
            .push_bind(&row.txhash_hex)
            .push_bind(row.txblock)
            .push_bind(row.block_time);
    });
    qb.push(" ON CONFLICT (account, token, tx_hash, direction) DO NOTHING");
    let batch_res = qb.build().execute(db).await;
    let took_ms = t0.elapsed().as_millis();

    Ok(Stats {
        logs_found: rows.len(),
        rows_written: batch_res?.rows_affected(),
        from_block: from,
        to_block: to,
        took_ms,
    })
}
