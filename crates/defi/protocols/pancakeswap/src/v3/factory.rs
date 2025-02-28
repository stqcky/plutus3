use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, ChainId, address, aliases::U24},
    providers::Provider,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_defi_protocols_uniswap::v3::{
    factory::IUniswapV3Factory::{IUniswapV3FactoryInstance, PoolCreated},
    fee::FeeAmount,
};

use super::pool::PancakeSwapV3Pool;

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> = HashMap::from([(
        42161,
        address!("0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865")
    )]);
}

#[derive(Clone, Copy)]
pub struct PancakeSwapV3Factory {
    address: Address,
}

impl PancakeSwapV3Factory {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub async fn get_pool_with_provider<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        fee: FeeAmount,
        provider: P,
    ) -> Result<Option<PancakeSwapV3Pool>, alloy::contract::Error> {
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
                PancakeSwapV3Pool::new_with_provider(address, provider, BlockId::latest()).await?,
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
