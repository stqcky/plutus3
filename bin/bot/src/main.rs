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

    let protocol_registry = ProtocolRegistry::new(provider.clone())
        .await?
        .with::<UniswapV2Protocol>()?
        .with::<UniswapV3Protocol>()?;

    protocol_registry
        .discover_and_store(provider.get_block_number().await?, &storage)
        .await?;

    Ok(())
}
