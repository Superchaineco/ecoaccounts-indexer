mod contracts;
mod strategies;
mod config;

use std::env;

use alloy::providers::ProviderBuilder;
use dotenv::dotenv;
use eyre::Result;
use indexer_core::db::connect_db;
use indexer_core::indexer;
use indexer_core::strategies::StrategyConfig;

use crate::indexer::run_indexer_and_follow;
use crate::strategies::{
    ProsperityAccountCreatedProcessor, VaultsTransactionsStCeloManagerProcessor,
};
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
    let strategies = vec![
        StrategyConfig::new(
            *Box::new(ProsperityAccountCreatedProcessor),
            "prosperity_account_created",
            config::read_block("STRAT_PROSPERITY_ACCOUNT_CREATED_FROM", 34_050_000),
            config::read_bool("STRAT_PROSPERITY_ACCOUNT_CREATED_REINDEX", false),
        ),
        StrategyConfig::new(
            *Box::new(VaultsTransactionsStCeloManagerProcessor),
            "vaults_transactions_stcelo",
            config::read_block("STRAT_VAULTS_TRANSACTIONS_STCELO_FROM", 34_050_000),
            config::read_bool("STRAT_VAULTS_TRANSACTIONS_STCELO_REINDEX", false),
        ),
    ];
    let provider = ProviderBuilder::new().connect(&rpc_url).await?;

    info!(rpc_url = %rpc_url, strategies = ?strategies, "launching indexer");

    run_indexer_and_follow(provider, &db, strategies, 100_000, 4, 5).await?;

    Ok(())
}
