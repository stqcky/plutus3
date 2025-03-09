use std::ops::Deref;

use alloy::primitives::{Address, ChainId, address};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v3::factory::UniswapV3Factory;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865")
    )]);
}

#[derive(Clone, Copy)]
pub struct PancakeSwapV3Factory(UniswapV3Factory);

impl Deref for PancakeSwapV3Factory {
    type Target = UniswapV3Factory;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PancakeSwapV3Factory {
    pub fn new(address: Address) -> Self {
        Self(UniswapV3Factory::new(address))
    }
}
