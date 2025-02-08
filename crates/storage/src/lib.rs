use std::str::FromStr;

use alloy::primitives::{Address, BlockNumber};
use anyhow::Context;
use dotenvy_macro::dotenv;
use pool::{PoolRecord, ProtocolType};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions, query};

pub mod pool;

pub struct Storage {
    pool: Pool<Postgres>,
}

impl Storage {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self::new_with_pool(
            PgPoolOptions::new()
                .max_connections(5)
                .connect(dotenv!("DATABASE_URL"))
                .await
                .context("failed to connect to storage database")?,
        ))
    }

    pub fn new_with_pool(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn get_last_discovered_block(
        &self,
        protocol: ProtocolType,
    ) -> anyhow::Result<BlockNumber> {
        Ok(query!(
            "SELECT last_block FROM discovered WHERE protocol = $1",
            protocol as i32
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to get last discovered block")?
        .map(|record| record.last_block as u64)
        .unwrap_or_default())
    }

    pub async fn set_last_discovered_block(
        &self,
        block: BlockNumber,
        protocol: ProtocolType,
    ) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO discovered (protocol, last_block)
            VALUES ($1, $2)
            ON CONFLICT (protocol) DO UPDATE
            SET last_block = $2
            "#,
            protocol as i32,
            block as i64
        )
        .execute(&self.pool)
        .await
        .context("failed to set last discovered block")?;

        Ok(())
    }

    pub async fn insert_pool(&self, pool: &PoolRecord) -> anyhow::Result<()> {
        query!(
            "INSERT INTO pool (address, protocol) VALUES ($1, $2)",
            pool.address.to_string(),
            pool.protocol as i32
        )
        .execute(&self.pool)
        .await
        .context("failed to insert pool")?;

        Ok(())
    }

    pub async fn insert_pools(&self, pools: &[PoolRecord]) -> anyhow::Result<()> {
        for pool in pools {
            self.insert_pool(pool).await?;
        }

        Ok(())
    }

    pub async fn get_pools(&self) -> anyhow::Result<Vec<PoolRecord>> {
        Ok(query!("SELECT address, protocol FROM pool")
            .fetch_all(&self.pool)
            .await
            .context("failed to get pools")?
            .into_iter()
            .map(|record| PoolRecord {
                address: Address::from_str(&record.address).expect("address is correct"),
                protocol: record.protocol.into(),
            })
            .collect())
    }

    pub async fn get_filtered_pools(&self, hash: u64) -> anyhow::Result<Option<Vec<PoolRecord>>> {
        let stored_hash = query!("SELECT filter_hash FROM state")
            .fetch_one(&self.pool)
            .await
            .context("failed to get stored filter hash")?
            .filter_hash as u64;

        if stored_hash != hash {
            return Ok(None);
        }

        Ok(Some(
            query!("SELECT address, protocol FROM filtered_pool")
                .fetch_all(&self.pool)
                .await
                .context("failed to get filtered pools")?
                .into_iter()
                .map(|record| PoolRecord {
                    address: Address::from_str(&record.address).expect("address is correct"),
                    protocol: record.protocol.into(),
                })
                .collect(),
        ))
    }

    pub async fn insert_filtered_pools(
        &self,
        pools: &[PoolRecord],
        hash: u64,
    ) -> anyhow::Result<()> {
        query!("UPDATE state SET filter_hash = $1", hash as i64)
            .execute(&self.pool)
            .await
            .context("failed to update filter hash")?;

        query!("DELETE FROM filtered_pool")
            .execute(&self.pool)
            .await
            .context("failed to clear filtered pools")?;

        for pool in pools {
            self.insert_filtered_pool(pool).await?;
        }

        Ok(())
    }

    async fn insert_filtered_pool(&self, pool: &PoolRecord) -> anyhow::Result<()> {
        query!(
            "INSERT INTO filtered_pool (address, protocol) VALUES ($1, $2)",
            pool.address.to_string(),
            pool.protocol as i32
        )
        .execute(&self.pool)
        .await
        .context("failed to insert filtered pool")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{BlockNumber, address};
    use sqlx::{Pool, Postgres};

    use crate::{
        Storage,
        pool::{PoolRecord, ProtocolType},
    };

    #[sqlx::test]
    async fn it_updates_last_discovered_block(pool: Pool<Postgres>) {
        let storage = Storage::new_with_pool(pool);

        let set = async |block: BlockNumber, protocol: ProtocolType| {
            storage
                .set_last_discovered_block(block, protocol)
                .await
                .unwrap()
        };

        let get = async |protocol: ProtocolType| {
            storage.get_last_discovered_block(protocol).await.unwrap()
        };

        assert_eq!(get(ProtocolType::UniswapV2).await, 0);

        set(1337, ProtocolType::UniswapV2).await;
        assert_eq!(get(ProtocolType::UniswapV2).await, 1337);

        set(777, ProtocolType::UniswapV2).await;
        assert_eq!(get(ProtocolType::UniswapV2).await, 777);

        set(100, ProtocolType::UniswapV3).await;
        assert_eq!(get(ProtocolType::UniswapV3).await, 100);
    }

    #[sqlx::test]
    async fn it_inserts_pools(pool: Pool<Postgres>) {
        let storage = Storage::new_with_pool(pool);

        let pool = PoolRecord {
            address: address!("1111111111111111111111111111111111111111"),
            protocol: ProtocolType::UniswapV2,
        };

        storage.insert_pool(&pool).await.unwrap();

        let pools = storage.get_pools().await.unwrap();

        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].address, pool.address);
        assert_eq!(pools[0].protocol, pool.protocol);

        assert_eq!(storage.insert_pool(&pool).await.is_err(), true);

        let pools = vec![
            PoolRecord {
                address: address!("2222222222222222222222222222222222222222"),
                protocol: ProtocolType::UniswapV2,
            },
            PoolRecord {
                address: address!("3333333333333333333333333333333333333333"),
                protocol: ProtocolType::UniswapV3,
            },
        ];

        storage.insert_pools(&pools).await.unwrap();

        assert_eq!(storage.get_pools().await.unwrap().len(), 3);
    }

    #[sqlx::test]
    async fn it_handles_filtered_pools(pool: Pool<Postgres>) {
        let storage = Storage::new_with_pool(pool);

        assert_eq!(
            storage.get_filtered_pools(100).await.unwrap().is_none(),
            true
        );

        let pools = vec![
            PoolRecord {
                address: address!("1111111111111111111111111111111111111111"),
                protocol: ProtocolType::UniswapV2,
            },
            PoolRecord {
                address: address!("2222222222222222222222222222222222222222"),
                protocol: ProtocolType::UniswapV3,
            },
        ];

        storage.insert_filtered_pools(&pools, 1337).await.unwrap();

        assert_eq!(
            storage.get_filtered_pools(100).await.unwrap().is_none(),
            true
        );

        assert_eq!(
            storage
                .get_filtered_pools(1337)
                .await
                .unwrap()
                .unwrap()
                .len(),
            pools.len()
        );
    }
}
