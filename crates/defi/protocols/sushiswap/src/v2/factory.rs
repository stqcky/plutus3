use std::ops::Deref;

use alloy::primitives::{Address, ChainId, address};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v2::factory::UniswapV2Factory;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
    )]);
}
#[derive(Clone, Copy)]
pub struct SushiSwapV2Factory(UniswapV2Factory);

impl Deref for SushiSwapV2Factory {
    type Target = UniswapV2Factory;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SushiSwapV2Factory {
    pub fn new(address: Address) -> Self {
        Self(UniswapV2Factory::new(address))
    }
}
