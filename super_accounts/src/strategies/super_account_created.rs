use std::borrow::Cow;

use alloy::{
    eips::BlockNumberOrTag,
    primitives::Address,
};
use async_trait::async_trait;
use eyre::{Ok, Result};
use indexer_core::strategies::{ChunkProcessor, Stats};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder};

use crate::config::super_account_module_addr;
use crate::contracts::SuperChainModule;

#[derive(Clone)]
pub struct SuperAccountCreatedProcessor;

#[async_trait]
impl<P: alloy::providers::Provider + Clone + Send + Sync + 'static> ChunkProcessor<P>
    for SuperAccountCreatedProcessor
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
    let super_chain_module_addr: Address = super_account_module_addr();
    let contract = SuperChainModule::new(super_chain_module_addr, provider.clone());
    let t0 = std::time::Instant::now();

    tracing::info!(from = from, to = to, "processing event range");

    let logs = contract
        .SuperChainSmartAccountCreated_filter()
        .from_block(BlockNumberOrTag::Number(from.into()))
        .to_block(BlockNumberOrTag::Number(to.into()))
        .query()
        .await?;

    if logs.is_empty() {
        tracing::info!(from = from, to = to, "no logs found in range");
        return Ok(Stats::default());
    }
    struct Row {
        account_hex: String,
        username: String,
        eoas: Vec<String>,
        noun_json: serde_json::Value,
        last_update_block_number: Option<i32>,
        last_update_tx_hash: Option<String>,
    }

    let mut rows = Vec::with_capacity(logs.len());
    for (event, raw_log) in logs {
        let (username_cow, nuls) = sanitize_text(&event.superChainId);
        let tx_hex = raw_log.transaction_hash.map(|h| format!("{:#x}", h));
        let block_num = raw_log.block_number;

        if nuls > 0 {
            tracing::warn!(
                nuls = nuls,
                account = format!("{:#x}", event.safe),
                tx = tx_hex,
                block = block_num,
                before_len = event.superChainId.len(),
                after_len = username_cow.len(),
                "sanitized NULs in username"
            );
        }

        let noun_json = json!({
            "background": event.noun.background.to::<u64>(),
            "body":       event.noun.body.to::<u64>(),
            "accessory":  event.noun.accessory.to::<u64>(),
            "head":       event.noun.head.to::<u64>(),
            "glasses":    event.noun.glasses.to::<u64>(),
        });

        rows.push(Row {
            account_hex: format!("{:#x}", event.safe),
            username: username_cow.into_owned(),
            eoas: vec![format!("{:#x}", event.initialOwner)],
            noun_json,
            last_update_block_number: raw_log.block_number.map(|b| b as i32),
            last_update_tx_hash: raw_log.transaction_hash.map(|h| format!("{:#x}", h)),
        });
    }

    let mut qb = QueryBuilder::new(
        "INSERT INTO super_accounts (
            account, nationality, username, eoas, level,
            noun, total_points, total_badges,
            last_update_block_number, last_update_tx_hash
        ) ",
    );

    qb.push_values(rows.iter(), |mut b, r| {
        b.push_bind(&r.account_hex)
            .push_bind(Option::<&str>::None) // nationality NULL
            .push_bind(&r.username)
            .push_bind(&r.eoas) // TEXT[]
            .push_bind(0i32) // level
            .push_bind(&r.noun_json) // JSONB
            .push_bind(0i32) // total_points
            .push_bind(0i32) // total_badges
            .push_bind(r.last_update_block_number)
            .push_bind(&r.last_update_tx_hash);
    });
    qb.push(" ON CONFLICT (account) DO UPDATE SET ");
    qb.push("username = EXCLUDED.username, ");
    qb.push("eoas = EXCLUDED.eoas, ");
    qb.push("noun = EXCLUDED.noun, ");
    qb.push("last_update_block_number = EXCLUDED.last_update_block_number, ");
    qb.push("last_update_tx_hash = EXCLUDED.last_update_tx_hash");

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
