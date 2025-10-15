use std::env;

use alloy::primitives::{Address, address};

// Minimal config helpers for prosperity_account

pub fn super_account_module_addr() -> Address {
    env::var("STRAT_SUPER_ACCOUNT_CREATED_ADDR")
        .ok()
        .and_then(|s| s.parse::<Address>().ok())
        .unwrap_or(address!("0x58f5805b5072C3Dd157805132714E1dF40E79c66"))
}

pub fn st_celo_addr() -> Address {
    env::var("STRAT_VAULTS_TRANSACTIONS_STCELO_ADDR")
        .ok()
        .and_then(|s| s.parse::<Address>().ok())
        .unwrap_or(address!("0xC668583dcbDc9ae6FA3CE46462758188adfdfC24"))
}

pub fn read_block(key: &str, fallback: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(fallback)
}

pub fn read_bool(key: &str, fallback: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(fallback)
}
