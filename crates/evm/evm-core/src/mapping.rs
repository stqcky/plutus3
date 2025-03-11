use std::hash::Hash;

use alloy::{
    primitives::{Keccak256, Signed, Uint},
    providers::Provider,
};
use hashbrown::HashMap;
use parking_lot::RwLock;
use revm::primitives::{B256, I256, U256, map::B256Map};
use revm_database::BlockId;

use crate::storage::SmartContractStorage;

#[derive(Debug)]
pub struct SolidityMapping<K, V, const SLOT: u128, const VALUE_SLOT_SIZE: usize = 1> {
    mapping: RwLock<HashMap<K, V>>,
    discovered_slots: RwLock<B256Map<K>>,
    slot: [u8; U256::BYTES],
}

impl<K, V, const SLOT: u128, const VALUE_SLOT_SIZE: usize>
    SolidityMapping<K, V, SLOT, VALUE_SLOT_SIZE>
where
    K: Eq + Hash + Copy + IntoU256,
    V: StorageDecodable + Copy,
{
    pub fn new() -> Self {
        Self {
            slot: U256::from(SLOT).to_be_bytes::<{ U256::BYTES }>(),
            mapping: RwLock::new(HashMap::default()),
            discovered_slots: RwLock::new(B256Map::default()),
        }
    }

    pub async fn get<P: Provider>(
        &self,
        storage: &SmartContractStorage,
        k: &K,
        block: BlockId,
        provider: P,
    ) -> Result<V, alloy::contract::Error> {
        {
            let mapping = self.mapping.read();

            if let Some(value) = mapping.get(k) {
                return Ok(*value);
            }
        };

        let slot = self.get_value_storage_slot(k);

        self.discover_slot(*k, slot);

        let value = V::decode(
            self.fetch_value_from_storage(slot, storage, block, provider)
                .await?,
        );

        self.mapping.write().insert(*k, value);

        Ok(value)
    }

    pub async fn fetch_many<P: Provider>(
        &self,
        storage: &SmartContractStorage,
        keys: &[K],
        block: BlockId,
        provider: P,
    ) -> Result<(), alloy::contract::Error> {
        let base_slots = keys
            .into_iter()
            .map(|key| self.get_value_storage_slot(key))
            .collect::<Vec<_>>();

        for (&k, &slot) in keys.iter().zip(base_slots.iter()) {
            self.discover_slot(k, slot);
        }

        let slots = base_slots
            .into_iter()
            .flat_map(|base_slot| (0..VALUE_SLOT_SIZE).map(move |i| base_slot + U256::from(i)))
            .collect::<Vec<_>>();

        let values = storage.get_many(&slots, block, provider).await?;

        let mut mapping = self.mapping.write();

        for (&k, values) in keys.into_iter().zip(values.chunks_exact(VALUE_SLOT_SIZE)) {
            let value = V::decode(
                values
                    .into_iter()
                    .flat_map(|value| value.to_le_bytes::<{ U256::BYTES }>())
                    .collect(),
            );

            mapping.insert(k, value);
        }

        Ok(())
    }

    pub fn insert(&self, k: K, v: V) {
        let slot = self.get_value_storage_slot(&k);
        self.discover_slot(k, slot);

        self.mapping.write().insert(k, v);
    }

    pub fn invalidate_many(&self, slots: &[U256]) {
        let mut mapping = self.mapping.write();

        for slot in slots {
            if let Some(key) = self.discovered_slots.read().get(&B256::from(*slot)) {
                mapping.remove(key);
            }
        }
    }

    pub fn invalidate(&self, slot: U256) {
        if let Some(key) = self.discovered_slots.read().get(&B256::from(slot)) {
            self.mapping.write().remove(key);
        }
    }

    fn discover_slot(&self, key: K, slot: U256) {
        let mut discovered_slots = self.discovered_slots.write();

        for i in 0..VALUE_SLOT_SIZE {
            discovered_slots.insert(B256::from(slot + U256::from(i)), key);
        }
    }

    async fn fetch_value_from_storage<P: Provider>(
        &self,
        slot: U256,
        storage: &SmartContractStorage,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<u8>, alloy::contract::Error> {
        let (v, cached) = storage
            .get_consecutive_with_cache_state(slot.into(), VALUE_SLOT_SIZE, block, provider)
            .await?;

        if !cached {
            // tracing::warn!("CACHE MISS {VALUE_SLOT_SIZE}");
        }

        Ok(v.into_iter()
            .flat_map(|value| value.to_le_bytes::<{ U256::BYTES }>())
            .collect())
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
