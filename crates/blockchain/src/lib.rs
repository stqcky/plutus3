pub mod address_book;

use address_book::{AddressBook, get_address_book_for_chain};
use alloy::primitives::ChainId;

pub struct Blockchain {
    chain_id: ChainId,
    address_book: AddressBook,
}

impl Blockchain {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            address_book: get_address_book_for_chain(chain_id),
        }
    }
}
