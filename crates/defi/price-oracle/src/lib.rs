use alloy::eips::BlockId;
use alloy::primitives::U256;
use alloy::primitives::address;
use futures::StreamExt;
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::Protocol;
use plutus_defi_protocols_protocol::ProtocolFactory;
use std::{
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};

use alloy::{primitives::Address, providers::Provider};
use lru::LruCache;
use parking_lot::Mutex;
use plutus_defi_protocols_uniswap::v3::UniswapV3Protocol;

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

pub struct PriceOracle<P> {
    price_cache: Mutex<LruCache<Address, (f64, Instant)>>,
    provider: P,
    uniswap: UniswapV3Protocol,

    usdt: ERC20,
    usdc: ERC20,
    weth: ERC20,
}

const CACHE_TTL: Duration = Duration::from_secs(30);

impl<P: Provider + Clone + 'static> PriceOracle<P> {
    pub async fn new(provider: P) -> anyhow::Result<Arc<Self>> {
        let chain_id = provider.get_chain_id().await?;

        let oracle = Arc::new(Self {
            price_cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
            usdt: ERC20::new_with_provider(USDT, &provider).await?,
            usdc: ERC20::new_with_provider(USDC, &provider).await?,
            weth: ERC20::new_with_provider(WETH, &provider).await?,
            uniswap: <UniswapV3Protocol as ProtocolFactory<P>>::new(chain_id).unwrap(),
            provider,
        });

        oracle.cache_price(
            oracle.weth.address,
            oracle.clone().get_uniswap_v3_price(&oracle.weth).await,
        );
        Ok(oracle)
    }

    pub async fn get_price(self: Arc<Self>, token: &ERC20) -> f64 {
        if let Some(price) = self.clone().get_cached_price(token) {
            return price;
        }

        let price = Box::pin(self.clone().get_uniswap_v3_price(token)).await;
        self.cache_price(token.address, price);

        price
    }

    pub async fn get_eth_price(self: Arc<Self>, amount: U256) -> f64 {
        self.clone().get_price(&self.weth).await * self.weth.to_float_amount(amount)
    }

    fn get_cached_price(self: Arc<Self>, token: &ERC20) -> Option<f64> {
        let mut cache = self.price_cache.lock();

        match cache.get(&token.address) {
            Some((price, timestamp)) if timestamp.elapsed() < CACHE_TTL => Some(*price),
            Some((price, timestamp)) if timestamp.elapsed() < CACHE_TTL * 2 => {
                let oracle = self.clone();
                let token = token.to_owned();

                tokio::spawn(async move {
                    let price = oracle.clone().get_uniswap_v3_price(&token).await;
                    oracle.cache_price(token.address, price);
                });

                Some(*price)
            }
            _ => None,
        }
    }

    fn cache_price(&self, token: Address, price: f64) {
        let mut price_cache = self.price_cache.lock();
        price_cache.push(token, (price, Instant::now()));
    }

    async fn get_uniswap_v3_price(self: Arc<Self>, token: &ERC20) -> f64 {
        let usdt_value = if token.address != self.usdt.address {
            self.get_token_value(token, &self.usdt)
                .await
                .unwrap_or_default()
        } else {
            1.0
        };

        let usdc_value = if token.address != self.usdc.address {
            self.get_token_value(token, &self.usdc)
                .await
                .unwrap_or_default()
        } else {
            1.0
        };

        let weth_value = if token.address != self.weth.address {
            self.get_token_value(token, &self.weth)
                .await
                .unwrap_or_default()
                * self.clone().get_price(&self.weth).await
        } else {
            1.0
        };

        usdt_value.max(usdc_value).max(weth_value)
    }

    async fn get_token_value(
        &self,
        of_token: &ERC20,
        in_token: &ERC20,
    ) -> Result<f64, alloy::contract::Error> {
        let pools = self
            .uniswap
            .get_pools_with_provider(of_token.address, in_token.address, self.provider.clone())
            .await?;

        let amount = of_token.to_token_amount(1.0);

        let values = futures::future::join_all(
            futures::stream::iter(pools.into_iter())
                .map(|pool| async move {
                    pool.simulate_swap(
                        of_token.address,
                        amount,
                        BlockId::latest(),
                        self.provider.clone(),
                    )
                    .await
                })
                .collect::<Vec<_>>()
                .await,
        )
        .await;

        Ok(in_token.to_float_amount(values.into_iter().max().unwrap_or_default()))
    }
}
