use alloy::{
    eips::BlockId,
    primitives::{U256, address},
    providers::Provider,
};
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use std::{sync::Arc, time::Instant};

use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_uniswap::v3::pool::UniswapV3Pool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();

    let provider = Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    );

    let pool = UniswapV3Pool::new_with_provider(
        address!("0x44c40a6544f29f331720E989Cd2724306b21c0d0"),
        provider.clone(),
        BlockId::latest(),
    )
    .await?;

    let now = Instant::now();
    provider.get_storage_at(pool.address, U256::ZERO).await?;
    tracing::info!("{:?}", now.elapsed());

    tracing::info!("1");
    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    let now = Instant::now();
    pool.simulate_swap(
        pool.token0.address,
        pool.token0.to_token_amount(1.0),
        BlockId::latest(),
        provider.clone(),
    )
    .await;
    tracing::info!("{:?}", now.elapsed());

    Ok(())
}
