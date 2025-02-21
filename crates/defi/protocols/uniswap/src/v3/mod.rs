pub mod factory;
pub mod fee;
pub mod pool;
pub mod quoter;
pub mod tick_bitmap;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, UniswapV3Factory};
use fee::FeeAmount;
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};
use plutus_evm::{EVM, errors::EvmCallError};
use pool::UniswapV3Pool;
use strum::IntoEnumIterator as _;

#[derive(Clone, Copy)]
pub struct UniswapV3Protocol {
    factory: UniswapV3Factory,
}

#[async_trait]
impl<P: Provider + 'static> Protocol<P> for UniswapV3Protocol {
    async fn get_pools_with_provider(
        &self,
        token0: Address,
        token1: Address,
        provider: P,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, alloy::contract::Error> {
        let mut pools: Vec<Box<dyn LiquidityPool<P>>> = vec![];

        for fee in FeeAmount::iter() {
            let Some(pool) = self
                .factory
                .get_pool_with_provider(token0, token1, fee, &provider)
                .await?
            else {
                continue;
            };

            pools.push(Box::new(pool));
        }

        Ok(pools)
    }

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Box<dyn LiquidityPool<P>>, alloy::contract::Error> {
        Ok(Box::new(
            UniswapV3Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

#[async_trait]
impl<P: Provider + 'static> DiscoverableProtocol<P> for UniswapV3Protocol {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error> {
        Ok(self
            .factory
            .pool_created_events(from, to, provider)
            .await?
            .into_iter()
            .map(|event| event.pool)
            .collect())
    }
}

impl<P: Provider + 'static> ProtocolFactory<P> for UniswapV3Protocol {
    const IDENTIFIER: &str = "uniswap_v3";

    fn new(chain_id: ChainId) -> Option<Self> {
        let factory_address = *FACTORY_ADDRESS.get(&chain_id)?;

        Some(Self {
            factory: UniswapV3Factory::new(factory_address),
        })
    }
}
