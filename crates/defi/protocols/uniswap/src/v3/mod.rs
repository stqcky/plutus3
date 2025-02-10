mod factory;

use alloy::{
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, UniswapV3Factory};
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};

pub struct UniswapV3Protocol {
    factory: UniswapV3Factory,
}

impl Protocol for UniswapV3Protocol {
    fn get_pools(&self, token0: Address, token1: Address) -> Vec<Box<dyn LiquidityPool>> {
        todo!()
    }

    fn create_pool(&self, address: Address) -> Box<dyn LiquidityPool> {
        todo!()
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
