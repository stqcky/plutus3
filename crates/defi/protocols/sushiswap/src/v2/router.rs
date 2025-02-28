use std::ops::Deref;

use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use plutus_defi_protocols_uniswap::v2::router::UniswapV2Router;

const DEPLOYMENT_ADDRESS: Address = address!("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506");

pub struct SushiSwapV2Router<P>(UniswapV2Router<P>);

impl<P: Provider> SushiSwapV2Router<P> {
    pub fn new(provider: P) -> Self {
        Self(UniswapV2Router::new_on_address(
            DEPLOYMENT_ADDRESS,
            provider,
        ))
    }
}

impl<P> Deref for SushiSwapV2Router<P> {
    type Target = UniswapV2Router<P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
