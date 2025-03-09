use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
    sol,
};
use async_trait::async_trait;
use dyn_clone::DynClone;
use pool::LiquidityPool;

pub mod filtering;
pub mod pool;
pub mod registry;

pub type SwapDataPayload = sol! { tuple(address, address, uint256, bytes) };

#[async_trait]
pub trait Protocol<P: Provider + Clone + 'static>: Send + Sync + DynClone {
    async fn get_pool_addresses_with_provider(
        &self,
        token0: Address,
        token1: Address,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error>;

    async fn get_pools_with_provider(
        &self,
        token0: Address,
        token1: Address,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<Arc<dyn LiquidityPool<P>>>, alloy::contract::Error> {
        let pool_addresses = self
            .get_pool_addresses_with_provider(token0, token1, block, provider.clone())
            .await?;

        let pools = futures::future::join_all(
            pool_addresses
                .into_iter()
                .map(|address| self.create_pool_with_provider(address, provider.clone(), block))
                .collect::<Vec<_>>(),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        Ok(pools)
    }

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Arc<dyn LiquidityPool<P>>, alloy::contract::Error>;
}

dyn_clone::clone_trait_object!(<P: Provider> Protocol<P>);

#[async_trait]
pub trait DiscoverableProtocol<P: Provider + Clone + 'static>: Protocol<P> + DynClone {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error>;
}

dyn_clone::clone_trait_object!(<P: Provider> DiscoverableProtocol<P>);

pub trait ProtocolFactory<P: Provider + Clone + 'static>: DiscoverableProtocol<P> + Sized {
    const IDENTIFIER: &'static str;

    fn new(chain_id: ChainId) -> Option<Self>;
}
