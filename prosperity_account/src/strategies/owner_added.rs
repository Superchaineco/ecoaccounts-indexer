use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
};
use async_trait::async_trait;
use eyre::{Ok, Result};
use indexer_core::strategies::{ChunkProcessor, Stats};
use serde_json;
use sqlx::{PgPool, QueryBuilder};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::config::super_account_module_addr;
use crate::contracts::SuperChainModule;

#[derive(Clone)]
pub struct OwnerAddedProcessor;

#[async_trait]
impl<P: alloy::providers::Provider + Clone + Send + Sync + 'static> ChunkProcessor<P>
    for OwnerAddedProcessor
{
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats> {
        process_owner_added_chunk(provider, db, from, to).await
    }

    fn box_clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync> {
        Box::new(self.clone())
    }
}

pub async fn process_owner_added_chunk<P>(
    provider: P,
    db: &PgPool,
    from: u64,
    to: u64,
) -> Result<Stats>
where
    P: alloy::providers::Provider + Clone + Send + Sync + 'static,
{
    let super_chain_module_addr: Address = super_account_module_addr();
    let contract = SuperChainModule::new(super_chain_module_addr, provider.clone());
    let t0 = std::time::Instant::now();

    tracing::info!(from = from, to = to, "processing event range");

    let logs = contract
        .OwnerAdded_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()))
        .query()
        .await?;

    if logs.is_empty() {
        tracing::info!(from = from, to = to, "no logs found in range");
        return Ok(Stats::default());
    }

    // Group owners by account to avoid "ON CONFLICT cannot affect row a second time" error
    let mut account_owners: HashMap<String, (Vec<String>, String)> = HashMap::new();
    for (event, _raw_log) in &logs {
        let account_hex = format!("{:#x}", event.safe);
        let new_owner_hex = format!("{:#x}", event.newOwner);
        let (username_clean, _) = sanitize_text(&event.superChainId);
        
        account_owners
            .entry(account_hex)
            .or_insert_with(|| (Vec::new(), username_clean.into_owned()))
            .0
            .push(new_owner_hex);
    }

    // Convert to Vec for batching
    let rows: Vec<_> = account_owners.into_iter().collect();

    // Process in smaller batches to avoid parameter limit
    let mut rows_written = 0u64;
    const BATCH_SIZE: usize = 500;

    for chunk in rows.chunks(BATCH_SIZE) {
        let mut qb = QueryBuilder::new(
            "INSERT INTO users (account, eoas, nationality, username, level, noun, total_points, total_badges) ",
        );

        qb.push_values(chunk.iter(), |mut b, (account_hex, (owners, username))| {
            b.push_bind(account_hex)
                .push_bind(owners)
                .push_bind(Option::<&str>::None) // nationality
                .push_bind(username)              // username
                .push_bind(0i32)                  // level
                .push_bind(serde_json::json!({})) // noun
                .push_bind(0i32)                  // total_points
                .push_bind(0i32);                 // total_badges
        });

        qb.push(" ON CONFLICT (account) DO UPDATE SET ");
        qb.push("eoas = (SELECT array_agg(DISTINCT e) FROM unnest(users.eoas || EXCLUDED.eoas) AS e)");

        let batch_res = qb.build().execute(db).await?;
        rows_written += batch_res.rows_affected();
    }

    let took_ms = t0.elapsed().as_millis();
    tracing::info!(
        from = from,
        to = to,
        logs = logs.len(),
        rows_written = rows_written,
        took_ms,
        "chunk processed",
    );
    Ok(Stats {
        logs_found: logs.len(),
        rows_written,
        from_block: from,
        to_block: to,
        took_ms,
    })
}

fn sanitize_text(s: &str) -> (Cow<'_, str>, usize) {
    let mut nul_count = 0usize;
    let cleaned: String = s
        .chars()
        .filter(|&ch| {
            if ch == '\0' {
                nul_count += 1;
                return false;
            }
            let code = ch as u32;
            !(code < 0x20 && ch != '\n' && ch != '\r' && ch != '\t')
        })
        .collect();

    if nul_count == 0 && cleaned.len() == s.len() {
        (Cow::Borrowed(s), 0)
    } else {
        (Cow::Owned(cleaned), nul_count)
    }
}
