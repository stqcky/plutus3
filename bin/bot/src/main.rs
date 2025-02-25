use futures::future;
use plutus_defi_price_oracle::PriceOracle;
use rayon::prelude::*;
use std::{sync::Arc, time::Instant};

use alloy::{
    eips::BlockId,
    primitives::{Address, U256, address, map::AddressSet},
    providers::{Provider, ProviderBuilder, RpcWithBlock},
    rpc::client::ClientBuilder,
};
use dotenvy_macro::dotenv;
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_evm::EVM;
use plutus_monitoring::{StateChange, StateMonitor, health::HealthMonitor};
use plutus_storage::Storage;
use plutus_token_graph::{Step, TokenGraph};
use tokio::sync::{Semaphore, mpsc};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env()
        .unwrap();

    tracing_subscriber::fmt()
        // .with_env_filter(filter)
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();
}

const UPDATE_CACHE: bool = false;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let provider = Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    );

    let start_block = provider.get_block_number().await?;

    tracing::info!("startup block: {start_block}");

    let state_monitor = StateMonitor::new(provider.clone());

    let (state_tx, mut state_rx) = mpsc::channel::<StateChange>(1024);

    state_monitor.subscribe_blocks(state_tx).await?;

    // let blockchain = Blockchain::new(provider.get_chain_id().await?);
    let storage = Storage::new().await?;

    let protocol_registry = Arc::new(
        ProtocolRegistry::new(provider.clone())
            .await?
            .with::<UniswapV2Protocol>()?
            .with::<UniswapV3Protocol>()?
            .with::<PancakeSwapV2Protocol>()?,
    );

    protocol_registry
        .discover_and_store(start_block, &storage)
        .await?;

    let pools = if UPDATE_CACHE {
        tracing::info!("updating cache");

        let now = Instant::now();

        let filtered = protocol_registry
            .get_filtered_pools(&storage, 2_000.0, start_block.into())
            .await?;

        protocol_registry
            .cache_filtered_pools(&storage, &filtered)
            .await?;

        tracing::info!("filtered in {:?}", now.elapsed());

        filtered
    } else {
        protocol_registry
            .get_cached_filtered_pools(&storage, start_block.into())
            .await?
    };

    tracing::info!("pool count: {}", pools.len());

    tracing::info!("creating token graph");
    let now = Instant::now();
    let mut token_graph =
        TokenGraph::new(pools.clone(), 0.001, start_block.into(), provider.clone()).await?;
    tracing::info!("token graph created in {:?}", now.elapsed());

    let price_oracle = PriceOracle::new(provider.clone()).await?;

    let health_monitor = HealthMonitor::new(provider.clone());
    let mut last_health_check = Instant::now();

    while let Some(state_change) = state_rx.recv().await {
        let current_block = state_change.block_header.number;

        // let catching_up = current_block < provider.get_block_number().await?;
        let catching_up = false;

        tracing::info!(
            "block {current_block}{}",
            if catching_up { ", catching up" } else { "" }
        );
        let current_block: BlockId = current_block.into();

        let affected_tokens = token_graph
            .apply_state(state_change.changes, provider.clone(), current_block)
            .await;

        if catching_up {
            continue;
        }

        if last_health_check.elapsed().as_secs() >= 2 {
            health_monitor
                .check_health(state_change.block_header.number, token_graph.pools.clone());

            last_health_check = Instant::now();
        }

        let now = Instant::now();
        let opportunities = token_graph
            .find_opportunities(affected_tokens.clone(), current_block, provider.clone())
            .await?;
        tracing::info!("found opportunities in {:?}", now.elapsed());

        let mut opportunities_with_usd = vec![];

        for opportunity in opportunities {
            let usd_price = price_oracle
                .clone()
                .get_price(&opportunity.base_token)
                .await;
            let usd_value = usd_price * opportunity.base_token.to_float_amount(opportunity.profit);

            opportunities_with_usd.push((opportunity, usd_value));
        }

        opportunities_with_usd.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        if let Some(best_opportunity) = opportunities_with_usd.get(0) {
            tracing::info!("{}", best_opportunity.0);
            tracing::info!("${}", best_opportunity.1);
        }
    }

    Ok(())
}

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

async fn get_usd_value<P: Provider + std::fmt::Debug + Clone>(
    token: &ERC20,
    amount: f64,
    registry: &ProtocolRegistry<P>,
    block: BlockId,
    provider: P,
) -> f64 {
    let usdt = ERC20::new_with_provider(USDT, provider.clone())
        .await
        .unwrap();
    let usdc = ERC20::new_with_provider(USDC, provider.clone())
        .await
        .unwrap();
    let weth = ERC20::new_with_provider(WETH, provider.clone())
        .await
        .unwrap();

    let token_amount = token.to_token_amount(amount);

    let usdt_value = usdt.to_float_amount(
        registry
            .get_token_value(token.address, USDT, token_amount, block)
            .await
            .unwrap(),
    );

    let usdc_value = usdc.to_float_amount(
        registry
            .get_token_value(token.address, usdc.address, token_amount, block)
            .await
            .unwrap(),
    );

    let weth_value = weth.to_float_amount(
        registry
            .get_token_value(token.address, weth.address, token_amount, block)
            .await
            .unwrap(),
    ) * 2723.39;

    usdt_value.max(usdc_value).max(weth_value)
}
