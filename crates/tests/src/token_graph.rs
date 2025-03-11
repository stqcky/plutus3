use alloy::{providers::Provider, rpc::types::state};
use futures::future;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_monitoring::{StateChange, StateMonitor, health::HealthMonitor};
use plutus_storage::Storage;
use plutus_token_graph::TokenGraph;
use tokio::sync::mpsc;

use crate::utils::create_provider;

#[tokio::test(flavor = "multi_thread")]
async fn token_graph_state_validity_realtime() -> anyhow::Result<()> {
    const CHECK_BLOCKS: u64 = 100;

    let provider = create_provider().await?;

    let start_block = provider.get_block_number().await?;

    println!("start block {start_block}");

    let state_monitor = StateMonitor::new(provider.clone());
    let (state_tx, mut state_rx) = mpsc::channel::<StateChange>(1024);
    state_monitor.subscribe_blocks(state_tx).await?;

    let storage = Storage::new().await?;
    let protocol_registry = ProtocolRegistry::new(provider.clone())
        .await?
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?
        .with::<PancakeSwapV2Protocol>()?;

    let pools = protocol_registry
        .get_cached_filtered_pools(&storage, start_block.into())
        .await?;

    let mut token_graph =
        TokenGraph::new(pools, 0.001, start_block.into(), provider.clone()).await?;

    let health_monitor = HealthMonitor::new(provider.clone());

    health_monitor
        .check_health(start_block, token_graph.pools.clone().to_vec())
        .await??;

    let mut blocks_checked = 0;

    while let Some(state_change) = state_rx.recv().await {
        let block = state_change.block_header.number;
        assert_eq!(block, start_block + blocks_checked + 1, "not sequential");

        println!("checking {block}");

        token_graph
            .apply_state(state_change.changes, provider.clone(), block.into())
            .await;

        health_monitor
            .check_health(block, token_graph.pools.clone().to_vec())
            .await??;

        blocks_checked += 1;

        if blocks_checked == CHECK_BLOCKS {
            break;
        }
    }

    Ok(())
}
