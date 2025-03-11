use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use alloy::{
    eips::BlockId,
    primitives::{Address, BlockNumber, U256},
    providers::Provider,
};
use async_trait::async_trait;
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_defi_protocols_uniswap::v2::pool::UniswapV2Pool;

pub struct PancakeSwapV2Pool(pub(super) UniswapV2Pool);

impl PancakeSwapV2Pool {
    pub async fn new_with_provider<P: Provider>(
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Self, alloy::contract::Error> {
        Ok(Self(
            UniswapV2Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

impl Deref for PancakeSwapV2Pool {
    type Target = UniswapV2Pool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PancakeSwapV2Pool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<P: Provider + 'static> LiquidityPool<P> for PancakeSwapV2Pool {
    fn simulate_swap(&self, token: Address, amount_in: U256, block: BlockId) -> U256 {
        let zero_for_one = token == self.token0.address;

        if let Some(amount_out) = self.swap_cache.get(block, zero_for_one, amount_in) {
            return amount_out;
        }

        let (reserve_in, reserve_out) = {
            let reserves = self.reserves;

            if zero_for_one {
                (U256::from(reserves.0), U256::from(reserves.1))
            } else {
                (U256::from(reserves.1), U256::from(reserves.0))
            }
        };

        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        let amount_in_with_fee = amount_in * U256::from(9975);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(10000) + amount_in_with_fee;

        let amount_out = numerator / denominator;

        self.swap_cache
            .insert(block, zero_for_one, amount_in, amount_out);

        amount_out
    }

    fn apply_storage_changes(&mut self, changes: hashbrown::HashMap<U256, U256>) {
        <UniswapV2Pool as LiquidityPool<P>>::apply_storage_changes(&mut self.0, changes);
    }

    fn is_liquidity_valid(&self) -> bool {
        <UniswapV2Pool as LiquidityPool<P>>::is_liquidity_valid(&self.0)
    }

    fn tokens_locked(&self) -> (U256, U256) {
        <UniswapV2Pool as LiquidityPool<P>>::tokens_locked(&self.0)
    }

    fn identifier(&self) -> &'static str {
        "pancakeswap_v2"
    }

    fn address(&self) -> Address {
        <UniswapV2Pool as LiquidityPool<P>>::address(&self.0)
    }

    fn token0(&self) -> &ERC20 {
        <UniswapV2Pool as LiquidityPool<P>>::token0(&self.0)
    }

    fn token1(&self) -> &ERC20 {
        <UniswapV2Pool as LiquidityPool<P>>::token1(&self.0)
    }

    async fn verify_health(
        &self,
        provider: Arc<P>,
        block_number: BlockNumber,
    ) -> anyhow::Result<bool> {
        <UniswapV2Pool as LiquidityPool<P>>::verify_health(&self.0, provider, block_number).await
    }

    async fn update_with_provider(
        &self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error> {
        <UniswapV2Pool as LiquidityPool<P>>::update_with_provider(&self.0, provider, block).await
    }

    fn create_payload(
        &self,
        recipient: Address,
        token_in: Address,
        amount: U256,
        extra: Vec<u8>,
    ) -> Vec<u8> {
        <UniswapV2Pool as LiquidityPool<P>>::create_payload(
            &self.0, recipient, token_in, amount, extra,
        )
    }
}

#[cfg(test)]
mod tests {
    use alloy::{primitives::address, providers::ProviderBuilder, rpc::client::ClientBuilder};
    use dotenvy_macro::dotenv;

    use crate::v2::router::PancakeSwapV2Router;

    use super::*;

    const POOLS: &[Address] = &[
        address!("171aacdE5cf1C4777fcA87B5Ee6CBdE222695e48"),
        address!("bB87Ba4F2b9354684eB9A2d2dDa93400b8a9D5Df"),
        address!("6b3287BA3D8E541Aaab6f1542De3e963853d9f03"),
        address!("5A09F489e1A9144f4B6088543c7882b316239FeA"),
        address!("3D11C7cf46914524C87829d12e41Fe0B2d7AB774"),
    ];

    fn as_liquidity_pool<P: Provider + 'static>(
        pool: PancakeSwapV2Pool,
        _provider: P,
    ) -> Box<dyn LiquidityPool<P>> {
        Box::new(pool) as Box<dyn LiquidityPool<P>>
    }

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

        let router = PancakeSwapV2Router::new(provider.clone());

        for address in POOLS {
            let pool =
                PancakeSwapV2Pool::new_with_provider(*address, provider.clone(), block).await?;

            let liquidity_pool = as_liquidity_pool(
                PancakeSwapV2Pool::new_with_provider(*address, provider.clone(), block).await?,
                provider.clone(),
            );

            let reserves = pool.reserves;

            for amount in 1..100 {
                let token0_out = liquidity_pool.simulate_swap(
                    pool.token0.address,
                    pool.token0.to_token_amount(amount as f64),
                    block,
                );

                let quoted_token0_out = router
                    .get_amount_out(
                        pool.token0.to_token_amount(amount as f64),
                        U256::from(reserves.0),
                        U256::from(reserves.1),
                        block,
                    )
                    .await?;

                if token0_out != quoted_token0_out {
                    panic!(
                        "swap mismatch: token0 -> token1, pool = {}, amount = {amount}, quoted {} != {}",
                        pool.address, quoted_token0_out, token0_out
                    );
                }

                let token1_out = liquidity_pool.simulate_swap(
                    pool.token1.address,
                    pool.token1.to_token_amount(amount as f64),
                    block,
                );

                let quoted_token1_out = router
                    .get_amount_out(
                        pool.token1.to_token_amount(amount as f64),
                        U256::from(reserves.1),
                        U256::from(reserves.0),
                        block,
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
}
