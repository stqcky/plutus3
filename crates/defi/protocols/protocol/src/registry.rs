use alloy::{
    primitives::{Address, BlockNumber, ChainId, U256, address},
    providers::Provider,
};
use anyhow::Context;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;
use plutus_evm::{EVM, errors::EvmCallError};
use plutus_storage::{IdentifiedLiquidityPool, Storage};

use crate::{DiscoverableProtocol, ProtocolFactory, pool::LiquidityPool};

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

    pub async fn get_stored_pools(
        &self,
        storage: &Storage,
        evm: &mut EVM<P>,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: std::fmt::Debug + 'static,
    {
        let identified_pool_records = storage.get_pools().await?;

        let mut pools = vec![];

        for pool_record in identified_pool_records {
            let protocol = &self.protocols[&pool_record.protocol];

            let Ok(pool) = protocol.create_pool(pool_record.address, evm) else {
                continue;
            };

            pools.push(pool);
        }

        Ok(pools)
    }

    pub async fn get_filtered_pools(
        &self,
        storage: &Storage,
        evm: &mut EVM<P>,
        usd_value: f64,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: std::fmt::Debug + 'static,
    {
        let pools = self.get_stored_pools(storage, evm).await?;

        let pools: Vec<_> = pools
            .into_iter()
            .filter(|pool| pool.is_liquidity_valid())
            .collect();

        let weth_value = usd_value / self.get_weth_usd_value(evm)?;

        let pools: Vec<_> = pools
            .into_iter()
            .filter(|pool| {
                if let Ok(true) =
                    self.pool_has_adequate_tvl(pool.as_ref(), usd_value, weth_value, evm)
                {
                    true
                } else {
                    false
                }
            })
            .collect();

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
        evm: &mut EVM<P>,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>> {
        let pool_records = storage
            .get_filtered_pools(0)
            .await?
            .expect("pools are cached");

        let mut pools = vec![];

        for pool in pool_records {
            let protocol = &self.protocols[&pool.protocol];

            let Ok(pool) = protocol.create_pool(pool.address, evm) else {
                continue;
            };

            pools.push(pool);
        }

        Ok(pools)
    }

    pub fn protocol_identifiers(&self) -> Vec<String> {
        self.protocols.keys().cloned().collect()
    }

    fn get_weth_usd_value(&self, evm: &mut EVM<P>) -> Result<f64, EvmCallError<P>> {
        let mut usdt_weth_pool = self.protocols["uniswap_v3"]
            .create_pool(address!("0x42161084d0672e1d3F26a9B53E653bE2084ff19C"), evm)?;

        let usdt = ERC20::new(address!("fd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"), evm)?;
        let weth = ERC20::new(address!("82af49447d8a07e3bd95bd0d56f35241523fbab1"), evm)?;

        let value = usdt_weth_pool.simulate_swap(weth.address, weth.to_token_amount(1.0), evm);

        Ok(usdt.to_float_amount(value))
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

    fn pool_has_adequate_tvl(
        &self,
        pool: &dyn LiquidityPool<P>,
        required_usd_value: f64,
        required_weth_value: f64,
        evm: &mut EVM<P>,
    ) -> Result<bool, EvmCallError<P>> {
        let (token0, token1) = pool.token_addresses();
        let (locked0, locked1) = pool.tokens_locked(evm)?;

        let usdt = ERC20::new(address!("fd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"), evm)?;
        let usdc = ERC20::new(address!("af88d065e77c8cc2239327c5edb3a432268e5831"), evm)?;
        let weth = ERC20::new(address!("82af49447d8a07e3bd95bd0d56f35241523fbab1"), evm)?;

        for usd_token in [usdt, usdc] {
            let usd_value0 = self.get_token_value(token0, usd_token.address, locked0, evm)?;
            let usd_value1 = self.get_token_value(token1, usd_token.address, locked1, evm)?;

            let usd_value = usd_token.to_float_amount(usd_value0 + usd_value1);

            if usd_value >= required_usd_value {
                return Ok(true);
            }
        }

        let weth_value0 = self.get_token_value(token0, weth.address, locked0, evm)?;
        let weth_value1 = self.get_token_value(token1, weth.address, locked1, evm)?;

        let weth_value = weth.to_float_amount(weth_value0 + weth_value1);

        if weth_value >= required_weth_value {
            return Ok(true);
        }

        Ok(false)
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
