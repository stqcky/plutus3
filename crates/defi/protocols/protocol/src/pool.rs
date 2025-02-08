use alloy::primitives::{Address, U256};

pub trait LiquidityPool {
    fn simulate_swap(&self, token: Address, amount: U256) -> U256;
}
