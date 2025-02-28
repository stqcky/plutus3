use alloy::{
    primitives::{Address, BlockNumber, ChainId, address},
    providers::Provider,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v2::factory::{
    IUniswapV2Factory::PairCreated, UniswapV2Factory,
};
use plutus_evm::{EVM, errors::EvmCallError};

use super::pool::SushiSwapV2Pool;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0xc35DADB65012eC5796536bD9864eD8773aBc74C4")
    )]);
}
#[derive(Clone, Copy)]
pub struct SushiSwapV2Factory(UniswapV2Factory);

impl SushiSwapV2Factory {
    pub fn new(address: Address) -> Self {
        Self(UniswapV2Factory::new(address))
    }

    pub fn get_pool<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Option<SushiSwapV2Pool>, EvmCallError<P>> {
        let pool = self.0.get_pool(token0, token1, evm)?;
        Ok(pool.map(SushiSwapV2Pool))
    }

    pub async fn get_pool_with_provider<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        provider: P,
    ) -> Result<Option<SushiSwapV2Pool>, alloy::contract::Error> {
        let pool = self
            .0
            .get_pool_with_provider(token0, token1, provider)
            .await?;

        Ok(pool.map(SushiSwapV2Pool))
    }

    pub async fn pair_created_events<P: Provider>(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<PairCreated>, alloy::contract::Error> {
        self.0.pair_created_events(from, to, provider).await
    }
}
