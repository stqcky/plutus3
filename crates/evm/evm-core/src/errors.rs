use alloy::{providers::Provider, sol_types::Revert};
use revm::{
    Database,
    context_interface::result::{EVMError, HaltReason, InvalidTransaction},
    primitives::Bytes,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EvmCallError<P: Provider> {
    #[error("invalid transaction: {0}")]
    InvalidTransaction(
        #[from] EVMError<<crate::EvmDatabase<P> as Database>::Error, InvalidTransaction>,
    ),

    #[error("transaction reverted")]
    Revert { reason: RevertReason, gas_used: u64 },

    #[error("transaction halted: {reason:?}")]
    Halt { reason: HaltReason, gas_used: u64 },

    #[error("failed to decode transaction output: {0}")]
    Decode(#[from] alloy::sol_types::Error),

    #[error("invalid create output")]
    CreateOutput,
}

#[derive(Debug)]
pub enum RevertReason {
    String(String),
    Unknown(Bytes),
}

impl From<Revert> for RevertReason {
    fn from(value: Revert) -> Self {
        Self::String(value.reason)
    }
}

impl From<Bytes> for RevertReason {
    fn from(value: Bytes) -> Self {
        Self::Unknown(value)
    }
}
