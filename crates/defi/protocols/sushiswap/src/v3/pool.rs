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
use plutus_defi_protocols_uniswap::v3::pool::UniswapV3Pool;

pub struct SushiSwapV3Pool(pub(super) UniswapV3Pool);

impl SushiSwapV3Pool {
    pub async fn new_with_provider<P: Provider>(
        address: Address,
        provider: P,
        block: BlockId,
    ) -> Result<Self, alloy::contract::Error> {
        Ok(Self(
            UniswapV3Pool::new_with_provider(address, provider, block).await?,
        ))
    }
}

impl Deref for SushiSwapV3Pool {
    type Target = UniswapV3Pool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SushiSwapV3Pool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl<P: Provider + 'static> LiquidityPool<P> for SushiSwapV3Pool {
    fn simulate_swap(&self, token: Address, amount: U256, block: BlockId) -> U256 {
        <UniswapV3Pool as LiquidityPool<P>>::simulate_swap(&self.0, token, amount, block)
    }

    fn apply_storage_changes(&self, changes: hashbrown::HashMap<U256, U256>) {
        <UniswapV3Pool as LiquidityPool<P>>::apply_storage_changes(&self.0, changes);
    }

    fn is_liquidity_valid(&self) -> bool {
        <UniswapV3Pool as LiquidityPool<P>>::is_liquidity_valid(&self.0)
    }

    fn tokens_locked(&self) -> (U256, U256) {
        <UniswapV3Pool as LiquidityPool<P>>::tokens_locked(&self.0)
    }

    fn identifier(&self) -> &'static str {
        "sushiswap_v3"
    }

    fn address(&self) -> Address {
        <UniswapV3Pool as LiquidityPool<P>>::address(&self.0)
    }

    fn token0(&self) -> &ERC20 {
        <UniswapV3Pool as LiquidityPool<P>>::token0(&self.0)
    }

    fn token1(&self) -> &ERC20 {
        <UniswapV3Pool as LiquidityPool<P>>::token1(&self.0)
    }

    async fn verify_health(
        &self,
        provider: Arc<P>,
        block_number: BlockNumber,
    ) -> anyhow::Result<bool> {
        <UniswapV3Pool as LiquidityPool<P>>::verify_health(&self.0, provider, block_number).await
    }

    async fn update_with_provider(
        &self,
        provider: P,
        block: BlockId,
    ) -> Result<(), alloy::contract::Error> {
        <UniswapV3Pool as LiquidityPool<P>>::update_with_provider(&self.0, provider, block).await
    }

    fn create_payload(
        &self,
        recipient: Address,
        token_in: Address,
        amount: U256,
        extra: Vec<u8>,
    ) -> Vec<u8> {
        <UniswapV3Pool as LiquidityPool<P>>::create_payload(
            &self.0, recipient, token_in, amount, extra,
        )
    }
}
