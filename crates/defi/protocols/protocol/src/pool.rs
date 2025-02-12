use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use dyn_clone::DynClone;
use hashbrown::HashMap;
use plutus_defi_erc20::ERC20;
use plutus_evm::{EVM, errors::EvmCallError};

pub trait LiquidityPool<P: Provider>: DynClone {
    fn identifier(&self) -> &'static str;
    fn address(&self) -> Address;

    fn simulate_swap(&mut self, token: Address, amount: U256, evm: &mut EVM<P>) -> U256;
    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);

    fn is_liquidity_valid(&self) -> bool;

    fn token_addresses(&self) -> (Address, Address);
    fn tokens(&self) -> (ERC20, ERC20);
    fn tokens_locked(&self, evm: &mut EVM<P>) -> Result<(U256, U256), EvmCallError<P>>;
}

dyn_clone::clone_trait_object!(<P: Provider> LiquidityPool<P>);
