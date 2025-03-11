use alloy::eips::BlockId;
use alloy::primitives::U256;
use alloy::primitives::address;
use alloy::{primitives::Address, providers::Provider};
use lru::LruCache;
use plutus_defi_erc20::ERC20;
use plutus_token_graph::TokenGraph;
use rayon::iter::IntoParallelIterator as _;
use rayon::iter::ParallelIterator as _;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

pub struct PriceOracle {
    price_cache: LruCache<Address, (f64, Instant)>,

    usdt: ERC20,
    usdc: ERC20,
    weth: ERC20,
}

const CACHE_TTL: Duration = Duration::from_secs(240);

// FIXME: this is badly designed
impl PriceOracle {
    pub async fn new<P: Provider + Clone + 'static>(
        tokens: Vec<ERC20>,
        token_graph: &TokenGraph<P>,
        provider: P,
    ) -> anyhow::Result<Self> {
        let mut oracle = Self {
            price_cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
            usdt: ERC20::new_with_provider(USDT, &provider).await?,
            usdc: ERC20::new_with_provider(USDC, &provider).await?,
            weth: ERC20::new_with_provider(WETH, &provider).await?,
        };

        let weth = oracle.weth.clone();
        let weth_price = oracle.get_uniswap_v3_price(&weth, token_graph);
        oracle.cache_price(oracle.weth.address, weth_price);

        oracle.prefetch_prices(tokens, token_graph);

        Ok(oracle)
    }

    fn prefetch_prices<P: Provider + Clone + 'static>(
        &mut self,
        tokens: Vec<ERC20>,
        token_graph: &TokenGraph<P>,
    ) {
        for token in tokens {
            let price = self.get_uniswap_v3_price(&token, token_graph);
            self.cache_price(token.address, price);
        }
    }

    pub fn get_price<P: Provider + Clone + 'static>(
        &mut self,
        token: &ERC20,
        token_graph: &TokenGraph<P>,
    ) -> f64 {
        if let Some(price) = self.get_cached_price(token, token_graph) {
            return price;
        }

        let price = self.get_uniswap_v3_price(token, token_graph);
        self.cache_price(token.address, price);

        price
    }

    pub fn get_eth_price<P: Provider + Clone + 'static>(
        &mut self,
        amount: U256,
        token_graph: &TokenGraph<P>,
    ) -> f64 {
        let weth = self.weth.clone();
        self.get_price(&weth, token_graph) * self.weth.to_float_amount(amount)
    }

    fn get_cached_price<P: Provider + Clone + 'static>(
        &mut self,
        token: &ERC20,
        token_graph: &TokenGraph<P>,
    ) -> Option<f64> {
        match self.price_cache.get(&token.address) {
            Some((price, timestamp)) if timestamp.elapsed() < CACHE_TTL => Some(*price),
            _ => {
                let token = token.to_owned();

                let price = self.get_uniswap_v3_price(&token, token_graph);
                self.cache_price(token.address, price);

                Some(price)
            }
        }
    }

    fn cache_price(&mut self, token: Address, price: f64) {
        self.price_cache.push(token, (price, Instant::now()));
    }

    fn get_uniswap_v3_price<P: Provider + Clone + 'static>(
        &mut self,
        token: &ERC20,
        token_graph: &TokenGraph<P>,
    ) -> f64 {
        let usdt_value = if token.address != self.usdt.address {
            self.get_token_value(token, &self.usdt, token_graph)
                .unwrap_or_default()
        } else {
            1.0
        };

        let usdc_value = if token.address != self.usdc.address {
            self.get_token_value(token, &self.usdc, token_graph)
                .unwrap_or_default()
        } else {
            1.0
        };

        let weth_value = if token.address != self.weth.address {
            let weth = self.weth.clone();
            let weth_price = self.get_price(&weth, token_graph);

            self.get_token_value(token, &self.weth, token_graph)
                .unwrap_or_default()
                * weth_price
        } else {
            1.0
        };

        usdt_value.max(usdc_value).max(weth_value)
    }

    fn get_token_value<P: Provider + Clone + 'static>(
        &self,
        of_token: &ERC20,
        in_token: &ERC20,
        token_graph: &TokenGraph<P>,
    ) -> Result<f64, alloy::contract::Error> {
        let pools: Vec<_> = token_graph
            .get_pools_between_tokens(of_token, in_token)
            .into_iter()
            .filter(|pool| pool.identifier() == "uniswap_v3")
            .collect();

        let amount = of_token.to_token_amount(1.0);

        let values = pools
            .into_par_iter()
            .map(|pool| pool.simulate_swap(of_token.address, amount, BlockId::latest()))
            .collect::<Vec<_>>();

        Ok(in_token.to_float_amount(values.into_iter().max().unwrap_or_default()))
    }
}
