use std::{sync::Arc, time::Instant};

use alloy::{
    primitives::{Address, U256, address},
    providers::{Provider, ProviderBuilder},
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
use tokio::sync::mpsc;

fn init_tracing() {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();
}

const UPDATE_CACHE: bool = true;

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

    let block_number = provider.get_block_number().await?;

    tracing::info!("startup block: {block_number}");

    let state_monitor = StateMonitor::new(provider.clone());

    let (state_tx, mut state_rx) = mpsc::channel::<StateChange>(1024);

    state_monitor.subscribe_blocks(state_tx).await?;

    // let blockchain = Blockchain::new(provider.get_chain_id().await?);
    let storage = Storage::new().await?;

    let protocol_registry = ProtocolRegistry::new(provider.clone())
        .await?
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?
        .with::<PancakeSwapV2Protocol>()?;

    protocol_registry
        .discover_and_store(block_number, &storage)
        .await?;

    let pools = if UPDATE_CACHE {
        tracing::info!("updating cache");

        let now = Instant::now();

        let filtered = protocol_registry
            .get_filtered_pools(&storage, 2_000.0, block_number.into())
            .await?;

        protocol_registry
            .cache_filtered_pools(&storage, &filtered)
            .await?;

        tracing::info!("filtered in {:?}", now.elapsed());

        filtered
    } else {
        protocol_registry
            .get_cached_filtered_pools(&storage, block_number.into())
            .await?
    };

    tracing::info!("pool count: {}", pools.len());

    let mut evm = EVM::new_on_block(provider.clone(), block_number);

    tracing::info!("creating token graph");
    let mut token_graph = TokenGraph::new(pools, 1.0, &mut evm);

    let health_monitor = HealthMonitor::new(provider.clone());

    // let mut evm = EVM::new(provider.clone(), block_number);

    // while let Some(block) = blocks.next().await {
    while let Some(state_change) = state_rx.recv().await {
        let catching_up = if state_change.block_header.number == provider.get_block_number().await?
        {
            false
        } else {
            true
        };

        tracing::info!(
            "block {}{}",
            state_change.block_header.number,
            if catching_up { ", catching up" } else { "" }
        );

        let mut evm = EVM::new_on_block(provider.clone(), state_change.block_header.number);

        if catching_up {
            token_graph.apply_state(state_change.changes, &mut evm);
            health_monitor
                .check_health(state_change.block_header.number, token_graph.pools.clone());

            continue;
        }

        token_graph.apply_state(state_change.changes, &mut evm);
        health_monitor.check_health(state_change.block_header.number, token_graph.pools.clone());

        for mut opportunity in token_graph.find_opportunities().await {
            // simulate_opportunity(&mut opportunity, &mut evm, 1.0, &protocol_registry);
            let Some(x) = optimize_profit(&mut opportunity, &mut evm) else {
                continue;
            };

            // let profit = calculate_opportunity(&mut opportunity, &mut evm, x) - x;
            // let usd_profit =
            //     get_usd_value(&opportunity[0].token0, profit, &mut evm, &protocol_registry);
            //
            // if usd_profit >= 0.01 {
            simulate_opportunity(&mut opportunity, &mut evm, x, &protocol_registry);
            println!("");
            // }
        }
    }

    Ok(())
}

fn calculate_opportunity<P: Provider>(
    opportunity: &mut [Step<P>],
    evm: &mut EVM<P>,
    amount: f64,
) -> f64 {
    let token0 = opportunity[0].token0.clone();

    let mut amount = token0.to_token_amount(amount);

    for step in opportunity {
        let amount_out = step.pool.simulate_swap(step.token0.address, amount, evm);
        amount = amount_out;
    }

    token0.to_float_amount(amount)
}

fn simulate_opportunity<P: Provider + std::fmt::Debug>(
    opportunity: &mut [Step<P>],
    evm: &mut EVM<P>,
    start_amount: f64,
    registry: &ProtocolRegistry<P>,
) -> U256 {
    tracing::info!("opportunity:");
    let token_start_amount = opportunity[0].token0.to_token_amount(start_amount);
    let mut amount = token_start_amount;

    for step in &mut *opportunity {
        let amount_out = step.pool.simulate_swap(step.token0.address, amount, evm);
        tracing::info!(
            "{} ({}) -> {} ({}) on {}",
            step.token0,
            step.token0.to_float_amount(amount),
            step.token1,
            step.token1.to_float_amount(amount_out),
            step.pool.identifier()
        );
        amount = amount_out;
    }

    if amount >= token_start_amount {
        let profit = opportunity[0]
            .token0
            .to_float_amount(amount - token_start_amount);

        let usd_profit = get_usd_value(&opportunity[0].token0, profit, evm, registry);

        tracing::info!("profit: {profit} (${usd_profit})");
    } else {
        tracing::info!("no profit");
    }

    amount
}

fn optimize_profit<P: Provider>(opportunity: &mut [Step<P>], evm: &mut EVM<P>) -> Option<f64> {
    let max_iter = 40;

    let mut g = |x| calculate_opportunity(opportunity, evm, x) - x;

    let mut a = 0.0;
    let mut c = 10.0;

    while g(c) > 0.0 {
        c *= 2.0;
    }

    let mut best_x = a;
    let mut best_profit = g(a);
    for _ in 0..max_iter {
        if c < a {
            break;
        }

        let m1 = a + (c - a) / 3.0;
        let m2 = c - (c - a) / 3.0;

        let g1 = g(m1);
        let g2 = g(m2);

        if g1 > best_profit {
            best_profit = g1;
            best_x = m1;
        }
        if g2 > best_profit {
            best_profit = g2;
            best_x = m2;
        }

        if g1 > g2 {
            c = m2;
        } else {
            a = m1;
        }
    }

    (best_profit > 0.0).then_some(best_x)
}

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

fn get_usd_value<P: Provider + std::fmt::Debug>(
    token: &ERC20,
    amount: f64,
    evm: &mut EVM<P>,
    registry: &ProtocolRegistry<P>,
) -> f64 {
    let usdt = ERC20::new(USDT, evm).unwrap();
    let usdc = ERC20::new(USDC, evm).unwrap();
    let weth = ERC20::new(WETH, evm).unwrap();

    let token_amount = token.to_token_amount(amount);

    let usdt_value = usdt.to_float_amount(
        registry
            .get_token_value(token.address, USDT, token_amount, evm)
            .unwrap(),
    );

    let usdc_value = usdc.to_float_amount(
        registry
            .get_token_value(token.address, usdc.address, token_amount, evm)
            .unwrap(),
    );

    let weth_value = weth.to_float_amount(
        registry
            .get_token_value(token.address, weth.address, token_amount, evm)
            .unwrap(),
    ) * 2666.39;

    usdt_value.max(usdc_value).max(weth_value)
}
