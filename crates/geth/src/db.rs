use alloy::{
    consensus::Header,
    primitives::{B256, BlockNumber},
    rlp::Decodable,
};
use rocksdb::{DB, Options};

const HEADER_PREFIX: &[u8] = b"h";
const HEADER_NUMBER_PREFIX: &[u8] = b"H";
const HEAD_BLOCK_KEY: &[u8] = b"LastBlock";

pub struct GethDB {
    pub db: DB,
}

impl GethDB {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let mut options = Options::default();
        options.set_max_open_files(10);
        options.set_unordered_write(true);
        options.set_max_subcompactions(16);

        Ok(Self {
            db: DB::open_for_read_only(&options, path, true)?,
        })
    }

    pub fn get_block_hash(&self) -> anyhow::Result<B256> {
        Ok(B256::from_slice(&self.db.get(HEAD_BLOCK_KEY)?.unwrap()))
    }

    pub fn get_block_number(&self) -> anyhow::Result<BlockNumber> {
        let block_hash = self.get_block_hash()?;
        let block_number_key = [HEADER_NUMBER_PREFIX, block_hash.as_slice()].concat();
        let block_number = self.db.get(block_number_key)?.unwrap();

        Ok(BlockNumber::from_le_bytes(block_number.try_into().unwrap()))
    }

    pub fn get_block_header(&self) -> anyhow::Result<Header> {
        let block_hash = self.get_block_hash()?;
        let block_number = self.get_block_number()?;

        let header_key = [
            HEADER_PREFIX,
            &block_number.to_le_bytes(),
            block_hash.as_slice(),
        ]
        .concat();

        let header_rlp = self.db.get(header_key)?.unwrap();

        Ok(Header::decode(&mut header_rlp.as_slice())?)
    }
}

#[cfg(test)]
mod tests {
    use dotenvy_macro::dotenv;

    use super::*;

    #[test]
    fn it_works() -> anyhow::Result<()> {
        let db = GethDB::new(dotenv!("CLIENT_DB")).expect("it connects to database");

        db.get_block_hash().expect("it gets block hash");
        db.get_block_number().expect("it gets block number");
        db.get_block_header().expect("it gets block header");

        Ok(())
    }
}
