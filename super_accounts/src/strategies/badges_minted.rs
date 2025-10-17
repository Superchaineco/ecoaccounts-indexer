use std::collections::HashMap;

use alloy::{eips::BlockNumberOrTag, primitives::Address, rpc::types::Log};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use eyre::{Ok, Result};
use futures_util::future::try_join;
use indexer_core::strategies::{ChunkProcessor, Stats};
use sqlx::{PgPool, QueryBuilder};

use crate::config::badges_addr;
use crate::contracts::SuperChainBadges::{self, BadgeMinted, BadgeTierUpdated};

#[derive(Clone)]
pub struct SuperChainBadgesMintedProccesor;

#[async_trait]
impl<P: alloy::providers::Provider + Clone + Send + Sync + 'static> ChunkProcessor<P>
    for SuperChainBadgesMintedProccesor
{
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats> {
        process_super_account_created_chunk(provider, db, from, to).await
    }

    fn box_clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync> {
        Box::new(self.clone())
    }
}

pub async fn process_super_account_created_chunk<P>(
    provider: P,
    db: &PgPool,
    from: u64,
    to: u64,
) -> Result<Stats>
where
    P: alloy::providers::Provider + Clone + Send + Sync + 'static,
{
    let super_chain_badges_addr: Address = badges_addr();
    let contract = SuperChainBadges::new(super_chain_badges_addr, provider.clone());
    let t0 = std::time::Instant::now();

    tracing::info!(from = from, to = to, "processing event range");

    let mint_filter = contract
        .BadgeMinted_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()));

    let update_filter = contract
        .BadgeTierUpdated_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()));

    let (mint_logs, update_logs) = try_join(mint_filter.query(), update_filter.query()).await?;

    enum Event {
        Minted(BadgeMinted, Log),
        Updated(BadgeTierUpdated, Log),
    }

    let all_logs: Vec<Event> = mint_logs
        .into_iter()
        .map(|(ev, log)| Event::Minted(ev, log))
        .chain(
            update_logs
                .into_iter()
                .map(|(ev, log)| Event::Updated(ev, log)),
        )
        .collect();

    if all_logs.is_empty() {
        tracing::info!(from = from, to = to, "no logs found in range");
        return Ok(Stats::default());
    }

    let mut block_timestamps: HashMap<u64, chrono::DateTime<chrono::Utc>> = HashMap::new();
    struct Row {
        badge_id: i32,
        account: String,
        tier: i32,
        points: i32,
        block_number: i32,
        tx_hash: String,
        claimed_at: chrono::DateTime<chrono::Utc>,
    }

    let mut rows = Vec::with_capacity(all_logs.len());
    for event in all_logs {
        match event {
            Event::Minted(ev, log) => {
                let tx_hex = log
                    .transaction_hash
                    .map(|h| format!("{:#x}", h))
                    .unwrap_or_default();
                let block_num = log.block_number.unwrap_or(0) as i32;
                rows.push(Row {
                    badge_id: ev.badgeId.to::<i32>(),
                    account: format!("{:#x}", ev.user).to_lowercase(),
                    tier: ev.initialTier.to::<i32>(),
                    points: ev.points.to::<i32>(),
                    block_number: block_num,
                    tx_hash: tx_hex,
                    claimed_at: if let Some(ts) = log.block_timestamp {
                        Utc.timestamp_opt(ts as i64, 0).unwrap()
                    } else if let Some(block_num) = log.block_number {
                        // Usar cache o fetch si no existe
                        if let Some(&cached_time) = block_timestamps.get(&block_num) {
                            cached_time
                        } else {
                            // Fetch block timestamp
                            let timestamp = provider
                                .get_block_by_number(BlockNumberOrTag::Number(block_num))
                                .await
                                .ok()
                                .flatten()
                                .map(|b| b.header.timestamp)
                                .unwrap_or(0);
                            let datetime = Utc.timestamp_opt(timestamp as i64, 0).unwrap();
                            block_timestamps.insert(block_num, datetime);
                            datetime
                        }
                    } else {
                        Utc.timestamp_opt(0, 0).unwrap()
                    },
                });
            }
            Event::Updated(ev, log) => {
                let tx_hex = log
                    .transaction_hash
                    .map(|h| format!("{:#x}", h))
                    .unwrap_or_default();
                let block_num = log.block_number.unwrap_or(0) as i32;
                rows.push(Row {
                    badge_id: ev.badgeId.to::<i32>(),
                    account: format!("{:#x}", ev.user).to_lowercase(),
                    tier: ev.tier.to::<i32>(),
                    points: ev.points.to::<i32>(),
                    block_number: block_num,
                    tx_hash: tx_hex,
                    claimed_at: chrono::Utc::now(),
                });
            }
        }
    }

    const MAX_PARAMS: usize = u16::MAX as usize;
    const PARAMS_PER_ROW: usize = 7;

    const MAX_ROWS_PER_BATCH: usize = MAX_PARAMS / PARAMS_PER_ROW;

    let mut total_rows_written: u64 = 0;

    if !rows.is_empty() {
        for chunk in rows.chunks(MAX_ROWS_PER_BATCH) {
            let mut qb = QueryBuilder::new(
                "INSERT INTO badge_claims (
                    badge_id, account, tier, points, block_number, tx_hash, claimed_at
                ) ",
            );

            qb.push_values(chunk.iter(), |mut b, r| {
                b.push_bind(r.badge_id)
                    .push_bind(&r.account)
                    .push_bind(r.tier)
                    .push_bind(r.points)
                    .push_bind(r.block_number)
                    .push_bind(&r.tx_hash)
                    .push_bind(r.claimed_at);
            });

            qb.push(" ON CONFLICT (badge_id, tier, account, block_number) DO NOTHING");

            let res = qb.build().execute(db).await?;
            total_rows_written = total_rows_written.saturating_add(res.rows_affected());
        }
    }

    let took_ms = t0.elapsed().as_millis();
    tracing::info!(
        from = from,
        to = to,
        logs = rows.len(),
        rows_written = total_rows_written,
        took_ms,
        "chunk processed",
    );
    Ok(Stats {
        logs_found: rows.len(),
        rows_written: total_rows_written,
        from_block: from,
        to_block: to,
        took_ms,
    })
}
