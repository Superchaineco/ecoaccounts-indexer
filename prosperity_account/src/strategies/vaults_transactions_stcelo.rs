use std::collections::HashSet;

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, address},
    rpc::types::Log,
};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use eyre::Result;
use futures_util::try_join;
use sqlx::{PgPool, QueryBuilder, query_scalar};
use indexer_core::strategies::{ChunkProcessor, Stats};

use crate::{
    contracts::StCelo::{self, Transfer},
};

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

const ST_CELO_ADDRESS: &str = "0xC668583dcbDc9ae6FA3CE46462758188adfdfC24";

#[derive(Clone)]
pub struct VaultsTransactionsStCeloManagerProcessor;


#[async_trait]
impl<P: alloy::providers::Provider + Clone + Send + Sync + 'static> ChunkProcessor<P>
    for VaultsTransactionsStCeloManagerProcessor
{
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats> {
        process_vaults_transactions_chunk(provider, db, from, to).await
    }

    fn box_clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync> {
        Box::new(self.clone())
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
    let st_celo_contract = address!("0xC668583dcbDc9ae6FA3CE46462758188adfdfC24");

    let st_celo_manager_contract = StCelo::new(st_celo_contract, provider.clone());

    let t0 = std::time::Instant::now();

    tracing::info!(from = from, to = to, "processing event range");
    let supply_filter = st_celo_manager_contract
        .Transfer_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()))
        .topic1(Address::ZERO);
    let withdraw_filter = st_celo_manager_contract
        .Transfer_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()))
        .topic1(address!("0xeA280B39437a64473a0C77949759E6629eD1Dc73"));

    let (supply_logs, withdraw_logs) = try_join!(supply_filter.query(), withdraw_filter.query())?;

    enum Event {
        Deposit(Transfer, Log),
        Withdraw(Transfer, Log),
    }

    let all_logs: Vec<Event> = supply_logs
        .into_iter()
        .map(|(ev, log)| Event::Deposit(ev, log))
        .chain(
            withdraw_logs
                .into_iter()
                .map(|(ev, log)| Event::Withdraw(ev, log)),
        )
        .collect();

    if all_logs.is_empty() {
        tracing::info!(from = from, to = to, "no logs found in range");
        return Ok(Stats::default());
    }

    let mut dsts: Vec<String> = all_logs
        .iter()
        .map(|event| match event {
            Event::Deposit(ev, _) => format!("{:#x}", ev.to).to_lowercase(),
            Event::Withdraw(ev, _) => format!("{:#x}", ev.from).to_lowercase(),
        })
        .collect();
    dsts.sort_unstable();
    dsts.dedup();

    // 2) Pide a la DB cuáles 'dst' existen como super_accounts.account
    let existing: Vec<String> = query_scalar!(
        r#"SELECT account FROM prosperity_account WHERE lower(account) = ANY($1::text[])"#,
        &dsts
    )
    .fetch_all(db)
    .await?;

    tracing::info!(matches = existing.len(), "prosperity_account matches");

    let existing_set: HashSet<String> = existing.into_iter().map(|s| s.to_lowercase()).collect();

    let filtered_logs: Vec<_> = all_logs
        .into_iter()
        .filter(|event| match event {
            Event::Deposit(ev, _) => {
                let d = format!("{:#x}", ev.to).to_lowercase();
                existing_set.contains(&d)
            }
            Event::Withdraw(ev, _) => {
                let d = format!("{:#x}", ev.from).to_lowercase();
                existing_set.contains(&d)
            }
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

    for event in filtered_logs {
        let (direction, account_hex, amount, log) = match event {
            Event::Deposit(ev, log) => (
                Direction::In,
                ev.to.to_string(),
                ev.value.to_string(),
                log,
            ),
            Event::Withdraw(ev, log) => (
                Direction::Out,
                ev.from.to_string(),
                ev.value.to_string(),
                log,
            ),
        };
        rows.push(Row {
            account_hex,
            token_hex: ST_CELO_ADDRESS.to_string(),
            amount: amount.parse()?,
            direction,
            txhash_hex: log
                .transaction_hash
                .map(|h| format!("{:#x}", h))
                .unwrap_or_default(),
            txblock: log.block_number.map(|b| b as i64).unwrap_or_default(),
            block_time: log
                .block_timestamp
                .map(|ts| Utc.timestamp_opt(ts as i64, 0).unwrap())
                .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap()),
        })
    }
    let mut qb: QueryBuilder<'_, sqlx::Postgres> = QueryBuilder::new(
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
