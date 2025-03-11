use alloy::{primitives::BlockNumber, providers::Provider};
use plutus_defi_protocols_protocol::pool::LiquidityPool;

pub struct HealthMonitor<P> {
    provider: P,
}

impl<P: Provider + Clone + 'static> HealthMonitor<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub async fn check_health(
        &self,
        block: BlockNumber,
        pools: &[Box<dyn LiquidityPool<P>>],
    ) -> anyhow::Result<()> {
        let provider = self.provider.clone();

        for pool in pools {
            pool.verify_health(provider.clone().into(), block)
                .await
                .inspect_err(|err| tracing::error!("health check failed: {err}"))?;
        }

        Ok(())
    }
}
