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
pub struct Reserves(pub U112, pub U112);

#[derive(Clone)]
pub struct UniswapV2Pool {
    pub address: Address,

    pub token0: ERC20,
    pub token1: ERC20,

    pub reserves: Reserves,
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
    async fn simulate_swap(
        &mut self,
        token: Address,
        amount: U256,
        _block: BlockId,
        _provider: P,
    ) -> U256 {
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

    async fn tokens_locked(&self, _provider: P) -> Result<(U256, U256), alloy::contract::Error> {
        Ok((U256::from(self.reserves.0), U256::from(self.reserves.1)))
    }

    fn identifier(&self) -> &'static str {
        "uniswap_v2"
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
        let instance = IUniswapV2PoolInstance::new(self.address, provider);

        let block: BlockId = block_number.into();

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

    async fn update_with_provider(
        &mut self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error> {
        let instance = IUniswapV2PoolInstance::new(self.address, provider);
        let reserves = instance.getReserves().block(block).call().await?;

        self.reserves.0 = reserves._reserve0;
        self.reserves.1 = reserves._reserve1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy::{primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder};
    use dotenvy_macro::dotenv;

    use crate::v2::router::UniswapV2Router;

    use super::*;

    const POOLS: &[Address] = &[
        address!("F64Dfe17C8b87F012FCf50FbDA1D62bfA148366a"),
        address!("d04Bc65744306A5C149414dd3CD5c984D9d3470d"),
        address!("6FA774876Fe6badEB1a4d0c6dCf9430A72d3873B"),
        address!("4c27B5E88c6d6ad95EBfAcEd535576608cA6fe0a"),
        address!("3dA7FE3f0eD5c8e82A6E5b046C635274912a5db0"),
    ];

    #[tokio::test(flavor = "multi_thread")]
    async fn swaps_are_correct() -> anyhow::Result<()> {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await?
                    .boxed(),
            ),
        );

        let block: BlockId = provider.get_block_number().await?.into();

        let router = UniswapV2Router::new(provider.clone());

        for address in POOLS {
            let mut pool =
                UniswapV2Pool::new_with_provider(*address, provider.clone(), block).await?;

            for amount in 1..100 {
                let token0_out = pool
                    .simulate_swap(
                        pool.token0.address,
                        pool.token0.to_token_amount(amount as f64),
                        block,
                        provider.clone(),
                    )
                    .await;

                let quoted_token0_out = router
                    .get_amount_out(
                        pool.token0.to_token_amount(amount as f64),
                        U256::from(pool.reserves.0),
                        U256::from(pool.reserves.1),
                    )
                    .await?;

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

                let quoted_token1_out = router
                    .get_amount_out(
                        pool.token1.to_token_amount(amount as f64),
                        U256::from(pool.reserves.1),
                        U256::from(pool.reserves.0),
                    )
                    .await?;

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

    #[tokio::test(flavor = "multi_thread")]
    async fn decoding_is_correct() -> anyhow::Result<()> {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await?
                    .boxed(),
            ),
        );

        let block_number = provider.get_block_number().await?;
        let mut evm = EVM::new(provider.clone(), block_number);

        for address in POOLS {
            let pool = UniswapV2Pool::new(*address, &mut evm)?;
            pool.verify_health(provider.clone(), block_number).await?;
        }

        Ok(())
    }
}
