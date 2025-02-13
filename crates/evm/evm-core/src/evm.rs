use crate::errors::EvmCallError;
use alloy::{
    network::Ethereum,
    primitives::BlockNumber,
    providers::Provider,
    sol_types::{Revert, SolCall, SolError},
    transports::BoxTransport,
};
use revm::{
    Context, Database as _, ExecuteEvm, MainBuilder, MainContext,
    context_interface::result::{ExecutionResult, Output},
    database_interface::WrapDatabaseAsync,
    primitives::{Address, TxKind, U256},
};
use revm_database::{AlloyDB, CacheDB};

pub type EvmDatabase<P> = CacheDB<WrapDatabaseAsync<AlloyDB<BoxTransport, Ethereum, P>>>;

pub struct EVM<P: Provider> {
    db: EvmDatabase<P>,
    static_block_number: Option<BlockNumber>,
}

pub struct EvmCall<T> {
    pub gas_used: u64,
    pub output: T,
}

impl<P: Provider> EVM<P> {
    pub fn new(provider: P, block_number: BlockNumber) -> Self {
        let alloydb = AlloyDB::new(provider, block_number.into());
        let db = CacheDB::new(WrapDatabaseAsync::new(alloydb).expect("tokio runtime is available"));

        Self {
            db,
            static_block_number: None,
        }
    }

    pub fn new_on_block(provider: P, block_number: BlockNumber) -> Self {
        let alloydb = AlloyDB::new(provider, block_number.into());
        let db = CacheDB::new(WrapDatabaseAsync::new(alloydb).expect("tokio runtime is available"));

        Self {
            db,
            static_block_number: Some(block_number),
        }
    }

    pub fn storage(&mut self, address: Address, slot: U256) -> U256 {
        self.db.storage(address, slot).unwrap_or_default()
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
            .modify_block_chained(|block| {
                if let Some(block_number) = self.static_block_number {
                    block.number = block_number;
                }
            })
            .modify_tx_chained(|tx| {
                tx.kind = TxKind::Call(to);
                tx.data = call.abi_encode().into();
            })
            .build_mainnet();

        let result = evm.transact_previous()?.result;

        Self::map_execution_result::<T>(result)
    }

    fn map_execution_result<T: SolCall>(
        result: ExecutionResult,
    ) -> Result<EvmCall<T::Return>, EvmCallError<P>> {
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
