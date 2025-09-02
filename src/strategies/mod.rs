mod super_account_created;
pub use super_account_created::process_super_account_created_chunk;

#[derive(Default, Debug)]
pub struct Stats {
    pub logs_found: usize,
    pub rows_written: u64,
    pub from_block: u64,
    pub to_block: u64,
    pub took_ms: u128,
}
