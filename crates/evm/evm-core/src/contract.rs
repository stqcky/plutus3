use alloy::providers::Provider;
use hashbrown::HashMap;
use revm::primitives::{Address, U256};

use crate::{EVM, errors::EvmCallError};

pub trait SmartContract {
    fn new<P: Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, EvmCallError<P>>
    where
        Self: Sized;

    fn apply_storage_changes(&mut self, changes: HashMap<U256, U256>);
}
