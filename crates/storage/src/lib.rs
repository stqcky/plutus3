use std::str::FromStr;

use alloy::primitives::{Address, BlockNumber};
use anyhow::Context;
use dotenvy_macro::dotenv;
use hashbrown::HashMap;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions, query};

pub struct Storage {
    pool: Pool<Postgres>,
}

pub struct IdentifiedLiquidityPool {
    pub address: Address,
    pub protocol: String,
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

    pub async fn get_last_discovered_block(&self, protocol: &str) -> anyhow::Result<BlockNumber> {
        Ok(query!(
            "SELECT last_block FROM discovered WHERE protocol = $1",
            protocol
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to get last discovered block")?
        .map(|record| record.last_block as u64)
        .unwrap_or_default())
    }

    pub async fn get_last_discovered_blocks(
        &self,
        protocols: &[String],
    ) -> anyhow::Result<HashMap<String, BlockNumber>> {
        let mut last_discovered_blocks = HashMap::default();

        for protocol in protocols.iter().cloned() {
            last_discovered_blocks.insert(
                protocol.to_string(),
                self.get_last_discovered_block(&protocol).await?,
            );
        }

        Ok(last_discovered_blocks)
    }

    pub async fn set_last_discovered_block(
        &self,
        block: BlockNumber,
        protocol: &str,
    ) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO discovered (protocol, last_block)
            VALUES ($1, $2)
            ON CONFLICT (protocol) DO UPDATE
            SET last_block = $2
            "#,
            protocol,
            block as i64
        )
        .execute(&self.pool)
        .await
        .context("failed to set last discovered block")?;

        Ok(())
    }

    pub async fn insert_pool(&self, pool: Address, protocol: &str) -> anyhow::Result<()> {
        query!(
            "INSERT INTO pool (address, protocol) VALUES ($1, $2)",
            pool.to_string(),
            protocol
        )
        .execute(&self.pool)
        .await
        .context("failed to insert pool")?;

        Ok(())
    }

    pub async fn insert_pools(&self, pools: &HashMap<String, Vec<Address>>) -> anyhow::Result<()> {
        for (protocol, pools) in pools {
            for pool in pools {
                self.insert_pool(*pool, &protocol).await?;
            }
        }

        Ok(())
    }

    pub async fn get_pools(&self) -> anyhow::Result<Vec<IdentifiedLiquidityPool>> {
        Ok(query!("SELECT address, protocol FROM pool")
            .fetch_all(&self.pool)
            .await
            .context("failed to get pools")?
            .into_iter()
            .map(|record| IdentifiedLiquidityPool {
                address: Address::from_str(&record.address).expect("address is correct"),
                protocol: record.protocol.into(),
            })
            .collect())
    }

    pub async fn get_filtered_pools(
        &self,
        hash: u64,
    ) -> anyhow::Result<Option<Vec<IdentifiedLiquidityPool>>> {
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
                .map(|record| IdentifiedLiquidityPool {
                    address: Address::from_str(&record.address).expect("address is correct"),
                    protocol: record.protocol.into(),
                })
                .collect(),
        ))
    }

    pub async fn insert_filtered_pools(
        &self,
        pools: &[IdentifiedLiquidityPool],
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

    async fn insert_filtered_pool(&self, pool: &IdentifiedLiquidityPool) -> anyhow::Result<()> {
        query!(
            "INSERT INTO filtered_pool (address, protocol) VALUES ($1, $2)",
            pool.address.to_string(),
            &pool.protocol
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
    use hashbrown::HashMap;
    use sqlx::{Pool, Postgres};

    use crate::{IdentifiedLiquidityPool, Storage};

    #[sqlx::test]
    async fn it_updates_last_discovered_block(pool: Pool<Postgres>) {
        let storage = Storage::new_with_pool(pool);

        let set = async |block: BlockNumber, protocol: &str| {
            storage
                .set_last_discovered_block(block, protocol.into())
                .await
                .unwrap()
        };

        let get = async |protocol: &str| {
            storage
                .get_last_discovered_block(protocol.into())
                .await
                .unwrap()
        };

        assert_eq!(get("uniswap_v2").await, 0);

        set(1337, "uniswap_v2").await;
        assert_eq!(get("uniswap_v2").await, 1337);

        set(777, "uniswap_v2").await;
        assert_eq!(get("uniswap_v2").await, 777);

        set(100, "uniswap_v3").await;
        assert_eq!(get("uniswap_v3").await, 100);
    }

    #[sqlx::test]
    async fn it_inserts_pools(pool: Pool<Postgres>) {
        let storage = Storage::new_with_pool(pool);

        storage
            .insert_pool(
                address!("1111111111111111111111111111111111111111"),
                "uniswap_v2",
            )
            .await
            .unwrap();

        let pools = storage.get_pools().await.unwrap();

        assert_eq!(pools.len(), 1);
        assert_eq!(
            pools[0].address,
            address!("1111111111111111111111111111111111111111")
        );
        assert_eq!(pools[0].protocol, "uniswap_v2");

        assert_eq!(
            storage
                .insert_pool(
                    address!("1111111111111111111111111111111111111111"),
                    "uniswap_v2",
                )
                .await
                .is_err(),
            true
        );

        storage
            .insert_pools(&HashMap::from_iter([
                ("uniswap_v2".to_string(), vec![address!(
                    "2222222222222222222222222222222222222222"
                )]),
                ("uniswap_v3".to_string(), vec![address!(
                    "3333333333333333333333333333333333333333"
                )]),
            ]))
            .await
            .unwrap();

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
            IdentifiedLiquidityPool {
                address: address!("1111111111111111111111111111111111111111"),
                protocol: "uniswap_v2".into(),
            },
            IdentifiedLiquidityPool {
                address: address!("2222222222222222222222222222222222222222"),
                protocol: "uniswap_v3".into(),
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
