use alloy::{
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, UniswapV2Factory};
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};
use plutus_evm::{EVM, errors::EvmCallError};
use pool::UniswapV2Pool;

pub mod factory;
pub mod pool;

pub struct UniswapV2Protocol {
    factory: UniswapV2Factory,
}

impl<P: Provider> Protocol<P> for UniswapV2Protocol {
    fn get_pools(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, EvmCallError<P>> {
        let address = self.factory.get_pool_address(token0, token1, evm)?;

        if let Some(address) = address {
            Ok(vec![self.create_pool(address, evm)?])
        } else {
            Ok(vec![])
        }
    }

    fn create_pool(
        &self,
        address: Address,
        evm: &mut EVM<P>,
    ) -> Result<Box<dyn LiquidityPool<P>>, EvmCallError<P>> {
        Ok(Box::new(UniswapV2Pool::new(address, evm)?) as Box<dyn LiquidityPool<P>>)
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
