pub mod factory;
pub mod pool;
pub mod router;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use factory::{FACTORY_ADDRESS, PancakeSwapV2Factory};
use plutus_defi_protocols_protocol::{
    DiscoverableProtocol, Protocol, ProtocolFactory, pool::LiquidityPool,
};
use plutus_evm::{EVM, errors::EvmCallError};
use pool::PancakeSwapV2Pool;

#[derive(Clone, Copy)]
pub struct PancakeSwapV2Protocol {
    factory: PancakeSwapV2Factory,
}

#[async_trait]
impl<P: Provider + 'static> Protocol<P> for PancakeSwapV2Protocol {
    fn get_pools(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, EvmCallError<P>> {
        let pool = self.factory.get_pool(token0, token1, evm)?;

        if let Some(pool) = pool {
            Ok(vec![Box::new(pool)])
        } else {
            Ok(vec![])
        }
    }

    async fn get_pools_with_provider(
        &self,
        token0: Address,
        token1: Address,
        provider: P,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, alloy::contract::Error> {
        let pool = self
            .factory
            .get_pool_with_provider(token0, token1, &provider)
            .await?;

        if let Some(pool) = pool {
            Ok(vec![Box::new(pool)])
        } else {
            Ok(vec![])
        }
    }

    fn create_pool(
        &self,
        address: Address,
        evm: &mut EVM<P>,
    ) -> Result<Box<dyn LiquidityPool<P>>, EvmCallError<P>> {
        Ok(Box::new(PancakeSwapV2Pool::new(address, evm)?))
    }

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Box<dyn LiquidityPool<P>>, alloy::contract::Error> {
        Ok(Box::new(
            PancakeSwapV2Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

#[async_trait]
impl<P: Provider + 'static> DiscoverableProtocol<P> for PancakeSwapV2Protocol {
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

impl<P: Provider + 'static> ProtocolFactory<P> for PancakeSwapV2Protocol {
    const IDENTIFIER: &str = "pancakeswap_v2";

    fn new(chain_id: ChainId) -> Option<Self> {
        let factory_address = *FACTORY_ADDRESS.get(&chain_id)?;

        Some(Self {
            factory: PancakeSwapV2Factory::new(factory_address),
        })
    }
}
