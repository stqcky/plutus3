use std::sync::Arc;

use alloy::{
    primitives::{Address, BlockNumber, U256},
    providers::Provider,
};
use async_trait::async_trait;
use dyn_clone::DynClone;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;
use plutus_evm::{EVM, errors::EvmCallError};

#[async_trait]
pub trait LiquidityPool<P: Provider>: DynClone + Send + Sync {
    fn identifier(&self) -> &'static str;
    fn address(&self) -> Address;

    fn simulate_swap(&mut self, token: Address, amount: U256, evm: &mut EVM<P>) -> U256;
    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);

    fn is_liquidity_valid(&self) -> bool;

    fn token_addresses(&self) -> (Address, Address);
    fn tokens(&self) -> (ERC20, ERC20);
    fn tokens_locked(&self, evm: &mut EVM<P>) -> Result<(U256, U256), EvmCallError<P>>;

    async fn verify_health(&self, provider: Arc<P>, block: BlockNumber) -> anyhow::Result<bool>;
}

dyn_clone::clone_trait_object!(<P: Provider> LiquidityPool<P>);
