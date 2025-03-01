use std::sync::Arc;

use alloy::{primitives::BlockNumber, providers::Provider};
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use tokio::{sync::Semaphore, task::JoinHandle};

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

    pub fn check_health(
        &self,
        block: BlockNumber,
        pools: Vec<Arc<dyn LiquidityPool<P>>>,
    ) -> JoinHandle<anyhow::Result<()>> {
        let provider = self.provider.clone();
        let semaphore = self.semaphore.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();

            // let now = Instant::now();
            for pool in pools {
                pool.verify_health(provider.clone().into(), block)
                    .await
                    .inspect_err(|err| tracing::error!("health check failed: {err}"))?;
            }

            // tracing::warn!("health check in {:?}", now.elapsed());
            Ok(())
        })
    }
}
