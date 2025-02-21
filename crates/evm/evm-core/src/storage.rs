use alloy::providers::Provider;
use hashbrown::hash_map::Entry;
use revm::primitives::{Address, B256, U256, map::B256Map};
use revm_database::BlockId;

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

    pub async fn get<P: Provider>(
        &mut self,
        slot: U256,
        block: BlockId,
        provider: P,
    ) -> Result<U256, alloy::contract::Error> {
        let entry = self.storage.entry(slot.into());
        match entry {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(vacant) => {
                let value = provider
                    .get_storage_at(self.address, slot)
                    .block_id(block)
                    .await?;
                Ok(*vacant.insert(value))
            }
        }
    }

    pub async fn get_consecutive<P: Provider>(
        &mut self,
        slot: U256,
        amount: usize,
        block: BlockId,
        provider: P,
    ) -> Result<Vec<U256>, alloy::contract::Error> {
        let mut values = Vec::with_capacity(amount);

        for i in 0..amount {
            values.push(self.get(slot + U256::from(i), block, &provider).await?);
        }

        Ok(values)
    }

    pub fn get_consecutive_cached(&self, slot: U256, amount: usize) -> Option<Vec<U256>> {
        let mut values = Vec::with_capacity(amount);

        for i in 0..amount {
            let key: B256 = (slot + U256::from(i)).into();
            values.push(*self.storage.get(&key)?);
        }

        Some(values)
    }

    pub fn insert(&mut self, slot: U256, value: U256) {
        self.storage.insert(slot.into(), value);
    }

    pub fn clear(&mut self) {
        self.storage.clear();
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
