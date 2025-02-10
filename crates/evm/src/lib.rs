use alloy::{
    network::Ethereum,
    primitives::ChainId,
    providers::Provider,
    sol_types::{Revert, SolCall, SolError},
    transports::BoxTransport,
};
use anyhow::Context as _;
use errors::EvmCallError;
use revm::{
    Context, ExecuteEvm, MainBuilder, MainContext,
    context_interface::result::{ExecutionResult, Output},
    database_interface::WrapDatabaseAsync,
    primitives::{Address, TxKind},
};
use revm_database::{AlloyDB, BlockId, CacheDB};

pub use revm;

pub mod errors;
pub mod storage;
mod test;

type EvmDatabase<P> = CacheDB<WrapDatabaseAsync<AlloyDB<BoxTransport, Ethereum, P>>>;

pub struct EVM<P: Provider> {
    db: EvmDatabase<P>,
    chain_id: ChainId,
}

pub struct EvmCall<T> {
    pub gas_used: u64,
    pub output: T,
}

impl<P: Provider> EVM<P> {
    pub async fn new(provider: P, block: BlockId) -> anyhow::Result<Self> {
        let chain_id = provider
            .get_chain_id()
            .await
            .context("failed to fetch chain id")?;

        let alloydb = AlloyDB::new(provider, block);
        let db = CacheDB::new(WrapDatabaseAsync::new(alloydb).context("failed to wrap database")?);

        Ok(Self { db, chain_id })
    }

    pub fn call<T: SolCall>(
        &mut self,
        to: Address,
        call: T,
    ) -> Result<EvmCall<T::Return>, EvmCallError<P>> {
        let mut evm = Context::mainnet()
            .with_db(&mut self.db)
            .modify_cfg_chained(|cfg| {
                cfg.disable_nonce_check = true;
            })
            .modify_tx_chained(|tx| {
                tx.kind = TxKind::Call(to);
                tx.data = call.abi_encode().into();
            })
            .build_mainnet();

        let result = evm.transact_previous()?.result;

        match result {
            ExecutionResult::Success {
                gas_used,
                output: Output::Call(output),
                ..
            } => Ok(EvmCall {
                gas_used,
                output: T::abi_decode_returns(&output, true)?,
            }),
            ExecutionResult::Revert { gas_used, output } => {
                if let Ok(reason) = Revert::abi_decode(&output, true) {
                    Err(EvmCallError::Revert {
                        reason: reason.into(),
                        gas_used,
                    })
                } else {
                    Err(EvmCallError::Revert {
                        reason: output.into(),
                        gas_used,
                    })
                }
            }
            ExecutionResult::Halt { reason, gas_used } => {
                Err(EvmCallError::Halt { reason, gas_used })
            }
            _ => Err(EvmCallError::CreateOutput),
        }
    }
}
