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

    let provider = Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    );

    let mut pool = UniswapV3Pool::new_with_provider(
        address!("0x44c40a6544f29f331720E989Cd2724306b21c0d0"),
        provider.clone(),
        BlockId::latest(),
    )
    .await?;

    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    Ok(())
}
