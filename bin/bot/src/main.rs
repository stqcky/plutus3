use std::{sync::Arc, time::Instant};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
};
use dotenvy_macro::dotenv;
use plutus_blockchain::Blockchain;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_evm::EVM;
use plutus_storage::Storage;

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

    let blockchain = Blockchain::new(provider.get_chain_id().await?);
    let storage = Storage::new().await?;

    let protocol_registry = ProtocolRegistry::new(provider.clone())
        .await?
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?;

    protocol_registry
        .discover_and_store(block_number, &storage)
        .await?;

    let mut evm = EVM::new(provider.clone(), block_number.into()).await?;

    let pools = if UPDATE_CACHE {
        tracing::info!("updating cache");

        let now = Instant::now();

        let filtered = protocol_registry
            .get_filtered_pools(&storage, &mut evm, 5_000.0)
            .await?;

        protocol_registry
            .cache_filtered_pools(&storage, &filtered)
            .await?;

        tracing::info!("filtered in {:?}", now.elapsed());

        filtered
    } else {
        protocol_registry
            .get_cached_filtered_pools(&storage, &mut evm)
            .await?
    };

    Ok(())
}
