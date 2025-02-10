// use std::collections::HashMap;
//
// use revm::primitives::{B256, U256};
//
// use crate::storage::{DecodableValue, StorageSlot};
//
// #[derive(DecodableStorageValue)]
// pub struct Slot0 {
//     sqrtprice: U256,
// }
//
// #[derive(SmartContract)]
// pub struct UniswapV3Pool {
//     #[immutable(token0Call)]
//     token0: ERC20,
//
//     #[slot(0)]
//     slot0: Slot0,
//
//     #[slot(1)]
//     fee_growth_0: U256,
//
//     // add
//     // storage: SmartContractStorage
// }
//
// impl DecodableSmartContract for UniswapV3Pool {
//     fn decode(&mut self, storage: HashMap<B256, B256>) {
//
//     }
// }
//
// impl DecodableStorageValue for Slot0 {
//     const SIZE = 1;
//
//     fn decode(&mut self, bytes: Vec<u8>) {
//         // self =
//     }
// }
//
// impl UniswapV3Pool {
//     pub fn apply_storage_changes(&mut self, storage: HashMap<B256, B256>) {
//         let a = self.slot0.sqrtprice;
//         let b = self.fee_growth_0 + U256::from(1);
//
//         for (k, v) in storage {
//             match k {
//                 _ if k == self.slot0.slot() => self.slot0.decode(v),
//                 _ if k == self.fee_growth_0.slot() => self.fee_growth_0.decode(v),
//                 _ => { /* insert into common storage */ }
//             }
//         }
//     }
// }
//
// impl DecodableValue for Slot0 {
//     fn decode(value: B256) -> Self {
//         todo!()
//     }
// }
