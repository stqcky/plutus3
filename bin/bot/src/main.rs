use plutus_defi_price_oracle::PriceOracle;
use plutus_defi_protocols_sushiswap::{v2::SushiSwapV2Protocol, v3::SushiSwapV3Protocol};
use plutus_executor::Executor;
use std::{sync::Arc, time::Instant};

use alloy::{
    eips::BlockId,
    network::EthereumWallet,
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
    signers::local::PrivateKeySigner,
};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_pancakeswap::{v2::PancakeSwapV2Protocol, v3::PancakeSwapV3Protocol};
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_monitoring::{StateChange, StateMonitor, health::HealthMonitor};
use plutus_storage::Storage;
use plutus_token_graph::TokenGraph;
use tokio::sync::mpsc;

fn init_tracing() {
    // let filter = EnvFilter::builder()
    //     .with_default_directive(LevelFilter::DEBUG.into())
    //     .from_env()
    //     .unwrap();

    tracing_subscriber::fmt()
        // .with_env_filter(filter)
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();
}

const UPDATE_CACHE: bool = false;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let signer: PrivateKeySigner = dotenv!("PRIVATE_KEY").parse().unwrap();
    let wallet = EthereumWallet::new(signer);

    let provider = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_client(
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
            .with::<PancakeSwapV2Protocol>()?
            .with::<PancakeSwapV3Protocol>()?
            .with::<SushiSwapV2Protocol>()?
            .with::<SushiSwapV3Protocol>()?,
    );

    protocol_registry
        .discover_and_store(start_block, &storage)
        .await?;

    let pools = if UPDATE_CACHE {
        tracing::info!("updating cache");

        let now = Instant::now();

        let filtered = protocol_registry
            .get_filtered_pools(&storage, 500.0, start_block.into())
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

    let executor = Executor::new(provider.clone()).await?;

    while let Some(state_change) = state_rx.recv().await {
        // let now = Instant::now();
        let current_block = state_change.block_header.number;

        let catching_up = current_block < provider.get_block_number().await?;

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

        tracing::info!("find_opportunities: {:?}", now.elapsed());

        if opportunities.len() == 0 {
            tracing::warn!("no opportunities");
        }

        // tracing::info!("found opportunities in {:?}", now.elapsed());

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

        if let Some(best_opportunity) = opportunities_with_usd.get_mut(0) {
            let opportunity = &best_opportunity.0;
            let usd_value = best_opportunity.1;

            tracing::info!("${}", usd_value);
            // if usd_value >= 0.011 {
            if usd_value >= 1.0 {
                tracing::info!("{}", opportunity);
                _ = executor
                    .execute(opportunity)
                    .await
                    .inspect_err(|err| tracing::error!("{err}"));

                // panic!("yo");
            }
        }

        tracing::info!("processed block in {:?}", now.elapsed());
    }

    Ok(())
}
