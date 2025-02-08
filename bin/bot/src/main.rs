use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Context;
use dotenvy_macro::dotenv;
use plutus_blockchain::Blockchain;
use plutus_storage::Storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_ipc(dotenv!("IPC_PROVIDER").to_string().into())
        .await
        .context("failed to initialize provider")?;

    let blockchain = Blockchain::new(provider.get_chain_id().await?);
    let storage = Storage::new().await?;

    Ok(())
}
