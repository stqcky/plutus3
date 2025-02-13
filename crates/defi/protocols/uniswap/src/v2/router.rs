use IUniswapV2Router02::IUniswapV2Router02Instance;
use alloy::{
    primitives::{Address, U256, address},
    providers::Provider,
    sol,
    transports::BoxTransport,
};

sol!(
    #[sol(rpc)]
    contract IUniswapV2Router02 {
       function getAmountOut(uint amountIn, uint reserveIn, uint reserveOut)
            public
            pure
            virtual
            override
            returns (uint amountOut);
    }
);

const DEPLOYMENT_ADDRESS: Address = address!("4752ba5dbc23f44d87826276bf6fd6b1c372ad24");

pub struct UniswapV2Router<P>(IUniswapV2Router02Instance<BoxTransport, P>);

impl<P: Provider> UniswapV2Router<P> {
    pub fn new(provider: P) -> Self {
        Self(IUniswapV2Router02Instance::new(
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
        Ok(self
            .0
            .getAmountOut(amount_in, reserve_in, reserve_out)
            .call()
            .await?
            .amountOut)
    }
}
