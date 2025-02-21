use std::ops::Deref;

use alloy::{
    primitives::{Address, address},
    providers::Provider,
};
use plutus_defi_protocols_uniswap::v3::quoter::Quoter;

pub use plutus_defi_protocols_uniswap::v3::quoter::IQuoterV2::QuoteExactInputSingleParams;

const DEPLOYMENT_ADDRESS: Address = address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997");

pub struct PancakeSwapV3Quoter<P>(Quoter<P>);

impl<P: Provider> PancakeSwapV3Quoter<P> {
    pub fn new(provider: P) -> Self {
        Self(Quoter::new_on_address(DEPLOYMENT_ADDRESS, provider))
    }
}

impl<P> Deref for PancakeSwapV3Quoter<P> {
    type Target = Quoter<P>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
