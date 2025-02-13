use IQuoterV2::{IQuoterV2Instance, QuoteExactInputSingleParams, quoteExactInputSingleReturn};
use alloy::{
    eips::BlockId,
    primitives::{Address, address},
    providers::Provider,
    sol,
    transports::BoxTransport,
};

sol!(
    #[sol(rpc)]
    contract IQuoterV2 {
        struct QuoteExactInputSingleParams {
            address token_in;
            address token_out;
            uint256 amount_in;
            uint24 fee;
            uint160 sqrt_price_limit_x96;
        }

        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
            external
            returns (
                uint256 amount_out,
                uint160 sqrt_price_x96_after,
                uint32 initialized_ticks_crossed,
                uint256 gas_estimate
            );
    }
);

const DEPLOYMENT_ADDRESS: Address = address!("61fFE014bA17989E743c5F6cB21bF9697530B21e");

pub struct Quoter<P>(IQuoterV2Instance<BoxTransport, P>);

impl<P: Provider> Quoter<P> {
    pub fn new(provider: P) -> Self {
        Self(IQuoterV2Instance::new(DEPLOYMENT_ADDRESS, provider))
    }

    pub async fn quote_exact_input_single(
        &self,
        params: QuoteExactInputSingleParams,
    ) -> Result<quoteExactInputSingleReturn, alloy::contract::Error> {
        self.0.quoteExactInputSingle(params).call().await
    }

    pub async fn quote_exact_input_single_on_block(
        &self,
        params: QuoteExactInputSingleParams,
        block: BlockId,
    ) -> Result<quoteExactInputSingleReturn, alloy::contract::Error> {
        self.0
            .quoteExactInputSingle(params)
            .block(block)
            .call()
            .await
    }
}
