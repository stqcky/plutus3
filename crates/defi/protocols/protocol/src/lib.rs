use alloy::primitives::{Address, BlockNumber, ChainId};
use async_trait::async_trait;
use pool::LiquidityPool;

pub mod pool;
pub mod registry;

pub trait Protocol {
    fn get_pools(&self, token0: Address, token1: Address) -> Vec<Box<dyn LiquidityPool>>;
    fn create_pool(&self, address: Address) -> Box<dyn LiquidityPool>;
}

#[async_trait]
pub trait DiscoverableProtocol<P>: Protocol {
    async fn discover(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<Address>, alloy::contract::Error>;
}

pub trait ProtocolFactory<P>: DiscoverableProtocol<P> + Sized {
    const IDENTIFIER: &'static str;

    fn new(chain_id: ChainId) -> Option<Self>;
}
