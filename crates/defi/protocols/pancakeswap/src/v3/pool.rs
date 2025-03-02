use std::sync::Arc;

use IPancakeSwapV3Pool::{IPancakeSwapV3PoolInstance, slot0Return};
use alloy::{
    eips::BlockId,
    hex,
    primitives::{
        Address, BlockNumber, I256, U16, U32, U160, U256,
        aliases::{I24, U24},
    },
    providers::Provider,
    sol,
    sol_types::{SolCall as _, SolType},
    uint,
};
use anyhow::bail;
use async_trait::async_trait;
use parking_lot::RwLock;
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::{SwapDataPayload, pool::LiquidityPool};
use plutus_defi_protocols_uniswap::v3::{
    pool::{Q128, StepComputations, SwapState, TickInfo},
    tick_bitmap::next_initialized_tick_within_one_word,
};
use plutus_evm::{
    mapping::{SolidityMapping, StorageDecodable},
    storage::SmartContractStorage,
};
use uniswap_v3_math::{
    full_math::mul_div,
    liquidity_math::add_delta,
    swap_math::compute_swap_step,
    tick_math::{
        MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK, get_sqrt_ratio_at_tick,
        get_tick_at_sqrt_ratio,
    },
};

sol!(
    #[sol(rpc)]
    contract IPancakeSwapV3Pool {
        address public immutable override factory;
        address public immutable override token0;
        address public immutable override token1;
        uint24 public immutable override fee;
        int24 public immutable override tickSpacing;
        uint128 public immutable override maxLiquidityPerTick;

        #[derive(Debug)]
        struct Slot0 {
            uint160 sqrt_price_x96;
            int24 tick;
            uint16 observation_index;
            uint16 observation_cardinality;
            uint16 observation_cardinality_next;
            uint32 fee_protocol;
            bool unlocked;
        }

        #[derive(Debug)]
        struct TickInfo {
            uint128 liquidity_gross;
            int128 liquidity_net;
            uint256 fee_growth_outside_0_x128;
            uint256 fee_growth_outside_1_x128;
            int56 tick_cumulative_outside;
            uint160 seconds_per_liquidity_outside_x128;
            uint32 seconds_outside;
            bool initialized;
        }

        struct ProtocolFees {
            uint128 token0;
            uint128 token1;
        }

        ProtocolFees public override protocolFees;

        function slot0() public view returns (Slot0);
        function ticks(int24 tick) public returns (TickInfo);

        uint256 public override feeGrowthGlobal0X128;
        uint256 public override feeGrowthGlobal1X128;

        uint128 public override liquidity;

        mapping(int16 => uint256) public override tickBitmap;

        function swap(
            address recipient,
            bool zeroForOne,
            int256 amountSpecified,
            uint160 sqrtPriceLimitX96,
            bytes calldata data
        ) external override returns (int256 amount0, int256 amount1);

        #[derive(Debug)]
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );
    }
);

#[derive(Debug)]
pub struct PancakeSwapV3Pool {
    pub address: Address,

    pub token0: ERC20,
    pub token1: ERC20,
    pub fee: U24,
    pub tick_spacing: I24,

    pub slot0: RwLock<Slot0>,
    pub fee_growth_global_0_x128: RwLock<U256>,
    pub fee_growth_global_1_x128: RwLock<U256>,
    pub liquidity: RwLock<u128>,

    pub ticks: SolidityMapping<I24, TickInfo, 6, 4>,
    pub tick_bitmap: SolidityMapping<i16, U256, 7>,

    pub storage: SmartContractStorage,
}

#[derive(Debug, Clone, Copy)]
pub struct Slot0 {
    pub sqrt_price_x96: U160,
    pub tick: I24,
    pub fee_protocol: u32,
}

impl From<slot0Return> for Slot0 {
    fn from(value: slot0Return) -> Self {
        let slot0 = value._0;

        Self {
            sqrt_price_x96: slot0.sqrt_price_x96,
            tick: slot0.tick,
            fee_protocol: slot0.fee_protocol,
        }
    }
}

pub struct SwapCache {
    pub fee_protocol: u32,
    pub liquidity_start: u128,
}

const SLOT0_SLOT: U256 = uint!(0U256);
const SLOT0_SECOND_SLOT: U256 = uint!(1U256);
const FEE_GROWTH_GLOBAL_0_X128_SLOT: U256 = uint!(2U256);
const FEE_GROWTH_GLOBAL_1_X128_SLOT: U256 = uint!(3U256);
const LIQUIDITY_SLOT: U256 = uint!(5U256);

const PROTOCOL_FEE_SP: u32 = 65536;
const PROTOCOL_FEE_DENOMINATOR: U256 = uint!(10000U256);

impl PancakeSwapV3Pool {
    pub async fn new_with_provider<P: Provider>(
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Self, alloy::contract::Error> {
        let instance = IPancakeSwapV3PoolInstance::new(address, &provider);

        let storage = SmartContractStorage::new(address);
        storage
            .get_consecutive(SLOT0_SLOT, 2, block, &provider)
            .await?;

        Ok(Self {
            address,
            token0: ERC20::new_with_provider(
                instance.token0().block(block).call().await?.token0,
                &provider,
            )
            .await?,
            token1: ERC20::new_with_provider(
                instance.token1().block(block).call().await?.token1,
                &provider,
            )
            .await?,
            fee: instance.fee().block(block).call().await?.fee,
            tick_spacing: instance
                .tickSpacing()
                .block(block)
                .call()
                .await?
                .tickSpacing,
            slot0: RwLock::new(instance.slot0().block(block).call().await?.into()),
            fee_growth_global_0_x128: RwLock::new(
                instance
                    .feeGrowthGlobal0X128()
                    .block(block)
                    .call()
                    .await?
                    .feeGrowthGlobal0X128,
            ),
            fee_growth_global_1_x128: RwLock::new(
                instance
                    .feeGrowthGlobal1X128()
                    .block(block)
                    .call()
                    .await?
                    .feeGrowthGlobal1X128,
            ),
            liquidity: RwLock::new(instance.liquidity().block(block).call().await?.liquidity),

            ticks: SolidityMapping::new(),
            tick_bitmap: SolidityMapping::new(),
            storage,
        })
    }

    pub async fn exact_input_of<P: Provider>(
        &self,
        token: Address,
        amount: U256,
        block: BlockId,
        provider: P,
    ) -> U256 {
        let zero_for_one = token == self.token0.address;

        self.swap(
            zero_for_one,
            I256::from_raw(amount),
            if zero_for_one {
                MIN_SQRT_RATIO + U256::from(1)
            } else {
                MAX_SQRT_RATIO - U256::from(1)
            },
            block,
            provider,
        )
        .await
        .unwrap_or(U256::from(0))
    }

    pub async fn swap<P: Provider>(
        &self,
        zero_for_one: bool,
        amount_specified: I256,
        sqrt_price_limit_x96: U256,
        block: BlockId,
        provider: P,
    ) -> anyhow::Result<U256> {
        let slot0 = *self.slot0.read();

        if zero_for_one {
            assert!(sqrt_price_limit_x96 > MIN_SQRT_RATIO);

            if sqrt_price_limit_x96 >= U256::from(slot0.sqrt_price_x96) {
                return Ok(U256::from(0));
            }
        } else {
            assert!(sqrt_price_limit_x96 < MAX_SQRT_RATIO);

            if sqrt_price_limit_x96 <= U256::from(slot0.sqrt_price_x96) {
                return Ok(U256::from(0));
            }
        }

        let cache = SwapCache {
            liquidity_start: *self.liquidity.read(),
            fee_protocol: if zero_for_one {
                slot0.fee_protocol % PROTOCOL_FEE_SP
            } else {
                slot0.fee_protocol >> 16
            },
        };

        let exact_input = amount_specified.is_positive();

        let mut state = SwapState {
            amount_specified_remaining: amount_specified,
            amount_calculated: I256::ZERO,
            sqrt_price_x96: U256::from(slot0.sqrt_price_x96),
            tick: slot0.tick.try_into().expect("it fits"),
            fee_growth_global_x128: if zero_for_one {
                *self.fee_growth_global_0_x128.read()
            } else {
                *self.fee_growth_global_1_x128.read()
            },
            protocol_fee: 0,
            liquidity: cache.liquidity_start,
        };

        while state.amount_specified_remaining != I256::ZERO
            && state.sqrt_price_x96 != sqrt_price_limit_x96
        {
            let mut step = StepComputations {
                sqrt_price_start_x96: U256::ZERO,
                tick_next: 0,
                initialized: false,
                sqrt_price_next_x96: U256::ZERO,
                amount_in: U256::ZERO,
                amount_out: U256::ZERO,
                fee_amount: U256::ZERO,
            };

            step.sqrt_price_start_x96 = state.sqrt_price_x96;

            (step.tick_next, step.initialized) = next_initialized_tick_within_one_word(
                &self.tick_bitmap,
                state.tick.try_into().unwrap(),
                self.tick_spacing.try_into().unwrap(),
                zero_for_one,
                &self.storage,
                block,
                &provider,
            )
            .await?;

            step.tick_next = step.tick_next.clamp(MIN_TICK, MAX_TICK);

            step.sqrt_price_next_x96 = get_sqrt_ratio_at_tick(step.tick_next)?;

            (
                state.sqrt_price_x96,
                step.amount_in,
                step.amount_out,
                step.fee_amount,
            ) = compute_swap_step(
                U256::from(state.sqrt_price_x96),
                if if zero_for_one {
                    step.sqrt_price_next_x96 < sqrt_price_limit_x96
                } else {
                    step.sqrt_price_next_x96 > sqrt_price_limit_x96
                } {
                    U256::from(sqrt_price_limit_x96)
                } else {
                    U256::from(step.sqrt_price_next_x96)
                },
                state.liquidity,
                state.amount_specified_remaining,
                self.fee.to(),
            )?;

            if exact_input {
                state.amount_specified_remaining -=
                    I256::from_raw(step.amount_in + step.fee_amount);
                state.amount_calculated -= I256::from_raw(step.amount_out);
            } else {
                state.amount_specified_remaining += I256::from_raw(step.amount_out);
                state.amount_calculated += I256::from_raw(step.amount_in + step.fee_amount);
            }

            if cache.fee_protocol > 0 {
                let delta =
                    (step.fee_amount * U256::from(cache.fee_protocol)) / PROTOCOL_FEE_DENOMINATOR;

                step.fee_amount -= delta;
                state.protocol_fee += delta.to::<u128>();
            }

            if state.liquidity > 0 {
                state.fee_growth_global_x128 +=
                    mul_div(step.fee_amount, Q128, U256::from(state.liquidity))?;
            }

            if state.sqrt_price_x96 == step.sqrt_price_next_x96 {
                if step.initialized {
                    let mut liquidity_net = self
                        .ticks
                        .get(
                            &self.storage,
                            &I24::unchecked_from(step.tick_next),
                            block,
                            &provider,
                        )
                        .await?
                        .liquidity_net;

                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    state.liquidity = add_delta(state.liquidity, liquidity_net)?;
                }

                state.tick = if zero_for_one {
                    step.tick_next - 1
                } else {
                    step.tick_next
                };
            } else if state.sqrt_price_x96 != step.sqrt_price_start_x96 {
                state.tick = get_tick_at_sqrt_ratio(state.sqrt_price_x96)?;
            }
        }

        let (amount0, amount1) = if zero_for_one == exact_input {
            (
                amount_specified - state.amount_specified_remaining,
                state.amount_calculated,
            )
        } else {
            (
                state.amount_calculated,
                amount_specified - state.amount_specified_remaining,
            )
        };

        Ok(if zero_for_one {
            (-amount1).into_raw()
        } else {
            (-amount0).into_raw()
        })
    }
}

#[async_trait]
impl<P: Provider + 'static> LiquidityPool<P> for PancakeSwapV3Pool {
    async fn simulate_swap(
        &self,
        token: Address,
        amount: U256,
        block: BlockId,
        provider: P,
    ) -> U256 {
        self.exact_input_of(token, amount, block, provider).await
    }

    fn apply_storage_changes(&self, changes: hashbrown::HashMap<U256, U256>) {
        let mut slot0_updated = false;

        for (slot, value) in changes {
            match slot {
                _ if slot == SLOT0_SLOT || slot == SLOT0_SECOND_SLOT => {
                    slot0_updated = true;
                    self.storage.insert(slot, value)
                }
                _ if slot == FEE_GROWTH_GLOBAL_0_X128_SLOT => {
                    *self.fee_growth_global_0_x128.write() = value
                }
                _ if slot == FEE_GROWTH_GLOBAL_1_X128_SLOT => {
                    *self.fee_growth_global_1_x128.write() = value
                }
                _ if slot == LIQUIDITY_SLOT => *self.liquidity.write() = value.to(),
                _ => self.storage.insert(slot, value),
            }
        }

        if slot0_updated {
            // pancakeswap v3 pool's slot0 is split between 2 slots :)
            *self.slot0.write() = Slot0::decode(
                self.storage
                    .get_consecutive_cached(SLOT0_SLOT, 2)
                    .expect("slot0 cache is populated")
                    .into_iter()
                    .map(|value| value.to_le_bytes::<{ U256::BYTES }>())
                    .flatten()
                    .collect(),
            );
        }
    }

    fn is_liquidity_valid(&self) -> bool {
        let sqrt_price_x96 = U256::from(self.slot0.read().sqrt_price_x96);

        *self.liquidity.read() != 0
            && sqrt_price_x96 > MIN_SQRT_RATIO + U256::from(1)
            && sqrt_price_x96 < MAX_SQRT_RATIO - U256::from(1)
    }

    async fn tokens_locked(&self, provider: P) -> Result<(U256, U256), alloy::contract::Error> {
        Ok((
            self.token0.balance_of(self.address, &provider).await?,
            self.token1.balance_of(self.address, &provider).await?,
        ))
    }

    fn identifier(&self) -> &'static str {
        "pancakeswap_v3"
    }

    fn address(&self) -> Address {
        self.address
    }

    fn token0(&self) -> &ERC20 {
        &self.token0
    }

    fn token1(&self) -> &ERC20 {
        &self.token1
    }

    async fn verify_health(
        &self,
        provider: Arc<P>,
        block_number: BlockNumber,
    ) -> anyhow::Result<bool> {
        let instance = IPancakeSwapV3PoolInstance::new(self.address, &provider);

        let block: BlockId = block_number.into();

        let slot0 = instance.slot0().block(block).call().await?._0;
        let self_slot0 = *self.slot0.read();

        if slot0.sqrt_price_x96 != self_slot0.sqrt_price_x96 {
            bail!(
                "sqrt_price_x96 mismatch (pool {}) on block {block_number}, real {} != {}",
                self.address,
                slot0.sqrt_price_x96,
                self_slot0.sqrt_price_x96
            );
        }

        if slot0.tick != self_slot0.tick {
            bail!("tick mismatch");
        }

        if slot0.fee_protocol != self_slot0.fee_protocol {
            bail!(
                "fee_protocol mismatch, real {} != {}",
                slot0.fee_protocol,
                self_slot0.fee_protocol
            );
        }

        if instance
            .feeGrowthGlobal0X128()
            .block(block)
            .call()
            .await?
            .feeGrowthGlobal0X128
            != *self.fee_growth_global_0_x128.read()
        {
            bail!("fee_growth_global_0_x128 mismatch");
        }

        if instance
            .feeGrowthGlobal1X128()
            .block(block)
            .call()
            .await?
            .feeGrowthGlobal1X128
            != *self.fee_growth_global_1_x128.read()
        {
            bail!("fee_growth_global_1_x128 mismatch");
        }

        let storage = self.storage.storage.read().clone();

        for (slot, simulated_value) in storage {
            let slot: U256 = slot.into();

            match slot {
                SLOT0_SLOT
                | FEE_GROWTH_GLOBAL_0_X128_SLOT
                | FEE_GROWTH_GLOBAL_1_X128_SLOT
                | LIQUIDITY_SLOT => continue,
                _ => {}
            }

            let real_value = provider
                .get_storage_at(self.address, slot)
                .block_id(block_number.into())
                .await?;

            if simulated_value != real_value {
                bail!(
                    "storage mismatch (pool {}) on block {block_number} at {}, real {} != {}",
                    self.address,
                    hex::encode(slot.to_be_bytes::<32>()),
                    hex::encode(real_value.to_be_bytes::<32>()),
                    hex::encode(simulated_value.to_be_bytes::<32>())
                );
            }
        }

        Ok(true)
    }

    async fn update_with_provider(
        &self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error> {
        let instance = IPancakeSwapV3PoolInstance::new(self.address, provider);

        *self.slot0.write() = instance.slot0().block(block).call().await?.into();

        *self.fee_growth_global_0_x128.write() = instance
            .feeGrowthGlobal0X128()
            .block(block)
            .call()
            .await?
            .feeGrowthGlobal0X128;

        *self.fee_growth_global_1_x128.write() = instance
            .feeGrowthGlobal1X128()
            .block(block)
            .call()
            .await?
            .feeGrowthGlobal1X128;

        *self.liquidity.write() = instance.liquidity().block(block).call().await?.liquidity;

        self.storage.clear();

        Ok(())
    }

    fn create_payload(
        &self,
        recipient: Address,
        token_in: Address,
        amount: U256,
        extra: Vec<u8>,
    ) -> Vec<u8> {
        let zero_for_one = token_in == self.token0.address;

        let price_limit = U160::from(if zero_for_one {
            MIN_SQRT_RATIO + U256::from(1)
        } else {
            MAX_SQRT_RATIO - U256::from(1)
        });

        let data = SwapDataPayload::abi_encode_sequence(&(self.address, token_in, amount, extra));

        IPancakeSwapV3Pool::swapCall::new((
            recipient,
            zero_for_one,
            I256::from_raw(amount),
            price_limit,
            data.into(),
        ))
        .abi_encode()
    }
}

impl StorageDecodable for Slot0 {
    fn decode(bytes: Vec<u8>) -> Self {
        let (sqrt_price_x96, bytes) = bytes.split_at(U160::BYTES);
        let (tick, bytes) = bytes.split_at(I24::BYTES);
        let (_observation_index, bytes) = bytes.split_at(U16::BYTES);
        let (_observation_cardinality, bytes) = bytes.split_at(U16::BYTES);
        let (_observation_cardinality_next, bytes) = bytes.split_at(U16::BYTES);
        let (_padding_between_slots, bytes) = bytes.split_at(U24::BYTES);
        let (fee_protocol, _) = bytes.split_at(U32::BYTES);

        Self {
            sqrt_price_x96: U160::from_le_slice(sqrt_price_x96),
            tick: I24::from_raw(U24::from_le_slice(tick)),
            fee_protocol: u32::from_le_bytes(fee_protocol.try_into().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::{U160, address},
        providers::ProviderBuilder,
        rpc::client::ClientBuilder,
    };
    use dotenvy_macro::dotenv;
    use uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MIN_SQRT_RATIO};

    use crate::v3::quoter::{PancakeSwapV3Quoter, QuoteExactInputSingleParams};

    use super::*;

    const POOLS: &[Address] = &[
        address!("4bfc22a4da7f31f8a912a79a7e44a822398b4390"),
        address!("d9e2a1a61b6e61b275cec326465d417e52c1b95c"),
        address!("5e3c3a063cc9a4aeb5310c7fadc2a98aebdd245d"),
        address!("389938cf14be379217570d8e4619e51fbdafaa21"),
        address!("7fcdc35463e3770c2fb992716cd070b63540b947"),
        address!("641b559551f8fc76a1664663df929906a83b0774"),
    ];

    #[tokio::test(flavor = "multi_thread")]
    pub async fn swaps_are_correct() -> anyhow::Result<()> {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await?
                    .boxed(),
            ),
        );

        let block: BlockId = provider.get_block_number().await?.into();

        let quoter = PancakeSwapV3Quoter::new(provider.clone());

        for address in POOLS {
            let pool =
                PancakeSwapV3Pool::new_with_provider(*address, provider.clone(), block).await?;

            for amount in 1..100 {
                let token0_out = pool
                    .simulate_swap(
                        pool.token0.address,
                        pool.token0.to_token_amount(amount as f64),
                        block,
                        provider.clone(),
                    )
                    .await;

                let quoted_token0_out = quoter
                    .quote_exact_input_single_on_block(
                        QuoteExactInputSingleParams {
                            token_in: pool.token0.address,
                            token_out: pool.token1.address,
                            amount_in: pool.token0.to_token_amount(amount as f64),
                            fee: pool.fee,
                            sqrt_price_limit_x96: U160::from(MIN_SQRT_RATIO + U256::from(1)),
                        },
                        block,
                    )
                    .await?
                    .amount_out;

                if token0_out != quoted_token0_out {
                    panic!(
                        "swap mismatch: token0 -> token1, pool = {}, amount = {amount}, quoted {} != {}",
                        pool.address, quoted_token0_out, token0_out
                    );
                }

                let token1_out = pool
                    .simulate_swap(
                        pool.token1.address,
                        pool.token1.to_token_amount(amount as f64),
                        block,
                        provider.clone(),
                    )
                    .await;

                let quoted_token1_out = quoter
                    .quote_exact_input_single_on_block(
                        QuoteExactInputSingleParams {
                            token_in: pool.token1.address,
                            token_out: pool.token0.address,
                            amount_in: pool.token1.to_token_amount(amount as f64),
                            fee: pool.fee,
                            sqrt_price_limit_x96: U160::from(MAX_SQRT_RATIO - U256::from(1)),
                        },
                        block,
                    )
                    .await?
                    .amount_out;

                if token1_out != quoted_token1_out {
                    panic!(
                        "swap mismatch: token1 -> token0, pool = {}, amount = {amount}, quoted {} != {}",
                        pool.address, quoted_token1_out, token1_out
                    );
                }
            }
        }

        Ok(())
    }

    // #[tokio::test(flavor = "multi_thread")]
    // pub async fn decoding_is_correct() -> anyhow::Result<()> {
    //     let provider = Arc::new(
    //         ProviderBuilder::new().with_recommended_fillers().on_client(
    //             ClientBuilder::default()
    //                 .ipc(dotenv!("IPC_PROVIDER").to_string().into())
    //                 .await?
    //                 .boxed(),
    //         ),
    //     );
    //
    //     let block_number = provider.get_block_number().await?;
    //     let mut evm = EVM::new(provider.clone(), block_number);
    //
    //     for address in POOLS {
    //         let pool = PancakeSwapV3Pool::new(*address, &mut evm)?;
    //         pool.verify_health(provider.clone(), block_number).await?;
    //     }
    //
    //     Ok(())
    // }
}
