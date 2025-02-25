use std::sync::Arc;

use alloy::network::{Ethereum, EthereumWallet};
use alloy::primitives::Address;
use alloy::providers::RootProvider;
use alloy::providers::fillers::{FillProvider, WalletFiller};
use alloy::sol_types::SolCall as _;
use alloy::{
    primitives::U256,
    providers::{
        Identity, Provider, ProviderBuilder,
        fillers::{BlobGasFiller, ChainIdFiller, GasFiller, JoinFill, NonceFiller},
        layers::AnvilProvider,
    },
    transports::BoxTransport,
};
use contract::IExecutor::{self, IExecutorInstance};
use plutus_defi_protocols_uniswap::v3::pool::IUniswapV3Pool::swapCall;
use plutus_token_graph::OpportunityLeg;

mod contract;

type FakeProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    AnvilProvider<RootProvider<BoxTransport>, BoxTransport>,
    BoxTransport,
    Ethereum,
>;

pub struct Executor {
    provider: Arc<FakeProvider>,
    contract: IExecutorInstance<BoxTransport, Arc<FakeProvider>>,
}

pub struct ExecutionStep {
    target: Address,
    data: Vec<u8>,
}

impl Executor {
    pub async fn new() -> anyhow::Result<Self> {
        let provider = Arc::new(
            ProviderBuilder::default()
                .with_recommended_fillers()
                .on_anvil_with_wallet_and_config(|anvil| anvil.fork("http:://localhost:8547")),
        );

        let contract = IExecutor::deploy(provider.clone()).await?;

        Ok(Self { provider, contract })
    }

    pub fn execute<P: Provider>(steps: Vec<OpportunityLeg<P>>, amount: U256) {}

    fn create_execution_step<P: Provider>(&self, step: &OpportunityLeg<P>) {
        assert!(step.pool.identifier() == "uniswap_v3", "not uniswap v3");
        // let zero_for_one = step.token0.address == step.pool.token_addresses().0;

        // swapCall::new((*self.contract.address(), zero_for_one, ));
    }
}
