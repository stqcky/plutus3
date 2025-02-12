use IUniswapV3Factory::{IUniswapV3FactoryInstance, PoolCreated, getPoolCall};
use alloy::{
    primitives::{Address, BlockNumber, ChainId, address, aliases::U24},
    providers::Provider,
    sol,
    sol_types::SolCall as _,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_evm::{EVM, errors::EvmCallError};

use super::{fee::FeeAmount, pool::UniswapV3Pool};

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> =
        HashMap::from([(42161, address!("1F98431c8aD98523631AE4a59f267346ea31F984"))]);
}

sol!(
    #[sol(rpc)]
    contract IUniswapV3Factory {
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24 indexed fee,
            int24 tick_spacing,
            address pool
        );

        function getPool(
            address tokenA,
            address tokenB,
            uint24 fee
        ) external view returns (address pool);
    }
);

pub struct UniswapV3Factory {
    address: Address,
}

impl UniswapV3Factory {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub fn get_pool<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        fee: FeeAmount,
        evm: &mut EVM<P>,
    ) -> Result<Option<UniswapV3Pool>, EvmCallError<P>> {
        let address = evm
            .call(
                self.address,
                getPoolCall::new((token0, token1, U24::from(fee as u32))),
            )?
            .output
            .pool;

        if address.is_zero() {
            Ok(None)
        } else {
            Ok(Some(UniswapV3Pool::new(address, evm)?))
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
