use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use hashbrown::HashMap;
use plutus_evm::EVM;

pub trait LiquidityPool<P: Provider> {
    fn simulate_swap(&mut self, token: Address, amount: U256, evm: &mut EVM<P>) -> U256;
    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);
}
