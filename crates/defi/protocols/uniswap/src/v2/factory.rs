use IUniswapV2Factory::{IUniswapV2FactoryInstance, PairCreated, getPairCall};
use alloy::{
    primitives::{Address, BlockNumber, ChainId, address},
    providers::Provider,
    sol,
    sol_types::SolCall as _,
};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use plutus_evm::{EVM, errors::EvmCallError};

lazy_static! {
    pub static ref FACTORY_ADDRESS: HashMap<ChainId, Address> =
        HashMap::from([(42161, address!("f1D7CC64Fb4452F05c498126312eBE29f30Fbcf9"))]);
}

sol!(
    #[sol(rpc)]
    contract IUniswapV2Factory {
        mapping(address => mapping(address => address)) public getPair;
        event PairCreated(address indexed token0, address indexed token1, address pair, uint);
    }
);

pub struct UniswapV2Factory {
    address: Address,
}

impl UniswapV2Factory {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub fn get_pool_address<P: Provider>(
        &self,
        token0: Address,
        token1: Address,
        evm: &mut EVM<P>,
    ) -> Result<Address, EvmCallError<P>> {
        Ok(evm
            .call(self.address, getPairCall::new((token0, token1)))?
            .output
            ._0)
    }

    pub async fn pair_created_events<P: Provider>(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        provider: P,
    ) -> Result<Vec<PairCreated>, alloy::contract::Error> {
        Ok(IUniswapV2FactoryInstance::new(self.address, provider)
            .PairCreated_filter()
            .from_block(from)
            .to_block(to)
            .query()
            .await?
            .into_iter()
            .map(|(event, _)| event)
            .collect())
    }
}
