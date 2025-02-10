use std::sync::Arc;

use alloy::{
    primitives::BlockNumber,
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
};
use dotenvy_macro::dotenv;
use plutus_blockchain::Blockchain;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::{v2::UniswapV2Protocol, v3::UniswapV3Protocol};
use plutus_storage::Storage;

fn init_tracing() {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().without_time().compact())
        .init();
}

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

    let blockchain = Blockchain::new(provider.get_chain_id().await?);
    let storage = Storage::new().await?;

    let protocol_registry = ProtocolRegistry::new(blockchain.chain_id, provider.clone())
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?;

    discover_and_store_pools(
        &protocol_registry,
        &storage,
        provider.get_block_number().await?,
    )
    .await?;

    // tracing::info!("{discovered:#?}");

    Ok(())
}

async fn discover_and_store_pools<P: Clone>(
    registry: &ProtocolRegistry<P>,
    storage: &Storage,
    block_number: BlockNumber,
) -> anyhow::Result<()> {
    let last_discovered_blocks = storage
        .get_last_discovered_blocks(&registry.protocol_identifiers())
        .await?;

    let discovered = registry
        .discover(&last_discovered_blocks, block_number)
        .await?;

    storage.insert_pools(&discovered).await?;

    for (protocol, _) in discovered {
        storage
            .set_last_discovered_block(block_number, protocol)
            .await?;
    }

    Ok(())
}
