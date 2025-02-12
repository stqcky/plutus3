use std::marker::PhantomData;

use alloy::{
    primitives::{Keccak256, Signed, Uint},
    providers::Provider,
};
use revm::primitives::{I256, U256};

use crate::{EVM, storage::SmartContractStorage};

#[derive(Debug, Clone, Copy)]
pub struct SolidityMapping<K, V, const SLOT: u128, const VALUE_SLOT_SIZE: usize = 1> {
    slot: [u8; U256::BYTES],
    _marker: PhantomData<(K, V)>,
}

impl<K, V, const SLOT: u128, const VALUE_SLOTS: usize> SolidityMapping<K, V, SLOT, VALUE_SLOTS>
where
    K: Copy + IntoU256,
    V: StorageDecodable,
{
    pub fn new() -> Self {
        Self {
            slot: U256::from(SLOT).to_be_bytes::<{ U256::BYTES }>(),
            _marker: PhantomData::default(),
        }
    }

    pub fn get<P: Provider>(
        &mut self,
        storage: &mut SmartContractStorage,
        k: &K,
        evm: &mut EVM<P>,
    ) -> V {
        let slot = self.get_value_storage_slot(k);

        let bytes: Vec<_> = storage
            .get_consecutive(slot, VALUE_SLOTS, evm)
            .into_iter()
            .flat_map(|value| value.to_le_bytes::<{ U256::BYTES }>())
            .collect();

        V::decode(bytes)
    }

    fn get_value_storage_slot(&self, k: &K) -> U256 {
        let mut hasher = Keccak256::new();

        hasher.update(k.into_u256().to_be_bytes::<{ U256::BYTES }>());
        hasher.update(self.slot);

        hasher.finalize().into()
    }
}

pub trait StorageDecodable {
    fn decode(bytes: Vec<u8>) -> Self;
}

impl StorageDecodable for U256 {
    fn decode(bytes: Vec<u8>) -> Self {
        Self::from_le_slice(&bytes)
    }
}

pub trait IntoU256 {
    fn into_u256(self) -> U256;
}

impl<const BITS: usize, const LIMBS: usize> IntoU256 for Uint<BITS, LIMBS> {
    fn into_u256(self) -> U256 {
        U256::from(self)
    }
}

impl<const BITS: usize, const LIMBS: usize> IntoU256 for Signed<BITS, LIMBS> {
    fn into_u256(self) -> U256 {
        let (sign, abs) = self.into_sign_and_abs();

        I256::checked_from_sign_and_abs(sign, U256::from(abs))
            .expect("i swear to god if this shit panics")
            .into_raw()
    }
}

impl IntoU256 for i16 {
    fn into_u256(self) -> U256 {
        I256::unchecked_from(self).into_u256()
    }
}
