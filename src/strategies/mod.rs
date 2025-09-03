mod super_account_created;
mod vaults_transactions_compound;

pub use super_account_created::SuperAccountCreatedProcessor;
pub use vaults_transactions_compound::VaultsTransactionsCompoundProcessor;

use alloy::providers::Provider;
use async_trait::async_trait;
use eyre::Result;
use sqlx::PgPool;
use tracing::info;

#[derive(Clone, Debug)]
pub struct StrategyConfig{
    pub name: &'static str,
    pub from_block: u64,
    pub force_reindex: bool,
}

impl StrategyConfig {
    pub fn new(name: &'static str, from_block: u64, force_reindex: bool) -> Self {
        Self {
            name,
            from_block,
            force_reindex,
        }
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub logs_found: usize,
    pub rows_written: u64,
    pub from_block: u64,
    pub to_block: u64,
    pub took_ms: u128,
}

#[async_trait]
pub trait ChunkProcessor<P: Provider + Clone + Send + Sync + 'static> {
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats>;
}

pub struct IndexedRangeDecorator<T> {
    inner: T,
    strategy_name: &'static str,
    force_reindex: bool,
}

impl<T> IndexedRangeDecorator<T> {
    pub fn new(inner: T, strategy_name: &'static str, force_reindex: bool) -> Self {
        Self {
            inner,
            strategy_name,
            force_reindex,
        }
    }
}

#[async_trait]
impl<P: Provider + Clone + Send + Sync + 'static, T: ChunkProcessor<P> + Send + Sync>
    ChunkProcessor<P> for IndexedRangeDecorator<T>
{
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats> {
        if !self.force_reindex {
            // Verificar si el rango ya está cubierto por la fila de la estrategia
            let row: Option<(i64, i64)> = sqlx::query_as(
                "SELECT from_block, to_block FROM indexed_ranges WHERE strategy_name = $1",
            )
            .bind(self.strategy_name)
            .fetch_optional(db)
            .await?;

            if let Some((db_from, db_to)) = row {
                if (from as i64) >= db_from && (to as i64) <= db_to {
                    info!(
                        from,
                        to,
                        db_from,
                        db_to,
                        strategy = self.strategy_name,
                        "range already indexed, skipping"
                    );
                    return Ok(Stats::default());
                }
            }
        }

        let result = self.inner.process(provider, db, from, to).await?;

        // Actualizar/insertar la fila con el rango acumulado
        sqlx::query(
            "INSERT INTO indexed_ranges (strategy_name, from_block, to_block, last_updated) 
             VALUES ($1, $2, $3, NOW()) 
             ON CONFLICT (strategy_name) DO UPDATE 
             SET from_block = LEAST(indexed_ranges.from_block, EXCLUDED.from_block),
                 to_block = GREATEST(indexed_ranges.to_block, EXCLUDED.to_block),
                 last_updated = NOW()",
        )
        .bind(self.strategy_name)
        .bind(from as i64)
        .bind(to as i64)
        .execute(db)
        .await?;

        Ok(result)
    }
}
