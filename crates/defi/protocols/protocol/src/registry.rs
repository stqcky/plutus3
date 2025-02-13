use std::{sync::Arc, time::Instant};

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId, U256, address},
    providers::Provider,
};
use anyhow::Context;
use futures::future;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;
use plutus_evm::{EVM, errors::EvmCallError};
use plutus_storage::{IdentifiedLiquidityPool, Storage};
use tokio::{sync::Semaphore, task::spawn_blocking};

use crate::{DiscoverableProtocol, ProtocolFactory, filtering::PoolFilter, pool::LiquidityPool};

pub const POOL_CREATION_TASK_LIMIT: usize = 50;

pub struct ProtocolRegistry<P> {
    chain_id: ChainId,
    provider: P,
    protocols: HashMap<String, Box<dyn DiscoverableProtocol<P>>>,
}

impl<P: Provider> ProtocolRegistry<P> {
    pub async fn new(provider: P) -> anyhow::Result<Self> {
        Ok(Self {
            chain_id: provider
                .get_chain_id()
                .await
                .context("failed to get chain id")?,
            protocols: HashMap::default(),
            provider,
        })
    }

    pub fn with<F: ProtocolFactory<P> + 'static>(mut self) -> anyhow::Result<Self> {
        self.protocols.insert(
            F::IDENTIFIER.into(),
            Box::new(
                F::new(self.chain_id)
                    .context(format!("failed to create protocol `{}`", F::IDENTIFIER))?,
            ),
        );

        Ok(self)
    }

    pub async fn discover(
        &self,
        discovered_blocks: &HashMap<String, BlockNumber>,
        to: BlockNumber,
    ) -> anyhow::Result<HashMap<String, Vec<Address>>>
    where
        P: Clone,
    {
        let mut discovered = HashMap::new();

        for (identifier, protocol) in &self.protocols {
            let discovered_block = *discovered_blocks.get(identifier).unwrap_or(&0);

            if discovered_block > to {
                continue;
            }

            discovered.insert(
                identifier.to_owned(),
                protocol
                    .discover(discovered_block, to, self.provider.clone())
                    .await
                    .context(format!("failed to discover protocol `{}`", identifier))?,
            );
        }

        Ok(discovered)
    }

    pub async fn discover_and_store(&self, to: BlockNumber, storage: &Storage) -> anyhow::Result<()>
    where
        P: Clone,
    {
        let last_discovered_blocks = storage
            .get_last_discovered_blocks(&self.protocol_identifiers())
            .await?;

        let discovered = self.discover(&last_discovered_blocks, to).await?;

        storage.insert_pools(&discovered).await?;

        for (protocol, _) in discovered {
            storage.set_last_discovered_block(to, &protocol).await?;
        }

        Ok(())
    }

    async fn create_pools_from_records(
        &self,
        records: Vec<IdentifiedLiquidityPool>,
        block: BlockId,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: Clone + 'static,
    {
        let mut pools_by_protocol: HashMap<String, Vec<Address>> = HashMap::new();

        let now = Instant::now();
        tracing::info!("creating objects");

        for record in records {
            pools_by_protocol
                .entry(record.protocol)
                .or_default()
                .push(record.address);
        }

        let mut pools = vec![];

        for (protocol, addresses) in pools_by_protocol {
            let protocol: Arc<dyn DiscoverableProtocol<P>> =
                self.protocols[&protocol].clone().into();

            pools.extend(
                self.create_protocol_pools(protocol, addresses, block)
                    .await?,
            );
        }

        tracing::info!("objects created in {:?}", now.elapsed());

        Ok(pools)
    }

    pub async fn get_stored_pools(
        &self,
        storage: &Storage,
        block: BlockId,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: std::fmt::Debug + 'static + Clone,
    {
        self.create_pools_from_records(storage.get_pools().await?, block)
            .await
    }

    async fn create_protocol_pools(
        &self,
        protocol: Arc<dyn DiscoverableProtocol<P>>,
        addresses: Vec<Address>,
        block: BlockId,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: Clone + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(POOL_CREATION_TASK_LIMIT));

        let tasks: Vec<_> = addresses
            .into_iter()
            .map(|address| {
                let protocol = protocol.clone();
                let provider = self.provider.clone();
                let semaphore = semaphore.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire_owned().await.unwrap();

                    protocol
                        .create_pool_with_provider(address, provider, block)
                        .await
                })
            })
            .collect();

        Ok(future::try_join_all(tasks)
            .await?
            .into_iter()
            .filter_map(Result::ok)
            .collect())
    }

    pub async fn get_filtered_pools(
        &self,
        storage: &Storage,
        usd_value: f64,
        block: BlockNumber,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: std::fmt::Debug + 'static + Clone,
    {
        let pools = self.get_stored_pools(storage, block.into()).await?;

        let filter = PoolFilter::new(
            usd_value,
            self.provider.clone(),
            block,
            self.protocols.values().cloned().collect(),
        )
        .await?;

        let now = Instant::now();
        tracing::info!("filtering pools");

        let pools = filter
            .filter_pools(
                pools,
                self.provider.clone(),
                self.protocols.values().cloned().collect(),
                block,
            )
            .await?;

        tracing::info!("filtered in {:?}", now.elapsed());

        Ok(pools)
    }

    pub async fn cache_filtered_pools(
        &self,
        storage: &Storage,
        pools: &[Box<dyn LiquidityPool<P>>],
    ) -> anyhow::Result<()> {
        let identified_pools: Vec<_> = pools
            .iter()
            .map(|pool| IdentifiedLiquidityPool {
                address: pool.address(),
                protocol: pool.identifier().to_string(),
            })
            .collect();

        storage.insert_filtered_pools(&identified_pools, 0).await?;

        Ok(())
    }

    pub async fn get_cached_filtered_pools(
        &self,
        storage: &Storage,
        block: BlockId,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: Clone + 'static,
    {
        let pool_records = storage
            .get_filtered_pools(0)
            .await?
            .expect("pools are cached");

        self.create_pools_from_records(pool_records, block).await
    }

    pub fn protocol_identifiers(&self) -> Vec<String> {
        self.protocols.keys().cloned().collect()
    }

    fn get_pools(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, EvmCallError<P>> {
        let mut pools = vec![];

        for protocol in self.protocols.values() {
            pools.extend(protocol.get_pools(token0, token1, evm)?);
        }

        Ok(pools)
    }

    pub fn get_token_value(
        &self,
        of_token: Address,
        in_token: Address,
        amount: U256,
        evm: &mut EVM<P>,
    ) -> Result<U256, EvmCallError<P>> {
        let pools = self.get_pools(of_token, in_token, evm)?;

        let values: Vec<_> = pools
            .into_iter()
            .map(|mut pool| pool.simulate_swap(of_token, amount, evm))
            .collect();

        Ok(values.into_iter().max().unwrap_or(U256::from(0)))
    }
}
