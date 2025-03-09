use plutus_defi_price_oracle::PriceOracle;
use plutus_defi_protocols_sushiswap::{v2::SushiSwapV2Protocol, v3::SushiSwapV3Protocol};
use plutus_executor::Executor;
use std::{sync::Arc, time::Instant};

use alloy::{
    eips::BlockId,
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, WalletProvider},
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
            .wallet(wallet.clone())
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

    let price_oracle = PriceOracle::new(
        provider.clone(),
        token_graph.pools.clone(),
        token_graph.pool_map.clone(),
    )
    .await?;

    tracing::info!("prefetching prices");
    let now = Instant::now();
    price_oracle
        .clone()
        .prefetch_prices(token_graph.get_tokens())
        .await;
    tracing::info!("prefetched prices in {:?}", now.elapsed());

    let health_monitor = HealthMonitor::new(provider.clone());
    let mut last_health_check = Instant::now();

    let mut executor = Executor::new(
        provider
            .get_transaction_count(provider.default_signer_address())
            .await?,
        plutus_arbitrum::create_provider(wallet),
    )
    .await?;

    while let Some(state_change) = state_rx.recv().await {
        let now = Instant::now();
        let current_block = state_change.block_header.number;

        let catching_up = current_block < provider.get_block_number().await?;

        tracing::info!(
            "block {current_block}{}",
            if catching_up { ", catching up" } else { "" }
        );
        let current_block: BlockId = current_block.into();

        let now_apply_state = Instant::now();
        let (affected_tokens, affected_pools) = token_graph
            .apply_state(state_change.changes, provider.clone(), current_block)
            .await;
        // tracing::info!("token_graph.apply_state in {:?}", now_apply_state.elapsed());

        if catching_up {
            continue;
        }

        if last_health_check.elapsed().as_secs() >= 2 {
            // health_monitor
            //     .check_health(state_change.block_header.number, token_graph.pools.clone());

            last_health_check = Instant::now();
        }

        let now_find_opportunities = Instant::now();
        let opportunities = token_graph
            .find_opportunities(
                affected_tokens.clone(),
                affected_pools,
                current_block,
                provider.clone(),
            )
            .await?;
        tracing::info!(
            "token_graph.find_opportunities: {:?}",
            now_find_opportunities.elapsed()
        );

        if opportunities.len() == 0 {
            // tracing::warn!("no opportunities");
        }

        // tracing::info!("found opportunities in {:?}", now.elapsed());

        let mut opportunities_with_usd = vec![];

        let now_prices = Instant::now();
        for opportunity in opportunities {
            let usd_price = price_oracle
                .clone()
                .get_price(&opportunity.base_token)
                .await;
            let usd_value = usd_price * opportunity.base_token.to_float_amount(opportunity.profit);

            opportunities_with_usd.push((opportunity, usd_value));
        }
        // tracing::info!("got prices in {:?}", now_prices.elapsed());

        opportunities_with_usd.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        if let Some(best_opportunity) = opportunities_with_usd.get_mut(0) {
            let opportunity = &best_opportunity.0;
            let usd_value = best_opportunity.1;

            let base_fee_per_gas = state_change.block_header.base_fee_per_gas.unwrap();
            let gas_price = price_oracle
                .clone()
                .get_eth_price(U256::from(base_fee_per_gas * 400_000))
                .await;

            // tracing::info!(
            //     "ETH: {}",
            //     price_oracle.clone().get_eth_price(U256::from(1e18)).await
            // );

            // if let Ok(gas) = executor
            //     .estimate_gas(opportunity, provider.default_signer_address())
            //     .await
            // {
            //     tracing::info!("gas: {gas} {}", gas);
            //     let gas_price = price_oracle.clone().get_eth_price(U256::from(gas)).await;
            //     tracing::info!("gas price: ${gas_price}");
            // } else {
            //     tracing::error!("gas error");
            // }
            if usd_value >= 0.011 {
                tracing::info!("${}", usd_value);
                tracing::info!("${gas_price} gas");

                tracing::info!("{}", opportunity);

                if usd_value >= gas_price {
                    tracing::info!("starting execution, {:?}", now.elapsed());
                    let now = Instant::now();
                    // _ = executor
                    //     .execute(opportunity)
                    //     .await
                    //     .inspect_err(|err| tracing::error!("{err}"));
                    tracing::info!("executed in {:?}", now.elapsed());
                } else {
                    tracing::warn!("less than gas");
                }

                // panic!("yo");
            }
        }

        tracing::info!("processed block in {:?}", now.elapsed());
    }

    Ok(())
}
