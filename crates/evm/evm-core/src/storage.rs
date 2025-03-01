use alloy::providers::Provider;
use parking_lot::RwLock;
use revm::primitives::{Address, B256, U256, map::B256Map};
use revm_database::BlockId;

#[derive(Debug)]
pub struct SmartContractStorage {
    address: Address,
    pub storage: RwLock<B256Map<U256>>,
}

impl SmartContractStorage {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            storage: RwLock::new(B256Map::default()),
        }
    }

    pub async fn get<P: Provider>(
        &self,
        slot: U256,
        block: BlockId,
        provider: P,
    ) -> Result<U256, alloy::contract::Error> {
        {
            let storage = self.storage.read();

            if let Some(value) = storage.get(&B256::from(slot)) {
                return Ok(*value);
            }
        };

        let value = provider
            .get_storage_at(self.address, slot)
            .block_id(block)
            .await?;

        self.storage.write().insert(slot.into(), value);

        Ok(value)
    }

    pub async fn get_consecutive<P: Provider>(
        &self,
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

        let storage = self.storage.read();

        for i in 0..amount {
            let key: B256 = (slot + U256::from(i)).into();
            values.push(*storage.get(&key)?);
        }

        Some(values)
    }

    pub fn insert(&self, slot: U256, value: U256) {
        self.storage.write().insert(slot.into(), value);
    }

    pub fn clear(&self) {
        self.storage.write().clear();
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
