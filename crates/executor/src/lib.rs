use alloy::primitives::Address;
use alloy::sol;
use alloy::sol_types::SolType;
use alloy::{providers::Provider, transports::BoxTransport};
use contract::IExecutor::{self, IExecutorInstance};
use dotenvy_macro::dotenv;
use plutus_token_graph::calculation::{CalculatedOpportunity, CalculatedOpportunityLeg};

pub mod contract;

pub struct Executor<P> {
    contract: IExecutorInstance<BoxTransport, P>,
}

pub struct ExecutionStep {
    target: Address,
    data: Vec<u8>,
}

impl<P: Provider + Clone> Executor<P> {
    pub async fn new(provider: P) -> anyhow::Result<Self> {
        let contract = IExecutor::new(
            dotenv!("EXECUTOR_DEPLOYMENT_ADDRESS").parse().unwrap(),
            provider.clone(),
        );

        Ok(Self { contract })
    }

    pub async fn execute(&self, opportunity: &CalculatedOpportunity<P>) -> anyhow::Result<()> {
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

        println!("target: {}", first_step.target);
        println!("{}", hex::encode(&first_step.data));

        let receipt = self
            .contract
            .execute1695833(first_step.target, first_step.data.into())
            .send()
            .await?
            .get_receipt()
            .await;

        println!("{receipt:#?}");

        Ok(())
    }

    fn create_execution_step(
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
