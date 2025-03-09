use std::ops::Deref;

use alloy::primitives::{Address, ChainId, address};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v2::factory::UniswapV2Factory;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> =
        HashMap::from([(42161, address!("02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E"))]);
}
#[derive(Clone, Copy)]
pub struct PancakeSwapV2Factory(UniswapV2Factory);

impl Deref for PancakeSwapV2Factory {
    type Target = UniswapV2Factory;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PancakeSwapV2Factory {
    pub fn new(address: Address) -> Self {
        Self(UniswapV2Factory::new(address))
    }
}
