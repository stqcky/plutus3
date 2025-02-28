use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId, address, aliases::U24},
    providers::Provider,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::{
    v2::factory::{IUniswapV2Factory::PairCreated, UniswapV2Factory},
    v3::{
        factory::{
            IUniswapV3Factory::{IUniswapV3FactoryInstance, PoolCreated},
            UniswapV3Factory,
        },
        fee::FeeAmount,
    },
};
use plutus_evm::{EVM, errors::EvmCallError};

use super::pool::SushiSwapV3Pool;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0x1af415a1EbA07a4986a52B6f2e7dE7003D82231e")
    )]);
}
#[derive(Clone, Copy)]
pub struct SushiSwapV3Factory {
    address: Address,
}

impl SushiSwapV3Factory {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub async fn get_pool_with_provider<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        fee: FeeAmount,
        provider: P,
    ) -> Result<Option<SushiSwapV3Pool>, alloy::contract::Error> {
        let instance = IUniswapV3FactoryInstance::new(self.address, &provider);

        let address = instance
            .getPool(token0, token1, U24::from(fee as u32))
            .call()
            .await?
            .pool;

        if address.is_zero() {
            Ok(None)
        } else {
            Ok(Some(
                SushiSwapV3Pool::new_with_provider(address, provider, BlockId::latest()).await?,
            ))
        }
    }

    pub async fn pool_created_events<P: Provider>(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<PoolCreated>, alloy::contract::Error> {
        Ok(IUniswapV3FactoryInstance::new(self.address, provider)
            .PoolCreated_filter()
            .from_block(from)
            .to_block(to)
            .query()
            .await?
            .into_iter()
            .map(|(event, _)| event)
            .collect())
    }
}
