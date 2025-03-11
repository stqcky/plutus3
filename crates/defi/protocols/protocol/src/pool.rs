use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U256},
    providers::Provider,
};
use async_trait::async_trait;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;

#[async_trait]
pub trait LiquidityPool<P: Provider>: Send + Sync {
    fn identifier(&self) -> &'static str;
    fn address(&self) -> Address;

    fn simulate_swap(&self, token: Address, amount: U256, block: BlockId) -> U256;

    fn apply_storage_changes(&self, changes: HashMap<U256, U256>);
    async fn update_with_provider(
        &self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error>;

    fn is_liquidity_valid(&self) -> bool;

    fn token0(&self) -> &ERC20;
    fn token1(&self) -> &ERC20;

    fn tokens(&self) -> (&ERC20, &ERC20) {
        (self.token0(), self.token1())
    }

    fn tokens_locked(&self) -> (U256, U256);

    async fn verify_health(&self, provider: Arc<P>, block: BlockNumber) -> anyhow::Result<bool>;

    fn create_payload(
        &self,
        recipient: Address,
        token_in: Address,
        amount: U256,
        extra: Vec<u8>,
    ) -> Vec<u8>;
}
