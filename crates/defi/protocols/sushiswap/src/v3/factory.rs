use std::ops::Deref;

use alloy::primitives::{Address, ChainId, address};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v3::factory::UniswapV3Factory;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0x1af415a1EbA07a4986a52B6f2e7dE7003D82231e")
    )]);
}
#[derive(Clone, Copy)]
pub struct SushiSwapV3Factory(UniswapV3Factory);

impl Deref for SushiSwapV3Factory {
    type Target = UniswapV3Factory;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SushiSwapV3Factory {
    pub fn new(address: Address) -> Self {
        Self(UniswapV3Factory::new(address))
    }
}
