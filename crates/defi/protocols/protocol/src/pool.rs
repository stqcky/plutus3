use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U256},
    providers::Provider,
};
use async_trait::async_trait;
use dyn_clone::DynClone;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;

#[async_trait]
pub trait LiquidityPool<P: Provider>: DynClone + Send + Sync {
    fn identifier(&self) -> &'static str;
    fn address(&self) -> Address;

    async fn simulate_swap(
        &mut self,
        token: Address,
        amount: U256,
        block: BlockId,
        provider: P,
    ) -> U256;

    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);
    async fn update_with_provider(
        &mut self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error>;

    fn is_liquidity_valid(&self) -> bool;

    fn token0(&self) -> &ERC20;
    fn token1(&self) -> &ERC20;

    fn tokens(&self) -> (&ERC20, &ERC20) {
        (self.token0(), self.token1())
    }

    async fn tokens_locked(&self, provider: P) -> Result<(U256, U256), alloy::contract::Error>;

    async fn verify_health(&self, provider: Arc<P>, block: BlockNumber) -> anyhow::Result<bool>;

    fn create_payload(
        &self,
        recipient: Address,
        token_in: Address,
        amount: U256,
        extra: Vec<u8>,
    ) -> Vec<u8>;
}

dyn_clone::clone_trait_object!(<P: Provider> LiquidityPool<P>);
