use std::sync::Arc;

use alloy::{
    network::Ethereum,
    providers::{
        Identity, ProviderBuilder, RootProvider,
        fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    },
    rpc::client::ClientBuilder,
    transports::BoxTransport,
};
use dotenvy_macro::dotenv;

pub async fn create_provider() -> anyhow::Result<
    Arc<
        FillProvider<
            JoinFill<
                Identity,
                JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
            >,
            RootProvider<BoxTransport>,
            BoxTransport,
            Ethereum,
        >,
    >,
> {
    Ok(Arc::new(
        ProviderBuilder::new().with_recommended_fillers().on_client(
            ClientBuilder::default()
                .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                .await?
                .boxed(),
        ),
    ))
}
