use alloy::{
    primitives::{Address, U256, address},
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

    pub async fn get_amount_out(
        &self,
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
    ) -> Result<U256, alloy::contract::Error> {
        self.0
            .get_amount_out(amount_in, reserve_in, reserve_out)
            .await
    }
}
