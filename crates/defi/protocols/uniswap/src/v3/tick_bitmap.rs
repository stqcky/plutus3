use alloy::{eips::BlockId, primitives::U256, providers::Provider, uint};
use plutus_evm::{mapping::SolidityMapping, storage::SmartContractStorage};
use uniswap_v3_math::{bit_math, tick_bitmap::position};

const U256_1: U256 = uint!(1U256);

pub async fn next_initialized_tick_within_one_word<P: Provider, const SLOT: u128>(
    tick_bitmap: &SolidityMapping<i16, U256, SLOT>,
    tick: i32,
    tick_spacing: i32,
    lte: bool,
    storage: &SmartContractStorage,
    block: BlockId,
    provider: P,
) -> anyhow::Result<(i32, bool)> {
    let compressed = if tick < 0 && tick % tick_spacing != 0 {
        (tick / tick_spacing) - 1
    } else {
        tick / tick_spacing
    };

    if lte {
        let (word_pos, bit_pos) = position(compressed);

        let mask = (U256_1 << bit_pos) - U256_1 + (U256_1 << bit_pos);

        let masked = tick_bitmap.get(storage, &word_pos, block, provider).await? & mask;

        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed
                - (bit_pos
                    .overflowing_sub(bit_math::most_significant_bit(masked)?)
                    .0) as i32)
                * tick_spacing
        } else {
            (compressed - bit_pos as i32) * tick_spacing
        };

        Ok((next, initialized))
    } else {
        let (word_pos, bit_pos) = position(compressed + 1);

        let mask = !((U256_1 << bit_pos) - U256_1);

        let masked = tick_bitmap.get(storage, &word_pos, block, provider).await? & mask;

        let initialized = !masked.is_zero();

        let next = if initialized {
            (compressed
                + 1
                + (bit_math::least_significant_bit(masked)?
                    .overflowing_sub(bit_pos)
                    .0) as i32)
                * tick_spacing
        } else {
            (compressed + 1 + ((0xFF - bit_pos) as i32)) * tick_spacing
        };

        Ok((next, initialized))
    }
}
