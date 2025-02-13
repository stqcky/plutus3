use std::sync::Arc;

use alloy::{primitives::BlockNumber, providers::Provider};
use plutus_defi_protocols_protocol::pool::LiquidityPool;

pub struct HealthMonitor<P> {
    provider: P,
}

impl<P: Provider + Clone + 'static> HealthMonitor<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn check_health(&self, block: BlockNumber, pools: Vec<Box<dyn LiquidityPool<P>>>) {
        let provider = self.provider.clone();

        tokio::spawn(async move {
            for pool in pools {
                if let Err(err) = pool.verify_health(provider.clone().into(), block).await {
                    tracing::error!("health check failed: {err}");
                }
            }
        });
    }
}
