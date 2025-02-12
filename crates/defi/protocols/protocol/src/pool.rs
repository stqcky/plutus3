use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use hashbrown::HashMap;
use plutus_evm::{EVM, errors::EvmCallError};

pub trait LiquidityPool<P: Provider> {
    fn identifier(&self) -> &'static str;
    fn address(&self) -> Address;

    fn simulate_swap(&mut self, token: Address, amount: U256, evm: &mut EVM<P>) -> U256;
    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);

    fn is_liquidity_valid(&self) -> bool;

    fn tokens(&self) -> (Address, Address);
    fn tokens_locked(&self, evm: &mut EVM<P>) -> Result<(U256, U256), EvmCallError<P>>;
}
