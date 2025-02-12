use IUniswapV3Pool::{feeCall, tickSpacingCall, token0Call, token1Call};
use alloy::{
    primitives::{
        Address, I128, I256, U8, U16, U32, U128, U160, U256,
        aliases::{I24, I56, U24, U56},
    },
    providers::Provider,
    sol,
    sol_types::SolCall as _,
    uint,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{
    EVM,
    errors::EvmCallError,
    mapping::{SolidityMapping, StorageDecodable},
    storage::{FromStorageValue, SmartContractStorage},
};
use uniswap_v3_math::{
    error::UniswapV3MathError,
    full_math::mul_div,
    liquidity_math::add_delta,
    swap_math::compute_swap_step,
    tick_math::{
        MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK, get_sqrt_ratio_at_tick,
        get_tick_at_sqrt_ratio,
    },
};

use super::tick_bitmap::next_initialized_tick_within_one_word;

sol!(
    #[sol(rpc)]
    contract IUniswapV3Pool {
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
            uint8 fee_protocol;
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

        event Initialize(uint160 sqrtPriceX96, int24 tick);
        event Mint(
            address sender,
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );

        event Collect(
            address indexed owner,
            address recipient,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount0,
            uint128 amount1
        );

        event Burn(
            address indexed owner,
            int24 indexed tickLower,
            int24 indexed tickUpper,
            uint128 amount,
            uint256 amount0,
            uint256 amount1
        );

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

        event Flash(
            address indexed sender,
            address indexed recipient,
            uint256 amount0,
            uint256 amount1,
            uint256 paid0,
            uint256 paid1
        );

        event IncreaseObservationCardinalityNext(
            uint16 observationCardinalityNextOld,
            uint16 observationCardinalityNextNew
        );

        event SetFeeProtocol(uint8 feeProtocol0Old, uint8 feeProtocol1Old, uint8 feeProtocol0New, uint8 feeProtocol1New);

        event CollectProtocol(address indexed sender, address indexed recipient, uint128 amount0, uint128 amount1);
    }
);

#[derive(Debug, Clone)]
pub struct UniswapV3Pool {
    pub address: Address,

    pub token0: ERC20,
    pub token1: ERC20,
    pub fee: U24,
    pub tick_spacing: I24,

    pub slot0: Slot0,
    pub fee_growth_global_0_x128: U256,
    pub fee_growth_global_1_x128: U256,
    pub liquidity: u128,

    pub ticks: SolidityMapping<I24, TickInfo, 5, 4>,
    pub tick_bitmap: SolidityMapping<i16, U256, 6>,

    pub storage: SmartContractStorage,
}

#[derive(Debug, Clone, Copy)]
pub struct TickInfo {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_x128: U256,
    pub fee_growth_outside_1_x128: U256,
    pub tick_cumulative_outside: I56,
    pub seconds_per_liquidity_outside_x128: U160,
    pub seconds_outside: u32,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Slot0 {
    pub sqrt_price_x96: U160,
    pub tick: I24,
    pub fee_protocol: u8,
}

enum Slot {
    Slot0,
    FeeGrowthGlobal0X128,
    FeeGrowthGlobal1X128,
    Liquidity,
}

struct SwapCache {
    fee_protocol: u8,
    liquidity_start: u128,
}

struct SwapState {
    amount_specified_remaining: I256,
    amount_calculated: I256,
    sqrt_price_x96: U256,
    tick: i32,
    fee_growth_global_x128: U256,
    protocol_fee: u128,
    liquidity: u128,
}

struct StepComputations {
    sqrt_price_start_x96: U256,
    tick_next: i32,
    initialized: bool,
    sqrt_price_next_x96: U256,
    amount_in: U256,
    amount_out: U256,
    fee_amount: U256,
}

const Q128: U256 = uint!(0x100000000000000000000000000000000U256);

const SLOT0_SLOT: U256 = uint!(0U256);
const FEE_GROWTH_GLOBAL_0_X128_SLOT: U256 = uint!(1U256);
const FEE_GROWTH_GLOBAL_1_X128_SLOT: U256 = uint!(2U256);
const LIQUIDITY_SLOT: U256 = uint!(4U256);

impl UniswapV3Pool {
    pub fn new<P: Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, EvmCallError<P>> {
        let mut storage = SmartContractStorage::new(address);

        Ok(Self {
            address,
            token0: ERC20::new(evm.call(address, token0Call::new(()))?.output.token0, evm)?,
            token1: ERC20::new(evm.call(address, token1Call::new(()))?.output.token1, evm)?,
            fee: evm.call(address, feeCall::new(()))?.output.fee,
            tick_spacing: evm
                .call(address, tickSpacingCall::new(()))?
                .output
                .tickSpacing,

            slot0: Slot0::from_storage_value(evm.storage(address, SLOT0_SLOT)),
            fee_growth_global_0_x128: storage.get(FEE_GROWTH_GLOBAL_0_X128_SLOT, evm),
            fee_growth_global_1_x128: storage.get(FEE_GROWTH_GLOBAL_1_X128_SLOT, evm),
            liquidity: storage.get(LIQUIDITY_SLOT, evm).to(),

            ticks: SolidityMapping::new(),
            tick_bitmap: SolidityMapping::new(),
            storage,
        })
    }

    pub fn exact_input_of<P: Provider>(
        &mut self,
        token: Address,
        amount: U256,
        evm: &mut EVM<P>,
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
            evm,
        )
        .unwrap_or(U256::from(0))
    }

    pub fn swap<P: Provider>(
        &mut self,
        zero_for_one: bool,
        amount_specified: I256,
        sqrt_price_limit_x96: U256,
        evm: &mut EVM<P>,
    ) -> Result<U256, UniswapV3MathError> {
        if zero_for_one {
            assert!(sqrt_price_limit_x96 > MIN_SQRT_RATIO);

            if sqrt_price_limit_x96 >= U256::from(self.slot0.sqrt_price_x96) {
                return Ok(U256::from(0));
            }
        } else {
            assert!(sqrt_price_limit_x96 < MAX_SQRT_RATIO);

            if sqrt_price_limit_x96 <= U256::from(self.slot0.sqrt_price_x96) {
                return Ok(U256::from(0));
            }
        }

        let cache = SwapCache {
            liquidity_start: self.liquidity,
            fee_protocol: if zero_for_one {
                self.slot0.fee_protocol % 16
            } else {
                self.slot0.fee_protocol >> 4
            },
        };

        let exact_input = amount_specified.is_positive();

        let mut state = SwapState {
            amount_specified_remaining: amount_specified,
            amount_calculated: I256::ZERO,
            sqrt_price_x96: U256::from(self.slot0.sqrt_price_x96),
            tick: self.slot0.tick.try_into().expect("it fits"),
            fee_growth_global_x128: if zero_for_one {
                self.fee_growth_global_0_x128
            } else {
                self.fee_growth_global_1_x128
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
                &mut self.tick_bitmap,
                state.tick.try_into().unwrap(),
                self.tick_spacing.try_into().unwrap(),
                zero_for_one,
                evm,
                &mut self.storage,
            )?;

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
                let delta = step.fee_amount / U256::from(cache.fee_protocol);
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
                        .get(&mut self.storage, &I24::unchecked_from(step.tick_next), evm)
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

impl<P: Provider> LiquidityPool<P> for UniswapV3Pool {
    fn simulate_swap(&mut self, token: Address, amount: U256, evm: &mut EVM<P>) -> U256 {
        self.exact_input_of(token, amount, evm)
    }

    fn apply_storage_changes(&mut self, changes: hashbrown::HashMap<U256, U256>) {
        for (slot, value) in changes {
            match slot {
                _ if slot == SLOT0_SLOT => self.slot0 = Slot0::from_storage_value(value),
                _ if slot == FEE_GROWTH_GLOBAL_0_X128_SLOT => self.fee_growth_global_0_x128 = value,
                _ if slot == FEE_GROWTH_GLOBAL_1_X128_SLOT => self.fee_growth_global_1_x128 = value,
                _ if slot == LIQUIDITY_SLOT => self.liquidity = value.to(),
                _ => self.storage.insert(slot, value),
            }
        }
    }

    fn is_liquidity_valid(&self) -> bool {
        let sqrt_price_x96 = U256::from(self.slot0.sqrt_price_x96);

        self.liquidity != 0
            && sqrt_price_x96 > MIN_SQRT_RATIO + U256::from(1)
            && sqrt_price_x96 < MAX_SQRT_RATIO - U256::from(1)
    }
}

impl StorageDecodable for TickInfo {
    fn decode(bytes: Vec<u8>) -> Self {
        let (liquidity_gross, bytes) = bytes.split_at(U128::BYTES);
        let (liquidity_net, bytes) = bytes.split_at(I128::BYTES);
        let (fee_growth_outside_0_x128, bytes) = bytes.split_at(U256::BYTES);
        let (fee_growth_outside_1_x128, bytes) = bytes.split_at(U256::BYTES);
        let (tick_cumulative_outside, bytes) = bytes.split_at(I56::BYTES);
        let (seconds_per_liquidity_outside_x128, bytes) = bytes.split_at(U160::BYTES);
        let (seconds_outside, initialized) = bytes.split_at(U32::BYTES);

        Self {
            liquidity_gross: u128::from_le_bytes(liquidity_gross.try_into().unwrap()),
            liquidity_net: i128::from_le_bytes(liquidity_net.try_into().unwrap()),
            fee_growth_outside_0_x128: U256::from_le_slice(fee_growth_outside_0_x128),
            fee_growth_outside_1_x128: U256::from_le_slice(fee_growth_outside_1_x128),
            tick_cumulative_outside: I56::from_raw(U56::from_le_slice(tick_cumulative_outside)),
            seconds_per_liquidity_outside_x128: U160::from_le_slice(
                seconds_per_liquidity_outside_x128,
            ),
            seconds_outside: u32::from_le_bytes(seconds_outside.try_into().unwrap()),
            initialized: initialized[0] != 0,
        }
    }
}

impl FromStorageValue for Slot0 {
    fn from_storage_value(value: U256) -> Self {
        let bytes = value.to_le_bytes::<32>();

        let (sqrt_price_x96, bytes) = bytes.split_at(U160::BYTES);
        let (tick, bytes) = bytes.split_at(I24::BYTES);
        let (_observation_index, bytes) = bytes.split_at(U16::BYTES);
        let (_observation_cardinality, bytes) = bytes.split_at(U16::BYTES);
        let (_observation_cardinality_next, bytes) = bytes.split_at(U16::BYTES);
        let (fee_protocol, _unlocked) = bytes.split_at(U8::BYTES);

        Self {
            sqrt_price_x96: U160::from_le_slice(sqrt_price_x96),
            tick: I24::from_raw(U24::from_le_slice(tick)),
            fee_protocol: fee_protocol[0],
        }
    }
}
