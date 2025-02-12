use IUniswapV2Pool::{token0Call, token1Call};
use alloy::{
    primitives::{Address, U256, aliases::U112},
    providers::Provider,
    sol,
    sol_types::SolCall as _,
    uint,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{EVM, errors::EvmCallError, storage::FromStorageValue};

sol!(
    #[sol(rpc)]
    contract IUniswapV2Pool {
        address public factory;

        address public token0;
        address public token1;

        uint112 private reserve0;
        uint112 private reserve1;

        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }
);

pub struct Reserves(U112, U112);

pub struct UniswapV2Pool {
    address: Address,

    token0: ERC20,
    token1: ERC20,

    reserves: Reserves,
}

impl UniswapV2Pool {
    pub fn new<P: Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, EvmCallError<P>> {
        Ok(Self {
            address,
            token0: ERC20::new(evm.call(address, token0Call::new(()))?.output.token0, evm)?,
            token1: ERC20::new(evm.call(address, token1Call::new(()))?.output.token1, evm)?,
            reserves: Reserves::from_storage_value(evm.storage(address, uint!(8U256))),
        })
    }

    pub fn swap(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        let amount_in_with_fee = amount_in * U256::from(997);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;

        numerator / denominator
    }
}

impl FromStorageValue for Reserves {
    fn from_storage_value(value: U256) -> Self {
        let bytes = value.to_le_bytes::<{ U256::BYTES }>();

        let (reserve0, bytes) = bytes.split_at(U112::BYTES);
        let (reserve1, _) = bytes.split_at(U112::BYTES);

        Self(U112::from_le_slice(reserve0), U112::from_le_slice(reserve1))
    }
}

impl<P: Provider> LiquidityPool<P> for UniswapV2Pool {
    fn simulate_swap(&mut self, token: Address, amount: U256, _evm: &mut EVM<P>) -> U256 {
        let (reserve_in, reserve_out) = if token == self.token0.address {
            (self.reserves.0, self.reserves.1)
        } else {
            (self.reserves.1, self.reserves.0)
        };

        Self::swap(amount, U256::from(reserve_in), U256::from(reserve_out))
    }

    fn apply_storage_changes(&mut self, changes: hashbrown::HashMap<U256, U256>) {
        let reserves_slot_value = changes.get(&uint!(8U256));

        if let Some(value) = reserves_slot_value {
            self.reserves = Reserves::from_storage_value(*value);
        }
    }

    fn is_liquidity_valid(&self) -> bool {
        !self.reserves.0.is_zero() && !self.reserves.1.is_zero()
    }

    fn tokens(&self) -> (Address, Address) {
        (self.token0.address, self.token1.address)
    }

    fn tokens_locked(&self, _evm: &mut EVM<P>) -> Result<(U256, U256), EvmCallError<P>> {
        Ok((U256::from(self.reserves.0), U256::from(self.reserves.1)))
    }

    fn identifier(&self) -> &'static str {
        "uniswap_v2"
    }

    fn address(&self) -> Address {
        self.address
    }
}
