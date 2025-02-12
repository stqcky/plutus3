use alloy::{
    primitives::{Address, BlockNumber, ChainId},
    providers::Provider,
};
use async_trait::async_trait;
use plutus_evm::{EVM, errors::EvmCallError};
use pool::LiquidityPool;

pub mod pool;
pub mod registry;

pub trait Protocol<P: Provider> {
    fn get_pools(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Vec<Box<dyn LiquidityPool<P>>>, EvmCallError<P>>;

    fn create_pool(
        &self,
        address: Address,
        evm: &mut EVM<P>,
    ) -> Result<Box<dyn LiquidityPool<P>>, EvmCallError<P>>;
}

#[async_trait]
pub trait DiscoverableProtocol<P: Provider>: Protocol<P> {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error>;
}

pub trait ProtocolFactory<P: Provider>: DiscoverableProtocol<P> + Sized {
    const IDENTIFIER: &'static str;

    fn new(chain_id: ChainId) -> Option<Self>;
}
