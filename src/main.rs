mod contracts;
mod db;
mod indexer;
mod strategies;

use std::env;

use alloy::providers::ProviderBuilder;
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
    let from_block = 140060441;
    let provider = ProviderBuilder::new().connect(&rpc_url).await?;

    info!(rpc_url = %rpc_url,  from_block, "launching indexer");

    run_indexer_and_follow(provider, &db, from_block, 100_000, 4, 5).await?;

    Ok(())
}
