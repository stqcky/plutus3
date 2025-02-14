use std::{sync::Arc, time::Instant};

use alloy::{primitives::BlockNumber, providers::Provider};
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use tokio::sync::Semaphore;

pub struct HealthMonitor<P> {
    provider: P,
    semaphore: Arc<Semaphore>,
}

impl<P: Provider + Clone + 'static> HealthMonitor<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            semaphore: Arc::new(Semaphore::new(5)),
        }
    }

    pub fn check_health(&self, block: BlockNumber, pools: Vec<Box<dyn LiquidityPool<P>>>) {
        let provider = self.provider.clone();
        let semaphore = self.semaphore.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();

            let now = Instant::now();
            for pool in pools {
                if let Err(err) = pool.verify_health(provider.clone().into(), block).await {
                    tracing::error!("health check failed: {err}");
                }
            }

            tracing::warn!("health check in {:?}", now.elapsed());
        });
    }
}
