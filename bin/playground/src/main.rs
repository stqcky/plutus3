use alloy::{
    eips::BlockId,
    hex,
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::{Address, BlockNumber, U64, U256, address, bytes, map::AddressSet},
    providers::{Provider, ProviderCall, ProviderLayer, RootProvider, WsConnect},
    rpc::{client::NoParams, types::BlockTransactionsKind},
    signers::local::PrivateKeySigner,
    sol_types::RevertReason,
    transports::BoxTransport,
};
use futures::{FutureExt, future};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_defi_protocols_protocol::{Protocol, registry::ProtocolRegistry};
use plutus_evm::EVM;
use plutus_executor::Executor;
use plutus_monitoring::{StateChange, StateMonitor, health::HealthMonitor};
use plutus_storage::{IdentifiedLiquidityPool, Storage};
use plutus_token_graph::{OpportunityLeg, TokenGraph};
use std::{sync::Arc, time::Instant};
use tokio::sync::{Semaphore, mpsc};

use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_protocol::ProtocolFactory;
use plutus_defi_protocols_uniswap::{
    v2::{UniswapV2Protocol, factory::UniswapV2Factory, pool::UniswapV2Pool},
    v3::{UniswapV3Protocol, pool::UniswapV3Pool},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();

    Ok(())
}
