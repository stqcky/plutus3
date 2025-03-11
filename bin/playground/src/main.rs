use plutus_geth::db::GethDB;
use std::{error::Error, sync::Arc, time::Instant};

use alloy::{
    primitives::{StorageValue, U256, address, aliases::U24},
    providers::{Provider, ProviderBuilder},
    rpc::client::{BatchRequest, ClientBuilder},
};
use dotenvy_macro::dotenv;

struct Amogus {
    db: GethDB,
}

#[derive(Debug)]
struct AmogusError {}

impl Error for AmogusError {}

impl std::fmt::Display for AmogusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error")
    }
}

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

    let block = provider.get_block_number().await?;

    let addr = address!("0xC24f7d8E51A64dc1238880BD00bb961D54cbeb29");

    let block = format!("0x{}", hex::encode(block.to_be_bytes()));

    let now = Instant::now();

    let mut batch = BatchRequest::new(provider.client());
    let mut batch2 = BatchRequest::new(provider.client());

    tracing::info!("{} {}", u16::MAX, U24::MAX);

    let mut calls = vec![];
    // let mut calls2 = vec![];

    for i in 0..1000 {
        calls.push(
            batch
                .add_call::<_, StorageValue>("eth_getStorageAt", &(addr, U256::from(i), "latest"))?
                .into_future(),
        );

        // calls2.push(
        //     batch2
        //         .add_call::<_, StorageValue>("eth_getStorageAt", &(addr, U256::from(i), "latest"))?
        //         .into_future(),
        // );
    }

    let now2 = Instant::now();
    batch.send().await?;
    // batch.send().await?;
    tracing::info!("batch.send {:?}", now2.elapsed());

    let now3 = Instant::now();
    let calls = futures::future::join_all(calls).await;
    // let calls2 = futures::future::join_all(calls2).await;
    tracing::info!("calls {:?}", now3.elapsed());

    // let mut calls = vec![];

    // futures::future::join_all(vec![provider.get_storage_at(addr, U256::from(0))]);
    // futures::future::join_all(calls).await;
    tracing::info!("total {:?}", now.elapsed());

    // let db = GethDB::new(dotenv!("CLIENT_DB"))?;
    // let header = db.get_block_header()?;

    // let amogus = Amogus { db };

    // let state_root = header.state_root;

    // tracing::info!("{}", state_root.to_string());

    // let triedb = TrieDBBuilder::<EIP1186Layout<KeccakHasher>>::new(&amogus, &state_root).build();
    //
    // triedb.get(&[1])?.unwrap();

    // let trie = EthTrie::new(amogus);
    // let a = trie.get(header.state_root.as_slice());
    // tracing::info!("{a:#?}");

    // let mut readopts = ReadOptions::default();
    // readopts.set_read_tier(ReadTier::All);
    // readopts.set_verify_checksums(false);
    //
    // db.db.get_opt(header.state_root, &readopts)?.unwrap();
    // let state_id = snapshot.get(header.state_root)?.unwrap();

    // tracing::info!("{state_id:?}");

    Ok(())
}
