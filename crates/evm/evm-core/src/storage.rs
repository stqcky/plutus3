use alloy::{
    primitives::{Signed, Uint},
    providers::Provider,
    uint,
};
use revm::primitives::{Address, I256, U256, map::B256Map};

use crate::EVM;

pub struct SmartContractStorage {
    address: Address,
    storage: B256Map<U256>,
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
