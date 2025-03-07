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

    println!("{}", i128::MAX);

    Ok(())
}
