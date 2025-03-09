use ethereum_triedb::{EIP1186Layout, keccak::KeccakHasher};
use hash_db::{AsHashDB, HashDB, HashDBRef, Hasher};
use parity_db::Db;
use plutus_geth::db::GethDB;
use primitive_types::H256;
use rocksdb::{DBAccess, ReadOptions, ReadTier};
use std::{
    error::Error,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use trie_db::{Trie, TrieDB, TrieDBBuilder};

use alloy::{
    primitives::{Address, B256, StorageKey, address},
    providers::{Provider, ProviderBuilder},
    rpc::{
        client::{ClientBuilder, NoParams},
        types::BlockTransactionsKind,
    },
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

impl HashDBRef<KeccakHasher, Vec<u8>> for Amogus {
    fn get(&self, key: &H256, prefix: hash_db::Prefix) -> Option<Vec<u8>> {
        tracing::info!("{key:?} {prefix:?}");
        todo!()
    }

    fn contains(&self, key: &H256, prefix: hash_db::Prefix) -> bool {
        todo!()
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

    // let now = Instant::now();
    // provider
    //     .get_storage_at(
    //         address!("0xC24f7d8E51A64dc1238880BD00bb961D54cbeb29"),
    //         U256::ZERO,
    //     )
    //     .await?;
    // tracing::info!("{:?}", now.elapsed());

    // let db = GethDB::new(dotenv!("CLIENT_DB"))?;
    // let snapshot = db.db.snapshot();
    //
    // let now = Instant::now();
    // let header = db.get_block_header()?;
    // tracing::info!("{:?}", now.elapsed());
    // tracing::info!("{header:#?}");

    // let amogus = Amogus { db };

    // let state_root = header.state_root;

    // tracing::info!("{}", state_root.to_string());

    let block_number = provider.get_block_number().await? - 5;
    let block = provider
        .get_block_by_number((block_number - 5).into(), BlockTransactionsKind::Hashes)
        .await?
        .unwrap();
    let tx_index = block.transactions.len().saturating_sub(1);

    let contract = address!("0x94AA7b3828BaA9236D18F0e6c9915460340fA1a0");

    let block = format!("0x{block_number:x}");
    tracing::info!("{}", block);

    let now = Instant::now();
    let a: String = provider
        .raw_request(
            "debug_storageRangeAt".into(),
            (
                block,
                0,
                "0x94AA7b3828BaA9236D18F0e6c9915460340fA1a0",
                "0x0000000000000000000000000000000000000000000000000000000000000000",
                10,
            ),
        )
        .await?;

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
