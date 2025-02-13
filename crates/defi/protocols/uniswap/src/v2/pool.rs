use std::sync::Arc;

use IUniswapV2Pool::{IUniswapV2PoolInstance, token0Call, token1Call};
use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U256, aliases::U112},
    providers::Provider,
    sol,
    sol_types::SolCall as _,
    uint,
};
use anyhow::bail;
use async_trait::async_trait;
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

        function getReserves() public view returns (uint112 _reserve0, uint112 _reserve1, uint32 _blockTimestampLast);

        function swap(uint amount0Out, uint amount1Out, address to, bytes calldata data) external;
    }
);

#[derive(Clone, Copy)]
pub struct Reserves(U112, U112);

#[derive(Clone)]
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

    pub async fn new_with_provider<P: Provider>(
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Self, alloy::contract::Error> {
        let instance = IUniswapV2PoolInstance::new(address, &provider);

        let reserves = instance.getReserves().call().block(block).await?;

        Ok(Self {
            address,
            token0: ERC20::new_with_provider(
                instance.token0().call().block(block).await?.token0,
                &provider,
            )
            .await?,
            token1: ERC20::new_with_provider(
                instance.token1().call().block(block).await?.token1,
                &provider,
            )
            .await?,
            reserves: Reserves(reserves._reserve0, reserves._reserve1),
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

#[async_trait]
impl<P: Provider + 'static> LiquidityPool<P> for UniswapV2Pool {
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

    fn token_addresses(&self) -> (Address, Address) {
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

    fn tokens(&self) -> (ERC20, ERC20) {
        (self.token0.clone(), self.token1.clone())
    }

    async fn verify_health(
        &self,
        provider: Arc<P>,
        block_number: BlockNumber,
    ) -> anyhow::Result<bool> {
        let instance = IUniswapV2PoolInstance::new(self.address, provider);

        let block: BlockId = block_number.into();

        // if instance.token0().block(block).call().await?.token0 != self.token0.address {
        //     bail!("token0 address mismatch");
        // }
        //
        // if instance.token1().block(block).call().await?.token1 != self.token1.address {
        //     bail!("token1 address mismatch");
        // }

        let reserves = instance.getReserves().block(block).call().await?;

        if reserves._reserve0 != self.reserves.0 {
            bail!(
                "reserve0 mismatch on block {block_number}, real {} != {}",
                reserves._reserve0,
                self.reserves.0
            );
        }

        if reserves._reserve1 != self.reserves.1 {
            bail!(
                "reserve1 mismatch on block {block_number}, real {} != {}",
                reserves._reserve1,
                self.reserves.1
            );
        }

        Ok(true)
    }
}
