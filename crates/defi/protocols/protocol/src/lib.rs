use alloy::{primitives::Address, providers::Provider, transports::Transport};
use pool::LiquidityPool;

pub mod pool;

pub trait Protocol {
    type Pool: LiquidityPool;

    fn discover<T: Transport + Clone, P: Provider<T>>(
        &self,
        from: u64,
        to: u64,
        provider: P,
    ) -> Vec<Self::Pool>;

    fn get_pools(&self, token0: Address, token1: Address) -> Vec<Self::Pool>;
}
