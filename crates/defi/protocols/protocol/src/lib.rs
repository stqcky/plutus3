use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use dyn_clone::DynClone;
use plutus_evm::{EVM, errors::EvmCallError};
use pool::LiquidityPool;

pub mod filtering;
pub mod pool;
pub mod registry;

#[async_trait]
pub trait Protocol<P: Provider>: Send + Sync + DynClone {
    async fn get_pools_with_provider(
        &self,
        token0: Address,
        token1: Address,
        provider: P,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, alloy::contract::Error>;

    async fn create_pool_with_provider(
        &self,
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Box<dyn LiquidityPool<P>>, alloy::contract::Error>;
}

dyn_clone::clone_trait_object!(<P: Provider> Protocol<P>);

#[async_trait]
pub trait DiscoverableProtocol<P: Provider>: Protocol<P> + DynClone {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error>;
}

dyn_clone::clone_trait_object!(<P: Provider> DiscoverableProtocol<P>);

pub trait ProtocolFactory<P: Provider>: DiscoverableProtocol<P> + Sized {
    const IDENTIFIER: &'static str;

    fn new(chain_id: ChainId) -> Option<Self>;
}
