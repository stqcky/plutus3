use alloy::{
    network::{Ethereum, EthereumWallet},
    providers::{
        Identity, ProviderBuilder, RootProvider,
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
    },
    rpc::client::ClientBuilder,
    transports::{
        BoxTransport,
        http::{Client, Http},
    },
};

const ARBITRUM_SEQUENCER: &str = "https://arb1-sequencer.arbitrum.io/rpc";

type SequencerProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;

pub fn create_provider(wallet: EthereumWallet) -> SequencerProvider {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_client(ClientBuilder::default().http(ARBITRUM_SEQUENCER.parse().unwrap()));

    provider
}
