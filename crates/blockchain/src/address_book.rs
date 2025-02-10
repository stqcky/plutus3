use alloy::primitives::{Address, ChainId, address};

#[derive(Clone, Copy)]
pub struct AddressBook {
    pub usdt: Address,
    pub usdc: Address,
    pub weth: Address,
    pub wbtc: Address,
    pub dai: Address,
}

pub const ARBITRUM: AddressBook = AddressBook {
    usdt: address!("fd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"),
    usdc: address!("af88d065e77c8cc2239327c5edb3a432268e5831"),
    weth: address!("82af49447d8a07e3bd95bd0d56f35241523fbab1"),
    wbtc: address!("2f2a2543b76a4166549f7aab2e75bef0aefc5b0f"),
    dai: address!("da10009cbd5d07dd0cecc66161fc93d7c9000da1"),
};

pub fn get_address_book_for_chain(chain_id: ChainId) -> AddressBook {
    match chain_id {
        42161 => ARBITRUM,
        _ => panic!("unknown chain id"),
    }
}
