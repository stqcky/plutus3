use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, UniswapV2Factory};
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};
use pool::UniswapV2Pool;

pub mod factory;
pub mod pool;
pub mod router;

#[derive(Clone, Copy)]
pub struct UniswapV2Protocol {
    factory: UniswapV2Factory,
}

#[async_trait]
impl<P: Provider + 'static> Protocol<P> for UniswapV2Protocol {
    async fn get_pools_with_provider(
        &self,
        token0: Address,
        token1: Address,
        provider: P,
    ) -> Result<Vec<Arc<dyn LiquidityPool<P>>>, alloy::contract::Error> {
        let pool = self
            .factory
            .get_pool_with_provider(token0, token1, &provider)
            .await?;

        if let Some(pool) = pool {
            Ok(vec![Arc::new(pool)])
        } else {
            Ok(vec![])
        }
    }

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Arc<dyn LiquidityPool<P>>, alloy::contract::Error> {
        Ok(Arc::new(
            UniswapV2Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

#[async_trait]
impl<P: Provider + 'static> DiscoverableProtocol<P> for UniswapV2Protocol {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error> {
        Ok(self
            .factory
            .pair_created_events(from, to, provider)
            .await?
            .into_iter()
            .map(|event| event.pair)
            .collect())
    }
}

impl<P: Provider + 'static> ProtocolFactory<P> for UniswapV2Protocol {
    const IDENTIFIER: &str = "uniswap_v2";

    fn new(chain_id: ChainId) -> Option<Self> {
        let factory_address = *FACTORY_ADDRESS.get(&chain_id)?;

        Some(Self {
            factory: UniswapV2Factory::new(factory_address),
        })
    }
}
