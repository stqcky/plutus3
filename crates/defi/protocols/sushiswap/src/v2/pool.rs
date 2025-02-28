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
use plutus_evm::{EVM, errors::EvmCallError};

#[derive(Clone)]
pub struct SushiSwapV2Pool(pub(super) UniswapV2Pool);

impl SushiSwapV2Pool {
    pub fn new<P: Provider>(address: Address, evm: &mut EVM<P>) -> Result<Self, EvmCallError<P>> {
        Ok(Self(UniswapV2Pool::new(address, evm)?))
    }

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

impl Deref for SushiSwapV2Pool {
    type Target = UniswapV2Pool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SushiSwapV2Pool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<P: Provider + 'static> LiquidityPool<P> for SushiSwapV2Pool {
    async fn simulate_swap(
        &mut self,
        token: Address,
        amount: U256,
        _block: BlockId,
        _provider: P,
    ) -> U256 {
        let (reserve_in, reserve_out) = if token == self.token0.address {
            (U256::from(self.reserves.0), U256::from(self.reserves.1))
        } else {
            (U256::from(self.reserves.1), U256::from(self.reserves.0))
        };

        if amount.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        let amount_in_with_fee = amount * U256::from(9975);
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(10000) + amount_in_with_fee;

        numerator / denominator
    }

    fn apply_storage_changes(&mut self, changes: hashbrown::HashMap<U256, U256>) {
        <UniswapV2Pool as LiquidityPool<P>>::apply_storage_changes(&mut self.0, changes);
    }

    fn is_liquidity_valid(&self) -> bool {
        <UniswapV2Pool as LiquidityPool<P>>::is_liquidity_valid(&self.0)
    }

    async fn tokens_locked(&self, _provider: P) -> Result<(U256, U256), alloy::contract::Error> {
        <UniswapV2Pool as LiquidityPool<P>>::tokens_locked(&self.0, _provider).await
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
        &mut self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error> {
        <UniswapV2Pool as LiquidityPool<P>>::update_with_provider(&mut self.0, provider, block)
            .await
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

    use crate::v2::router::SushiSwapV2Router;

    use super::*;

    const POOLS: &[Address] = &[
        address!("171aacdE5cf1C4777fcA87B5Ee6CBdE222695e48"),
        address!("bB87Ba4F2b9354684eB9A2d2dDa93400b8a9D5Df"),
        address!("6b3287BA3D8E541Aaab6f1542De3e963853d9f03"),
        address!("5A09F489e1A9144f4B6088543c7882b316239FeA"),
        address!("3D11C7cf46914524C87829d12e41Fe0B2d7AB774"),
    ];

    // #[tokio::test(flavor = "multi_thread")]
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

        let router = SushiSwapV2Router::new(provider.clone());

        for address in POOLS {
            let mut pool =
                SushiSwapV2Pool::new_with_provider(*address, provider.clone(), block).await?;

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
}
