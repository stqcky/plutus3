mod factory;
pub mod fee;
pub mod pool;
mod tick_bitmap;

use alloy::{
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

pub struct UniswapV3Protocol {
    factory: UniswapV3Factory,
}

impl<P: Provider + 'static> Protocol<P> for UniswapV3Protocol {
    fn get_pools(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, EvmCallError<P>> {
        let mut pools = vec![];

        for fee in FeeAmount::iter() {
            let Some(pool) = self.factory.get_pool(token0, token1, fee, evm)? else {
                continue;
            };

            pools.push(Box::new(pool) as Box<dyn LiquidityPool<P>>);
        }

        Ok(pools)
    }

    fn create_pool(
        &self,
        address: Address,
        evm: &mut EVM<P>,
    ) -> Result<Box<dyn LiquidityPool<P>>, EvmCallError<P>> {
        Ok(Box::new(UniswapV3Pool::new(address, evm)?) as Box<dyn LiquidityPool<P>>)
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
