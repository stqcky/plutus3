use std::collections::HashMap;

use alloy::primitives::{I256, Keccak256, Signed, U256, Uint, aliases::I24};

#[tokio::main]
async fn main() {
    let value = 0u64;
    let slot = U256::from_limbs([value, 0, 0, 0]);

    assert_eq!(U256::from(value), slot);
}
