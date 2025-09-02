mod db;
mod indexer;


use std::env;

use alloy::primitives::address;
use db::connect_db;
use eyre::Result;
use indexer::sync_from_block;
use dotenv::dotenv;


#[tokio::main]
async fn main() -> Result<()> {
dotenv().ok();
let db = connect_db().await?;


let rpc_url =  env::var("RPC_URL")?;
let contract = address!("0x1Ee397850c3CA629d965453B3cF102E9A8806Ded");
let from_block = 125901332;


sync_from_block(&rpc_url, contract, from_block, &db).await?;


Ok(())
}