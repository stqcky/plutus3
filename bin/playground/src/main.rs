use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U64, U256, address, map::AddressSet},
    providers::{Provider, ProviderCall, ProviderLayer, RootProvider, WsConnect},
    rpc::{client::NoParams, types::BlockTransactionsKind},
    transports::BoxTransport,
};
use futures::{FutureExt, future};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_defi_protocols_protocol::{Protocol, registry::ProtocolRegistry};
use plutus_evm::EVM;
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

struct FixedBlockLayer {
    block: BlockId,
}

impl<P: Provider> ProviderLayer<P, BoxTransport> for FixedBlockLayer {
    type Provider = FixedBlockProvider<P>;

    fn layer(&self, inner: P) -> Self::Provider {
        FixedBlockProvider {
            inner,
            block: self.block,
        }
    }
}

struct FixedBlockProvider<P> {
    inner: P,
    block: BlockId,
}

impl<P: Provider> Provider for FixedBlockProvider<P> {
    #[inline]
    fn root(&self) -> &RootProvider<BoxTransport> {
        self.inner.root()
    }

    fn get_block_number(&self) -> ProviderCall<BoxTransport, NoParams, U64, BlockNumber> {
        // self.root().get_block_number()
        ProviderCall::Ready(Some(Ok(10)))
    }
}

pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();

    dicks().await?;
    check_arb_on_block(309132690, 0).await?;

    Ok(())
}

async fn dicks() -> anyhow::Result<()> {
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

    let pools = if false {
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

    let health_monitor = HealthMonitor::new(provider.clone());
    let mut last_health_check = Instant::now();

    let mut current_block = start_block + 1;
    loop {
        tracing::info!("block {current_block}",);

        let state_change = state_monitor
            .get_state_changes(
                provider
                    .get_block_by_number(current_block.into(), BlockTransactionsKind::Hashes)
                    .await?
                    .unwrap()
                    .header,
            )
            .await;

        let block_id: BlockId = current_block.into();

        let affected_tokens = token_graph
            .apply_state(state_change.changes, provider.clone(), block_id)
            .await;

        tracing::info!("affected_tokens: {affected_tokens:#?}");

        if last_health_check.elapsed().as_secs() >= 2 {
            health_monitor
                .check_health(state_change.block_header.number, token_graph.pools.clone());

            last_health_check = Instant::now();
        }

        let opportunities = token_graph
            .find_uncalculated_opportunities(affected_tokens)
            .await;

        tracing::info!("found {} opportunities", opportunities.len());

        let semaphore = Arc::new(Semaphore::new(20));
        let tasks: Vec<_> = opportunities
            .into_iter()
            .map(|opportunity| {
                let provider = provider.clone();
                let semaphore = semaphore.clone();
                let protocol_registry = protocol_registry.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire_owned().await.unwrap();

                    check_opportunity(opportunity, &protocol_registry, block_id, provider).await
                })
            })
            .collect();

        let opportunities: Vec<_> = future::try_join_all(tasks)
            .await?
            .into_iter()
            .filter_map(|x| x)
            .collect();

        for opportunity in opportunities {
            let x = opportunity.1;
            let mut steps = opportunity.2;

            simulate_opportunity(
                &mut steps,
                x,
                &protocol_registry,
                block_id,
                provider.clone(),
            )
            .await;
        }

        current_block += 1;
    }

    Ok(())
}

async fn check_opportunity<P: Provider + Clone + std::fmt::Debug>(
    mut opportunity: Vec<Step<P>>,
    protocol_registry: &ProtocolRegistry<P>,
    block: BlockId,
    provider: P,
) -> Option<(f64, f64, Vec<Step<P>>)> {
    let x = optimize_profit(&mut opportunity, block, provider.clone()).await?;

    let profit = calculate_opportunity(&mut opportunity, x, block, provider.clone()).await - x;

    if profit <= 0.0 {
        return None;
    }

    let now = Instant::now();
    let usd_profit = get_usd_value(
        &opportunity[0].token0,
        profit,
        &protocol_registry,
        block,
        provider.clone(),
    )
    .await;
    // tracing::info!("get usd value: {:?}", now.elapsed());

    if usd_profit >= 0.01 {
        // tracing::info!("optimized amount in:");
        // simulate_opportunity(
        //     &mut opportunity,
        //     x,
        //     &protocol_registry,
        //     block,
        //     provider.clone(),
        // )
        // .await;
        // println!("");
        Some((usd_profit, x, opportunity))
    } else {
        None
        // tracing::error!("usd profit < 0.01 ({usd_profit})");
    }
}

async fn check_arb_on_block(block: BlockNumber, runup: u64) -> anyhow::Result<()> {
    let provider = Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    );

    let start_block = block - runup;
    let target_block = block;

    let state_monitor = StateMonitor::new(provider.clone());

    let storage = Storage::new().await?;

    let protocol_registry = Arc::new(
        ProtocolRegistry::new(provider.clone())
            .await?
            .with::<UniswapV2Protocol>()?
            .with::<UniswapV3Protocol>()?
            .with::<PancakeSwapV2Protocol>()?,
    );

    let pools = protocol_registry
        .get_cached_filtered_pools(&storage, start_block.into())
        .await?;

    let mut token_graph =
        TokenGraph::new(pools.clone(), 0.001, start_block.into(), provider.clone()).await?;

    let health_monitor = HealthMonitor::new(provider.clone());
    // let block: BlockId = block.into();

    for block in start_block..=target_block {
        let current_block = block;

        let catching_up = block < target_block;

        tracing::info!("block {block}, running_up = {catching_up}");

        let changes = state_monitor
            .get_state_changes(
                provider
                    .get_block(block.into(), BlockTransactionsKind::Hashes)
                    .await?
                    .unwrap()
                    .header,
            )
            .await;

        let affected_tokens = token_graph
            .apply_state(changes.changes, provider.clone(), block.into())
            .await;

        tracing::info!("affected tokens: {affected_tokens:#?}");

        if catching_up {
            continue;
        }

        health_monitor.check_health(changes.block_header.number, token_graph.pools.clone());

        let opportunities = token_graph
            .find_uncalculated_opportunities(affected_tokens)
            .await;
        tracing::info!("found {} opportunities", opportunities.len());

        let block: BlockId = block.into();

        for mut opportunity in opportunities {
            // tracing::info!("opportunity:");
            // tracing::info!("ROI: {}", calculate_roi(&mut opportunity, &mut evm));
            // tracing::info!("opportunity with 1.0 amount in:");
            // simulate_opportunity(
            //     &mut opportunity,
            //     0.703378,
            //     &protocol_registry,
            //     block,
            //     provider.clone(),
            // )
            // .await;

            let Some(x) = optimize_profit(&mut opportunity, block, provider.clone()).await else {
                continue;
            };
            // simulate_opportunity(
            //     &mut opportunity,
            //     x,
            //     &protocol_registry,
            //     block,
            //     provider.clone(),
            // )
            // .await;

            let profit =
                calculate_opportunity(&mut opportunity, x, block, provider.clone()).await - x;
            if profit <= 0.0 {
                continue;
            }
            let usd_profit = get_usd_value(
                &opportunity[0].token0,
                profit,
                &protocol_registry,
                block,
                provider.clone(),
            )
            .await;

            if usd_profit >= 0.01 {
                tracing::info!("optimized amount in:");
                simulate_opportunity(
                    &mut opportunity,
                    x,
                    &protocol_registry,
                    block,
                    provider.clone(),
                )
                .await;
                println!("");
            }

            // println!("");
        }
    }

    Ok(())
}

async fn calculate_opportunity<P: Provider + Clone>(
    opportunity: &mut [Step<P>],
    amount: f64,
    block: BlockId,
    provider: P,
) -> f64 {
    let token0 = opportunity[0].token0.clone();

    let mut amount = token0.to_token_amount(amount);

    for step in opportunity {
        let amount_out = step
            .pool
            .simulate_swap(step.token0.address, amount, block, provider.clone())
            .await;
        amount = amount_out;
    }

    token0.to_float_amount(amount)
}

async fn simulate_opportunity<P: Provider + std::fmt::Debug + Clone>(
    opportunity: &mut [Step<P>],
    start_amount: f64,
    registry: &ProtocolRegistry<P>,
    block: BlockId,
    provider: P,
) -> U256 {
    tracing::info!("opportunity:");
    let token_start_amount = opportunity[0].token0.to_token_amount(start_amount);
    let mut amount = token_start_amount;

    for step in &mut *opportunity {
        let amount_out = step
            .pool
            .simulate_swap(step.token0.address, amount, block, provider.clone())
            .await;
        tracing::info!(
            "{} ({}) -> {} ({}) on {}",
            step.token0,
            step.token0.to_float_amount(amount),
            step.token1,
            step.token1.to_float_amount(amount_out),
            step.pool.identifier(),
            // step.pool.address()
        );
        amount = amount_out;
    }

    if amount >= token_start_amount {
        let profit = opportunity[0]
            .token0
            .to_float_amount(amount - token_start_amount);

        let usd_profit = get_usd_value(
            &opportunity[0].token0,
            profit,
            registry,
            block,
            provider.clone(),
        )
        .await;

        tracing::info!("profit: {profit} (${usd_profit})");
    } else {
        tracing::info!("no profit");
    }

    amount
}

type Step<P> = OpportunityLeg<P>;

async fn optimize_profit<P: Provider + Clone>(
    opportunity: &mut [Step<P>],
    block: BlockId,
    provider: P,
) -> Option<f64> {
    let mut get_profit =
        async |x| calculate_opportunity(opportunity, x, block, provider.clone()).await - x;

    let mut lower_bound = 0.0;
    let mut upper_bound = 1000.0;

    let max_iter = 70;

    for _ in 0..max_iter {
        let middle = (lower_bound + upper_bound) / 2.0;

        let lower_profit = get_profit(lower_bound + (middle - lower_bound) / 2.0).await;
        let upper_profit = get_profit(middle + (upper_bound - middle) / 2.0).await;

        if lower_profit > upper_profit {
            upper_bound = middle;
        } else {
            lower_bound = middle;
        }
    }

    Some((lower_bound + upper_bound) / 2.0)
}

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");

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
