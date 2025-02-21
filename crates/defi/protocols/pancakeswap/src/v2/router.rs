use std::ops::Deref;

use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use plutus_defi_protocols_uniswap::v2::router::UniswapV2Router;

const DEPLOYMENT_ADDRESS: Address = address!("0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb");

pub struct PancakeSwapV2Router<P>(UniswapV2Router<P>);

impl<P: Provider> PancakeSwapV2Router<P> {
    pub fn new(provider: P) -> Self {
        Self(UniswapV2Router::new_on_address(
            DEPLOYMENT_ADDRESS,
            provider,
        ))
    }
}

impl<P> Deref for PancakeSwapV2Router<P> {
    type Target = UniswapV2Router<P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
