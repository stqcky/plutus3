use std::{
    collections::HashMap,
    ops::{Add, Deref, DerefMut, Sub},
};

use derive_more::Add;
use revm::primitives::{B256, U256};

impl DecodableValue for U256 {
    fn decode(value: B256) -> Self {
        todo!()
    }
}

pub trait DecodableValue {
    fn decode(value: B256) -> Self;
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Add)]
pub struct StorageSlot<T, const SLOT: usize>(T);

impl<T, const SLOT: usize> Deref for StorageSlot<T, SLOT> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const SLOT: usize> StorageSlot<T, SLOT> {
    pub fn decode(&mut self, value: B256)
    where
        T: DecodableValue,
    {
        self.0 = T::decode(value);
    }

    pub fn slot(&self) -> B256 {
        U256::from(SLOT).into()
    }
}

impl<T, const SLOT: usize> DerefMut for StorageSlot<T, SLOT> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// impl<T, const SLOT: usize> Add<T> for StorageSlot<T, SLOT>
// where
//     T: Add<Output = T>,
// {
//     type Output = T;
//
//     fn add(self, rhs: T) -> Self::Output {
//         self.0 + rhs
//     }
// }

impl<T: Sub<Output = T>, const SLOT: usize> Sub<T> for StorageSlot<T, SLOT> {
    type Output = T;

    fn sub(self, rhs: T) -> Self::Output {
        self.0 - rhs
    }
}

impl<T, const SLOT: usize> From<T> for StorageSlot<T, SLOT> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
