mod contracts;
mod db;
mod indexer;
mod strategies;

use std::env;

use alloy::{primitives::address, providers::ProviderBuilder};
use db::connect_db;
use dotenv::dotenv;
use eyre::Result;

use crate::indexer::run_indexer_and_follow;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("starting application");

    let db = connect_db().await?;

    let rpc_url = env::var("RPC_URL")?;
    let contract = address!("0x1Ee397850c3CA629d965453B3cF102E9A8806Ded");
    let from_block = 140018050;
    let provider = ProviderBuilder::new().connect(&rpc_url).await?;

    info!(rpc_url = %rpc_url, contract = %format!("{:#x}", contract), from_block, "launching indexer");

    run_indexer_and_follow(provider, &db, contract, from_block, 100_000, 4, 5).await?;

    Ok(())
}
