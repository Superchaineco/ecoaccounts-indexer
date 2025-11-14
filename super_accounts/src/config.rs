use std::env;

use alloy::primitives::{Address, address};

/// Dirección de contrato y helpers para las strategies.
/// Lee de variables de entorno específicas y hace fallback a constantes.

pub fn vaults_comet_addr() -> Address {
    env::var("STRAT_VAULTS_TRANSACTIONS_COMPOUND_ADDR")
        .ok()
        .and_then(|s| s.parse::<Address>().ok())
        .unwrap_or(address!("0xE36A30D249f7761327fd973001A32010b521b6Fd"))
}

pub fn super_account_module_addr() -> Address {
    env::var("STRAT_SUPER_ACCOUNT_CREATED_ADDR")
        .ok()
        .and_then(|s| s.parse::<Address>().ok())
        .unwrap_or(address!("0x1Ee397850c3CA629d965453B3cF102E9A8806Ded"))
}

pub fn badges_addr() -> Address {
    env::var("STRAT_BADGES_MINTED_ADDR")
        .ok()
        .and_then(|s| s.parse::<Address>().ok())
        .unwrap_or(address!("0x03e2c563cf77e3Cdc0b7663cEE117dA14ea60848"))
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
