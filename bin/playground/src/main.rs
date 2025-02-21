use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U64, U256, address, map::AddressSet},
    providers::{Provider, ProviderCall, ProviderLayer, RootProvider, WsConnect},
    rpc::{client::NoParams, types::BlockTransactionsKind},
    transports::BoxTransport,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_defi_protocols_protocol::{Protocol, registry::ProtocolRegistry};
use plutus_evm::EVM;
use plutus_monitoring::StateMonitor;
use plutus_storage::{IdentifiedLiquidityPool, Storage};
use plutus_token_graph::{Step, TokenGraph};
use std::{sync::Arc, time::Instant};

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

    let provider = Arc::new(
        ProviderBuilder::new()
            .layer(FixedBlockLayer { block: 100.into() })
            .on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await?
                    .boxed(),
            ),
    );

    let block = provider.get_block_number().await?;
    tracing::info!("{block}");

    // let provider = Arc::new(
    //     ProviderBuilder::new().with_recommended_fillers().on_client(
    //         ClientBuilder::default()
    //             .ws(WsConnect::new(
    //                 "wss://arbitrum.gateway.tenderly.co/qHnrhFxjbqvYCPGxbVcJh".to_string(),
    //             ))
    //             // .ipc(dotenv!("IPC_PROVIDER").to_string().into())
    //             .await?
    //             .boxed(),
    //     ),
    // );

    // let block = 308382381;
    //
    // let storage = Storage::new().await?;
    //
    // let protocol_registry = ProtocolRegistry::new(provider.clone())
    //     .await?
    //     .with::<UniswapV2Protocol>()?
    //     .with::<UniswapV3Protocol>()?
    //     .with::<PancakeSwapV2Protocol>()?;
    //
    // let pools = protocol_registry
    //     .get_cached_filtered_pools(&storage, block.into())
    //     .await?;
    //
    // tracing::info!("pool count: {}", pools.len());
    //
    // let mut token_graph = TokenGraph::new(pools, 0.001, block, provider.clone()).await?;
    //
    // let targets = AddressSet::from_iter([
    //     WETH,
    //     USDT,
    //     address!("0x912CE59144191C1204E64559FE8253a0e49E6548"),
    // ]);
    //
    // let opportunities = token_graph.find_opportunities(targets).await;
    // tracing::info!("found {} opportunities", opportunities.len());
    //

    // let block = provider.get_block_number().await?;
    //
    // let mut evm = EVM::new_on_block(provider.clone(), block);
    //
    // let pool = address!("0x97bca422Ec0Ee4851F2110eA743C1cd0a14835a1");
    //
    // let now = Instant::now();
    // let a = provider
    //     .get_storage_at(pool, U256::ZERO)
    //     .block_id(block.into())
    //     .await?;
    // tracing::info!("provider: {:?}", now.elapsed());
    //
    // let now = Instant::now();
    // let a = provider
    //     .get_storage_at(pool, U256::ZERO)
    //     .block_id(block.into())
    //     .await?;
    // tracing::info!("provider: {:?}", now.elapsed());
    //
    // let now = Instant::now();
    // let a = evm.storage(pool, U256::ZERO);
    // tracing::info!("revm: {:?}", now.elapsed());
    //
    // let now = Instant::now();
    // let a = evm.storage(pool, U256::ZERO);
    // tracing::info!("revm: {:?}", now.elapsed());

    // for mut opportunity in opportunities {
    //     // tracing::info!("opportunity:");
    //     // tracing::info!("ROI: {}", calculate_roi(&mut opportunity, &mut evm));
    //     // tracing::info!("opportunity with 1.0 amount in:");
    //     // simulate_opportunity(&mut opportunity, &mut evm, 1.0, &protocol_registry);
    //
    //     let Some(x) = optimize_profit(&mut opportunity, &mut evm) else {
    //         continue;
    //     };
    //
    //     let profit = calculate_opportunity(&mut opportunity, &mut evm, x) - x;
    //     let usd_profit =
    //         get_usd_value(&opportunity[0].token0, profit, &mut evm, &protocol_registry);
    //
    //     if usd_profit >= 0.01 {
    //         tracing::info!("optimized amount in:");
    //         simulate_opportunity(&mut opportunity, &mut evm, x, &protocol_registry);
    //         println!("");
    //     }
    //
    //     // println!("");
    // }

    Ok(())
}

// async fn run_up() -> anyhow::Result<()> {
//     let provider = Arc::new(
//         ProviderBuilder::new().with_recommended_fillers().on_client(
//             ClientBuilder::default()
//                 .ws(WsConnect::new(
//                     "wss://arbitrum.gateway.tenderly.co/qHnrhFxjbqvYCPGxbVcJh".to_string(),
//                 ))
//                 // .ipc(dotenv!("IPC_PROVIDER").to_string().into())
//                 .await?
//                 .boxed(),
//         ),
//     );
//
//     let target_block = 307_349_041;
//     let start_block = 307_348_900;
//
//     let protocol_registry = ProtocolRegistry::new(provider.clone())
//         .await?
//         .with::<UniswapV2Protocol>()?
//         .with::<UniswapV3Protocol>()?;
//
//     let pools = vec![
//         IdentifiedLiquidityPool {
//             address: address!("A961F0473dA4864C5eD28e00FcC53a3AAb056c1b"),
//             protocol: "uniswap_v3".to_string(),
//         },
//         IdentifiedLiquidityPool {
//             address: address!("3dA7FE3f0eD5c8e82A6E5b046C635274912a5db0"),
//             protocol: "uniswap_v2".to_string(),
//         },
//         IdentifiedLiquidityPool {
//             address: address!("641C00A822e8b671738d32a431a4Fb6074E5c79d"),
//             protocol: "uniswap_v3".to_string(),
//         },
//     ];
//
//     let pools = protocol_registry
//         .create_pools_from_records(pools, start_block.into())
//         .await?;
//
//     let mut token_graph = TokenGraph::new(pools, 0.001, start_block, provider.clone()).await?;
//
//     let state_monitor = StateMonitor::new(provider.clone());
//
//     for block in start_block..=target_block {
//         tracing::info!("block {block}");
//         let mut evm = EVM::new_on_block(provider.clone(), block);
//
//         let header = provider
//             .get_block_by_number(block.into(), BlockTransactionsKind::Hashes)
//             .await?
//             .unwrap()
//             .header;
//
//         let change = state_monitor.get_state_changes(header).await;
//         token_graph
//             .apply_state(change.changes, provider.clone(), block.into(), &mut evm)
//             .await;
//
//         let targets = AddressSet::from_iter([]);
//         let opportunities = token_graph.find_opportunities(targets).await;
//         tracing::info!("found {} opportunities", opportunities.len());
//
//         for mut opportunity in opportunities {
//             let Some(x) = optimize_profit(&mut opportunity, &mut evm) else {
//                 continue;
//             };
//
//             simulate_opportunity(&mut opportunity, &mut evm, x, &protocol_registry);
//         }
//     }
//
//     Ok(())
// }
//
// fn calculate_roi<P: Provider>(opportunity: &mut [Step<P>], evm: &mut EVM<P>) -> f64 {
//     let token0 = opportunity[0].token0.clone();
//
//     let start_amount = token0.to_token_amount(1.0);
//     let mut amount = start_amount;
//
//     for step in opportunity {
//         amount = step.pool.simulate_swap(step.token0.address, amount, evm);
//     }
//
//     f64::from(amount) / f64::from(start_amount)
// }
//
// fn calculate_opportunity<P: Provider>(
//     opportunity: &mut [Step<P>],
//     evm: &mut EVM<P>,
//     amount: f64,
// ) -> f64 {
//     let token0 = opportunity[0].token0.clone();
//
//     let mut amount = token0.to_token_amount(amount);
//
//     for step in opportunity {
//         let amount_out = step.pool.simulate_swap(step.token0.address, amount, evm);
//         amount = amount_out;
//     }
//
//     token0.to_float_amount(amount)
// }
//
// fn simulate_opportunity<P: Provider + std::fmt::Debug>(
//     opportunity: &mut [Step<P>],
//     evm: &mut EVM<P>,
//     start_amount: f64,
//     registry: &ProtocolRegistry<P>,
// ) -> U256 {
//     tracing::info!("opportunity:");
//     let token_start_amount = opportunity[0].token0.to_token_amount(start_amount);
//     let mut amount = token_start_amount;
//
//     for step in &mut *opportunity {
//         let amount_out = step.pool.simulate_swap(step.token0.address, amount, evm);
//         tracing::info!(
//             "{} ({}) -> {} ({}) on {}",
//             step.token0,
//             step.token0.to_float_amount(amount),
//             step.token1,
//             step.token1.to_float_amount(amount_out),
//             step.pool.identifier(),
//             // step.pool.address()
//         );
//         amount = amount_out;
//     }
//
//     if amount >= token_start_amount {
//         let profit = opportunity[0]
//             .token0
//             .to_float_amount(amount - token_start_amount);
//
//         let usd_profit = get_usd_value(&opportunity[0].token0, profit, evm, registry);
//
//         tracing::info!("profit: {profit} (${usd_profit})");
//     } else {
//         tracing::info!("no profit");
//     }
//
//     amount
// }
//
// fn optimize_profit<P: Provider>(opportunity: &mut [Step<P>], evm: &mut EVM<P>) -> Option<f64> {
//     let mut get_profit = |x| calculate_opportunity(opportunity, evm, x) - x;
//
//     let mut lower_bound = 0.0;
//     let mut upper_bound = 1000.0;
//
//     let max_iter = 50;
//
//     for _ in 0..max_iter {
//         let middle = (lower_bound + upper_bound) / 2.0;
//
//         let lower_profit = get_profit(lower_bound + (middle - lower_bound) / 2.0);
//         let upper_profit = get_profit(middle + (upper_bound - middle) / 2.0);
//
//         if lower_profit > upper_profit {
//             upper_bound = middle;
//         } else {
//             lower_bound = middle;
//         }
//     }
//
//     Some((lower_bound + upper_bound) / 2.0)
// }
//
// pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
//
// fn get_usd_value<P: Provider + std::fmt::Debug>(
//     token: &ERC20,
//     amount: f64,
//     evm: &mut EVM<P>,
//     registry: &ProtocolRegistry<P>,
// ) -> f64 {
//     let usdt = ERC20::new(USDT, evm).unwrap();
//     let usdc = ERC20::new(USDC, evm).unwrap();
//     let weth = ERC20::new(WETH, evm).unwrap();
//
//     let token_amount = token.to_token_amount(amount);
//
//     let usdt_value = usdt.to_float_amount(
//         registry
//             .get_token_value(token.address, USDT, token_amount, evm)
//             .unwrap(),
//     );
//
//     let usdc_value = usdc.to_float_amount(
//         registry
//             .get_token_value(token.address, usdc.address, token_amount, evm)
//             .unwrap(),
//     );
//
//     let weth_value = weth.to_float_amount(
//         registry
//             .get_token_value(token.address, weth.address, token_amount, evm)
//             .unwrap(),
//     ) * 2666.39;
//
//     usdt_value.max(usdc_value).max(weth_value)
// }
