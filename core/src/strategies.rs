use std::fmt::Debug;

use alloy::providers::Provider;
use async_trait::async_trait;
use eyre::Result;
use sqlx::PgPool;
use tracing::info;

pub struct StrategyConfig<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    pub processor: Box<dyn ChunkProcessor<P> + Send + Sync>,
    pub name: &'static str,
    pub from_block: u64,
    pub force_reindex: bool,
}

impl<P> StrategyConfig<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    pub fn new<T>(processor: T, name: &'static str, from_block: u64, force_reindex: bool) -> Self
    where
        T: ChunkProcessor<P> + Send + Sync + 'static,
    {
        Self {
            processor: Box::new(processor),
            name,
            from_block,
            force_reindex,
        }
    }
}

impl<P> Clone for StrategyConfig<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            processor: self.processor.clone(),
            name: self.name,
            from_block: self.from_block,
            force_reindex: self.force_reindex,
        }
    }
}

impl<P> Debug for StrategyConfig<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyConfig")
            .field("name", &self.name)
            .field("from_block", &self.from_block)
            .field("force_reindex", &self.force_reindex)
            .finish()
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
pub trait ChunkProcessor<P: Provider + Clone + Send + Sync + 'static>: Send + Sync {
    async fn process(&self, provider: P, db: &PgPool, from: u64, to: u64) -> Result<Stats>;

    fn box_clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync>;
}

impl<P> Clone for Box<dyn ChunkProcessor<P> + Send + Sync>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync> {
        self.box_clone()
    }
}

#[derive(Clone)]
pub struct IndexedRangeDecorator<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    inner: Box<dyn ChunkProcessor<P> + Send + Sync>,
    strategy_name: &'static str,
    force_reindex: bool,
}

impl<P> IndexedRangeDecorator<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    pub fn new(
        inner: Box<dyn ChunkProcessor<P> + Send + Sync>,
        strategy_name: &'static str,
        force_reindex: bool,
    ) -> Self {
        Self {
            inner,
            strategy_name,
            force_reindex,
        }
    }
}

#[async_trait]
impl<P> ChunkProcessor<P> for IndexedRangeDecorator<P>
where
    P: Provider + Clone + Send + Sync + 'static,
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

        // Delegate to inner processor
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

    fn box_clone(&self) -> Box<dyn ChunkProcessor<P> + Send + Sync> {
        Box::new(Self {
            inner: self.inner.clone(),
            strategy_name: self.strategy_name,
            force_reindex: self.force_reindex,
        })
    }
}
