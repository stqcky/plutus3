use std::sync::Arc;

use alloy::{
    primitives::BlockNumber,
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::client::ClientBuilder,
};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_protocol::registry::ProtocolRegistry;
use plutus_defi_protocols_uniswap::v2::UniswapV2Protocol;
use plutus_storage::Storage;

const ARBITRUM_ONE_GENESIS_BLOCK_NUMBER: BlockNumber = 22_207_817;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // let provider = Arc::new(
    //     ProviderBuilder::new().with_recommended_fillers().on_client(
    //         ClientBuilder::default()
    //             .ws(WsConnect::new(dotenv!("ARBITRUM_LEGACY_WS_PROVIDER")))
    //             .await?
    //             .boxed(),
    //     ),
    // );
    //
    // let storage = Storage::new().await?;
    //
    // let protocol_registry = ProtocolRegistry::new(provider.get_chain_id().await?, provider.clone())
    //     .with::<UniswapV2Protocol>()?;
    // // .with::<UniswapV3Protocol>()?;
    //
    // let discovered_blocks = storage
    //     .get_last_discovered_blocks(&protocol_registry.protocol_identifiers())
    //     .await?;
    //
    // let discovered = protocol_registry
    //     .discover(&discovered_blocks, ARBITRUM_ONE_GENESIS_BLOCK_NUMBER)
    //     .await?;
    //
    // println!("{discovered:#?}");

    Ok(())
}
