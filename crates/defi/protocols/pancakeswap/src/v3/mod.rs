pub mod factory;
pub mod pool;
pub mod quoter;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, PancakeSwapV3Factory};
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};
use pool::PancakeSwapV3Pool;

#[derive(Clone, Copy)]
pub struct PancakeSwapV3Protocol {
    factory: PancakeSwapV3Factory,
}

#[async_trait]
impl<P: Provider + Clone + 'static> Protocol<P> for PancakeSwapV3Protocol {
    async fn get_pool_addresses_with_provider(
        &self,
        token0: Address,
        token1: Address,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error> {
        self.factory
            .get_pool_addresses_with_provider(token0, token1, block, provider)
            .await
    }

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Box<dyn LiquidityPool<P>>, alloy::contract::Error> {
        Ok(Box::new(
            PancakeSwapV3Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

#[async_trait]
impl<P: Provider + Clone + 'static> DiscoverableProtocol<P> for PancakeSwapV3Protocol {
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

impl<P: Provider + Clone + 'static> ProtocolFactory<P> for PancakeSwapV3Protocol {
    const IDENTIFIER: &str = "pancakeswap_v3";

    fn new(chain_id: ChainId) -> Option<Self> {
        let factory_address = *FACTORY_ADDRESS.get(&chain_id)?;

        Some(Self {
            factory: PancakeSwapV3Factory::new(factory_address),
        })
    }
}
