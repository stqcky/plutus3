use alloy::eips::eip2718::Encodable2718;
use alloy::network::TransactionBuilder;
use alloy::primitives::Address;
use alloy::providers::WalletProvider;
use alloy::rpc::types::TransactionRequest;
use alloy::sol;
use alloy::sol_types::{SolCall as _, SolType};
use alloy::transports::Transport;
use alloy::{providers::Provider, transports::BoxTransport};
use contract::IExecutor::{self, IExecutorInstance, execute1695833Call};
use dotenvy_macro::dotenv;
use plutus_token_graph::calculation::{CalculatedOpportunity, CalculatedOpportunityLeg};
use std::time::Instant;

pub mod contract;

pub struct Executor<T, P> {
    contract: IExecutorInstance<T, P>,
    provider: P,
    nonce: u64,
}

pub struct ExecutionStep {
    target: Address,
    data: Vec<u8>,
}

impl<T: Transport + Clone, P: Provider<T> + Clone + WalletProvider> Executor<T, P> {
    pub async fn new(nonce: u64, provider: P) -> anyhow::Result<Self> {
        let contract = IExecutor::new(
            dotenv!("EXECUTOR_DEPLOYMENT_ADDRESS").parse().unwrap(),
            provider.clone(),
        );

        Ok(Self {
            nonce,
            contract,
            provider,
        })
    }

    pub async fn execute<PP: Provider>(
        &mut self,
        opportunity: &CalculatedOpportunity<PP>,
    ) -> anyhow::Result<()> {
        let now = Instant::now();
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
        tracing::info!("prepared to execute in {:?}", now.elapsed());

        let now = Instant::now();
        let tx = self
            .contract
            .execute1695833(first_step.target, first_step.data.into())
            .into_transaction_request()
            .with_nonce(self.nonce)
            .with_gas_limit(1_000_000)
            .max_fee_per_gas(30_000_000)
            .max_priority_fee_per_gas(0)
            .with_chain_id(42161);

        let tx_envelope = tx.build(self.provider.wallet()).await?;
        let tx_encoded = tx_envelope.encoded_2718();

        self.nonce += 1;

        let now_send = Instant::now();
        let pending_tx = self.provider.send_raw_transaction(&tx_encoded).await?;
        tracing::info!("sent in {:?}", now_send.elapsed());

        tracing::info!("constructed and sent raw tx in {:?}", now.elapsed());
        // tracing::info!("{}", pending_tx)

        Ok(())
    }

    // pub async fn estimate_gas(
    //     &self,
    //     opportunity: &CalculatedOpportunity<P>,
    //     from: Address,
    // ) -> anyhow::Result<u64> {
    //     let other_legs = &opportunity.legs[1..];
    //     let other_steps = other_legs
    //         .iter()
    //         .map(|leg| self.create_execution_step(leg, vec![]))
    //         .collect::<Vec<_>>();
    //
    //     let targets = other_steps
    //         .iter()
    //         .map(|step| step.target)
    //         .collect::<Vec<_>>();
    //
    //     let datas = other_steps
    //         .into_iter()
    //         .map(|step| step.data)
    //         .collect::<Vec<_>>();
    //
    //     type ExtraData = sol! { tuple (address[], bytes[]) };
    //
    //     let extra = ExtraData::abi_encode_sequence(&(targets, datas));
    //
    //     let first_step = self.create_execution_step(&opportunity.legs[0], extra);
    //
    //     Ok(self
    //         .contract
    //         .execute1695833(first_step.target, first_step.data.into())
    //         .from(from)
    //         .estimate_gas()
    //         .await?)
    // }

    fn create_execution_step<PP: Provider>(
        &self,
        leg: &CalculatedOpportunityLeg<PP>,
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
