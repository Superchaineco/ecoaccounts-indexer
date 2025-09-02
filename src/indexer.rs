use alloy::{
    primitives::{Address},
    providers::{Provider, ProviderBuilder},
    rpc::types::BlockNumberOrTag,
};
use eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;
use sqlx::PgPool;
use sqlx::QueryBuilder;
use std::time::Instant;
use std::borrow::Cow;

use crate::contracts::SuperChainModule;


pub async fn sync_from_block(
    rpc_url: &str,
    contract_addr: Address,
    from_block: u64,
    db: &PgPool,
) -> Result<()> {
    let provider = ProviderBuilder::new().connect(rpc_url).await?;
    let contract = SuperChainModule::new(contract_addr, provider.clone());

    let latest_block = provider.get_block_number().await?;
    eyre::ensure!(from_block <= latest_block, "from_block > latest_block");

    let total_blocks = latest_block - from_block;

    let step: u64 = 100_000;

    let bar = ProgressBar::new(total_blocks.into());
    bar.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {percent}% | Block {pos}/{len} | ETA {eta}",
        )?
        .progress_chars("=>-"),
    );

    eprintln!(
        "[sync] rpc_url={} contract={} from_block={} latest_block={} total_blocks={}",
        rpc_url,
        format!("{:#x}", contract_addr),
        from_block,
        latest_block,
        total_blocks
    );

    let mut cur = from_block;
    while cur <= latest_block {
        let chunk_start = cur;
        let chunk_end = (chunk_start + step - 1).min(latest_block);

        let t0 = Instant::now();
        let logs = contract
            .SuperChainSmartAccountCreated_filter()
            .from_block(BlockNumberOrTag::Number(chunk_start.into()))
            .to_block(BlockNumberOrTag::Number(chunk_end.into()))
            .query()
            .await?;
        let dt = t0.elapsed();

        eprintln!(
            "[sync] OK   [{chunk_start}-{chunk_end}] step={} logs={} t={:?}",
            step,
            logs.len(),
            dt
        );

        if !logs.is_empty() {
            struct Row {
                account_hex: String,
                username_clean: String,
                username_orig_len: usize,
                username_nuls: usize,
                eoas: Vec<String>,
                noun_json: serde_json::Value,
                last_update_block_number: Option<i32>,
                last_update_tx_hash: Option<String>,
            }

            let mut rows = Vec::with_capacity(logs.len());
            for (event, raw_log) in logs {
                let (username_cow, nuls) = sanitize_text(&event.superChainId);
                if nuls > 0 {
                    eprintln!(
                        "[sanitize] NULs={} addr={} tx={:?} blk={:?} username_len_before={} username_len_after={}",
                        nuls,
                        format!("{:#x}", event.safe),
                        raw_log.transaction_hash.map(|h| format!("{:#x}", h)),
                        raw_log.block_number,
                        event.superChainId.len(),
                        username_cow.len()
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
                    username_clean: username_cow.into_owned(),
                    username_orig_len: event.superChainId.len(),
                    username_nuls: nuls,
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
                    .push_bind(&r.username_clean) // username saneado
                    .push_bind(&r.eoas) // TEXT[]
                    .push_bind(0i32) // level
                    .push_bind(&r.noun_json) // JSONB
                    .push_bind(0i32) // total_points
                    .push_bind(0i32) // total_badges
                    .push_bind(r.last_update_block_number)
                    .push_bind(&r.last_update_tx_hash);
            });
            qb.push(" ON CONFLICT (account) DO NOTHING");

            let t_db0 = Instant::now();
            let batch_res = qb.build().execute(db).await;

            match batch_res {
                Ok(res) => {
                    eprintln!(
                        "[sync][db] inserted {} rows in {:?}",
                        res.rows_affected(),
                        t_db0.elapsed()
                    );
                }
                Err(e) => {
                    // 3) Fallback: por fila, con LOG DETALLADO para ubicar la fila culpable
                    eprintln!(
                        "[sync][db][batch-err] {} — fallback per-row with verbose logs",
                        e
                    );
                    for r in rows {
                        let per = sqlx::query!(
                            r#"
                    INSERT INTO super_accounts (
                        account, nationality, username, eoas, level,
                        noun, total_points, total_badges,
                        last_update_block_number, last_update_tx_hash
                    ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                    ON CONFLICT (account) DO NOTHING
                    "#,
                            r.account_hex,
                            Option::<&str>::None,
                            r.username_clean,
                            &r.eoas,
                            0,
                            r.noun_json,
                            0,
                            0,
                            r.last_update_block_number,
                            r.last_update_tx_hash
                        )
                        .execute(db)
                        .await;

                        if let Err(pe) = per {
                            eprintln!(
                                "[sync][db][row-err] {pe}\n  account={}\n  username.len={} (orig_len={}, nul_count={})\n  eoas={:?}\n  last_block={:?}\n  last_tx={:?}",
                                r.account_hex,
                                r.username_clean.len(),
                                r.username_orig_len,
                                r.username_nuls,
                                r.eoas,
                                r.last_update_block_number,
                                r.last_update_tx_hash
                            );
                        }
                    }
                }
            }
        }

        bar.inc(chunk_end - chunk_start + 1);

        cur = chunk_end.saturating_add(1);
    }

    bar.finish_with_message("✅ Sync completed.");
    Ok(())
}


// async fn stream(rpc_url: &str) -> Result<()> {
//     let ws = WsConnect::new(rpc_url);
//     let provider = ProviderBuilder::new().connect_ws(ws).await?;

//     let super_chain_badges_contract = SuperChainModule::new(
//         address!("0x1Ee397850c3CA629d965453B3cF102E9A8806Ded"),
//         provider.clone(),
//     );

//     let badge_minter_filter = super_chain_badges_contract
//         .SuperChainSmartAccountCreated_filter()
//         .watch()
//         .await?;

//     let mut stream = badge_minter_filter.into_stream();

//     while let Some(log) = stream.next().await {
//         println!("(stream) BadgeMinted log: {log:#?}");
//     }

//     Ok(())
// }



fn sanitize_text(s: &str) -> (Cow<'_, str>, usize) {
    let mut nul_count = 0usize;
    let cleaned: String = s.chars().filter(|&ch| {
        if ch == '\0' { nul_count += 1; return false; }
        let code = ch as u32;
        !(code < 0x20 && ch != '\n' && ch != '\r' && ch != '\t')
    }).collect();

    if nul_count == 0 && cleaned.len() == s.len() {
        (Cow::Borrowed(s), 0)
    } else {
        (Cow::Owned(cleaned), nul_count)
    }
}
