mod prosperity_account_created;
mod vaults_transactions_stcelo;
mod badges_minted;
mod owner_added;

pub use prosperity_account_created::ProsperityAccountCreatedProcessor;
pub use vaults_transactions_stcelo::VaultsTransactionsStCeloManagerProcessor;
pub use badges_minted::SuperChainBadgesMintedProccesor;
pub use owner_added::OwnerAddedProcessor;