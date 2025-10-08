use std::borrow::Cow;

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, address},
    rpc::types::Log,
};
use async_trait::async_trait;
use eyre::{Ok, Result};
use futures_util::future::try_join;
use indexer_core::strategies::{ChunkProcessor, Stats};
use sqlx::{PgPool, QueryBuilder};

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
    let super_chain_badges_addr: Address = address!("0x03e2c563cf77e3Cdc0b7663cEE117dA14ea60848");
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
                let tx_hex = log.transaction_hash.map(|h| format!("{:#x}", h)).unwrap_or_default();
                let block_num = log.block_number.unwrap_or(0) as i32;
                rows.push(Row {
                    badge_id: ev.badgeId.to::<i32>(),
                    account: format!("{:#x}", ev.user).to_lowercase(),
                    tier: ev.initialTier.to::<i32>(),
                    points: ev.points.to::<i32>(),
                    block_number: block_num,
                    tx_hash: tx_hex,
                    claimed_at: chrono::Utc::now(),
                });
            }
            Event::Updated(ev, log) => {
                let tx_hex = log.transaction_hash.map(|h| format!("{:#x}", h)).unwrap_or_default();
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

    let mut qb = QueryBuilder::new(
        "INSERT INTO badge_claims (
            badge_id, account, tier, points, block_number, tx_hash, claimed_at
        ) ",
    );

    qb.push_values(rows.iter(), |mut b, r| {
        b.push_bind(r.badge_id)
            .push_bind(&r.account)
            .push_bind(r.tier)
            .push_bind(r.points)
            .push_bind(r.block_number)
            .push_bind(&r.tx_hash)
            .push_bind(r.claimed_at);
    });
    qb.push(" ON CONFLICT (badge_id, tier, account, block_number) DO NOTHING");

    let batch_res = qb.build().execute(db).await;
    let took_ms = t0.elapsed().as_millis();
    tracing::info!(
        from = from,
        to = to,
        logs = rows.len(),
        rows_written = batch_res.as_ref().map(|r| r.rows_affected()).unwrap_or(0),
        took_ms,
        "chunk processed",
    );
    Ok(Stats {
        logs_found: rows.len(),
        rows_written: batch_res?.rows_affected(),
        from_block: from,
        to_block: to,
        took_ms,
    })
}
