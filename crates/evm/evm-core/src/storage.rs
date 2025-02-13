use alloy::providers::Provider;
use revm::primitives::{Address, U256, map::B256Map};

use crate::EVM;

#[derive(Debug, Clone)]
pub struct SmartContractStorage {
    address: Address,
    pub storage: B256Map<U256>,
}

impl SmartContractStorage {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            storage: B256Map::default(),
        }
    }

    pub fn get<P: Provider>(&mut self, slot: U256, evm: &mut EVM<P>) -> U256 {
        *self
            .storage
            .entry(slot.into())
            .or_insert_with(|| evm.storage(self.address, slot))
    }

    pub fn get_consecutive<P: Provider>(
        &mut self,
        slot: U256,
        amount: usize,
        evm: &mut EVM<P>,
    ) -> Vec<U256> {
        let mut values = Vec::with_capacity(amount);

        for i in 0..amount {
            values.push(self.get(slot + U256::from(i), evm))
        }

        values
    }

    pub fn insert(&mut self, slot: U256, value: U256) {
        self.storage.insert(slot.into(), value);
    }
}

pub trait FromStorageValue {
    fn from_storage_value(value: U256) -> Self;
}

// impl<const BITS: usize, const LIMBS: usize> FromStorageValue for Uint<BITS, LIMBS> {
//     fn from_storage_value(value: U256) -> Self {
//         Self::from(value)
//     }
// }

// impl<const BITS: usize, const LIMBS: usize> FromStorageValue for Signed<BITS, LIMBS> {
//     fn from_storage_value(value: U256) -> Self {
//         Self::from_raw(Uint::<BITS, LIMBS>::from(value))
//     }
// }

impl<T> FromStorageValue for T
where
    T: TryFrom<U256>,
    <T as TryFrom<U256>>::Error: std::fmt::Debug,
{
    fn from_storage_value(value: U256) -> Self {
        Self::try_from(value).unwrap()
    }
}
