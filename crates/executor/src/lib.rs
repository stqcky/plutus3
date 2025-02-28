use std::sync::Arc;

use alloy::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy::primitives::{Address, BlockNumber, address};
use alloy::providers::fillers::{FillProvider, WalletFiller};
use alloy::providers::{RootProvider, WalletProvider};
use alloy::sol;
use alloy::sol_types::{SolCall as _, SolType};
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
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_uniswap::v3::pool::IUniswapV3Pool::swapCall;
use plutus_evm::EVM;
use plutus_token_graph::OpportunityLeg;
use plutus_token_graph::calculation::{CalculatedOpportunity, CalculatedOpportunityLeg};

pub mod contract;

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
    block: BlockNumber,
}

pub struct ExecutionStep {
    target: Address,
    data: Vec<u8>,
}

impl Executor {
    pub async fn new(block: BlockNumber) -> anyhow::Result<Self> {
        let provider = Arc::new(
            ProviderBuilder::default()
                .with_recommended_fillers()
                .on_anvil_with_wallet_and_config(|anvil| {
                    anvil.fork("http://localhost:8547").fork_block_number(block)
                }),
        );

        println!("signer: {}", provider.default_signer_address());

        let contract = IExecutor::deploy(provider.clone()).await?;

        println!("contract address: {}", contract.address());

        Ok(Self {
            provider,
            contract,
            block,
        })
    }

    pub async fn execute<P: Provider>(
        &self,
        opportunity: CalculatedOpportunity<P>,
    ) -> anyhow::Result<()> {
        let base = opportunity.base_token;

        let user = self.provider.default_signer_address();

        // let before = base.balance_of(user, self.provider.clone()).await?;
        // println!("base balance before: {before}");

        let other_legs = &opportunity.legs[1..];
        let other_steps = other_legs
            .iter()
            .map(|leg| self.create_execution_step(leg, vec![]))
            .collect::<Vec<_>>();

        let targets = other_steps
            .iter()
            .map(|step| step.target)
            .collect::<Vec<_>>();

        let datas = other_steps
            .into_iter()
            .map(|step| step.data)
            .collect::<Vec<_>>();

        type ExtraData = sol! { tuple (address[], bytes[]) };

        let extra = ExtraData::abi_encode_sequence(&(targets, datas));

        let first_step = self.create_execution_step(&opportunity.legs[0], extra);

        // println!("{}", first_step.target);
        // println!("{}", hex::encode(&first_step.data));

        // let mut evm = EVM::new_on_block(self.provider.clone(), self.block);
        //
        // let res = evm.call(
        //     *self.contract.address(),
        //     executeCall::new((first_step.target, first_step.data.into())),
        // );
        //
        // match res {
        //     Ok(_) => println!("ok"),
        //     Err(err) => {
        //         println!("{err:#?}");
        //     }
        // }

        // let result = self
        //     .contract
        //     .execute1695833(first_step.target, first_step.data.into())
        //     .from(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"))
        //     .block(self.block.into())
        //     .call()
        //     .await;
        //
        // match result {
        //     Err(err) => println!("{err:#?}"),
        //     Ok(a) => println!("ok"),
        // };

        // println!("{receipt:#?}");
        let receipt = self
            .contract
            .execute1695833(first_step.target, first_step.data.into())
            .from(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"))
            .block(self.block.into())
            .send()
            .await?
            .get_receipt()
            .await;

        // println!("{receipt:#?}");

        let after = base
            .balance_of(
                address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
                self.provider.clone(),
            )
            .await?;
        println!("base balance after: {after}");

        Ok(())
    }

    fn create_execution_step<P: Provider>(
        &self,
        leg: &CalculatedOpportunityLeg<P>,
        extra: Vec<u8>,
    ) -> ExecutionStep {
        let data = leg.pool.create_payload(
            *self.contract.address(),
            leg.token_in.address,
            leg.amount_in,
            extra,
        );

        ExecutionStep {
            target: leg.pool.address(),
            data,
        }
    }
}
