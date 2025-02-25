use alloy::providers::Provider;
use plutus_defi_protocols_pancakeswap::v2::PancakeSwapV2Protocol;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_monitoring::health::HealthMonitor;
use plutus_storage::Storage;

use crate::utils::create_provider;

// #[tokio::test(flavor = "multi_thread")]
async fn protocol_registry_pools() -> anyhow::Result<()> {
    let provider = create_provider().await?;

    let block = provider.get_block_number().await?;
    let storage = Storage::new().await?;
    let protocol_registry = ProtocolRegistry::new(provider.clone())
        .await?
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?
        .with::<PancakeSwapV2Protocol>()?;

    let pools = protocol_registry
        .get_cached_filtered_pools(&storage, block.into())
        .await?;

    let health_monitor = HealthMonitor::new(provider.clone());
    health_monitor.check_health(block, pools).await??;

    Ok(())
}
