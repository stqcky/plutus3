use alloy::{primitives::U256, providers::Provider, uint};
use plutus_evm::{EVM, mapping::SolidityMapping, storage::SmartContractStorage};
use uniswap_v3_math::{bit_math, error::UniswapV3MathError, tick_bitmap::position};

const U256_1: U256 = uint!(1U256);

pub fn next_initialized_tick_within_one_word<P: Provider>(
    tick_bitmap: &mut SolidityMapping<i16, U256, 6>,
    tick: i32,
    tick_spacing: i32,
    lte: bool,
    evm: &mut EVM<P>,
    storage: &mut SmartContractStorage,
) -> Result<(i32, bool), UniswapV3MathError> {
    let compressed = if tick < 0 && tick % tick_spacing != 0 {
        (tick / tick_spacing) - 1
    } else {
        tick / tick_spacing
    };

    if lte {
        let (word_pos, bit_pos) = position(compressed);

        let mask = (U256_1 << bit_pos) - U256_1 + (U256_1 << bit_pos);

        let masked = tick_bitmap.get(storage, &word_pos, evm) & mask;

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

        let masked = tick_bitmap.get(storage, &word_pos, evm) & mask;

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
