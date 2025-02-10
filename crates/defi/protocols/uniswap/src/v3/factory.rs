use IUniswapV3Factory::{IUniswapV3FactoryInstance, PoolCreated};
use alloy::{
    primitives::{Address, BlockNumber, ChainId, address},
    providers::Provider,
    sol,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;

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
