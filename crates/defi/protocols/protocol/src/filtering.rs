use std::{marker::PhantomData, sync::Arc};

use alloy::{
    eips::BlockId,
    primitives::{Address, address, map::AddressMap},
    providers::Provider,
};
use futures::future;
use parking_lot::Mutex;
use plutus_defi_erc20::ERC20;
use tokio::sync::Semaphore;

use crate::{DiscoverableProtocol, pool::LiquidityPool};

pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
pub const USDC: Address = address!("af88d065e77c8cc2239327c5edb3a432268e5831");

pub const FILTER_TASK_LIMIT: usize = 50;

#[derive(Clone, Default)]
pub struct TokenValueCache {
    value_cache: Arc<Mutex<AddressMap<AddressMap<f64>>>>,
}

impl TokenValueCache {
    pub async fn get_value<P: Provider + Clone + 'static>(
        &self,
        of_token: &ERC20,
        in_token: &ERC20,
        protocols: Arc<Vec<Box<dyn DiscoverableProtocol<P>>>>,
        block: BlockId,
        provider: P,
    ) -> f64 {
        {
            let mut lock = self.value_cache.lock();
            let in_token_map = &mut *lock.entry(in_token.address).or_default();

            if in_token_map.contains_key(&of_token.address) {
                return in_token_map[&of_token.address];
            }
        }

        {
            let value = Self::get_best_value(of_token, in_token, protocols, block, provider).await;

            let mut lock = self.value_cache.lock();
            let in_token_map = &mut *lock.entry(in_token.address).or_default();
            in_token_map.insert(of_token.address, value);

            value
        }
    }

    async fn get_best_value<P: Provider + Clone + 'static>(
        of_token: &ERC20,
        in_token: &ERC20,
        protocols: Arc<Vec<Box<dyn DiscoverableProtocol<P>>>>,
        block: BlockId,
        provider: P,
    ) -> f64 {
        if of_token == in_token {
            return 1.0;
        }

        let pools = Self::get_pools(
            of_token.address,
            in_token.address,
            protocols,
            block,
            provider.clone(),
        )
        .await;

        let mut values = vec![];

        for pool in pools {
            values.push(pool.simulate_swap(of_token.address, of_token.to_token_amount(1.0), block));
        }

        let best_value = values.into_iter().max();

        in_token.to_float_amount(best_value.unwrap_or_default())
    }

    async fn get_pools<P: Provider + Clone + 'static>(
        token0: Address,
        token1: Address,
        protocols: Arc<Vec<Box<dyn DiscoverableProtocol<P>>>>,
        block: BlockId,
        provider: P,
    ) -> Vec<Box<dyn LiquidityPool<P>>> {
        let mut pools = vec![];

        for protocol in protocols.iter() {
            let Ok(protocol_pools) = protocol
                .get_pools_with_provider(token0, token1, block, provider.clone())
                .await
            else {
                continue;
            };

            pools.extend(protocol_pools);
        }

        pools
    }
}

#[derive(Clone)]
struct RequiredTokenValue {
    token: ERC20,
    value: f64,
}

impl RequiredTokenValue {
    pub fn new(token: ERC20, value: f64) -> Self {
        Self { token, value }
    }
}

pub struct PoolFilter<P> {
    required: Vec<RequiredTokenValue>,
    token_value_cache: TokenValueCache,
    _marker: PhantomData<P>,
}

impl<P: Provider + std::fmt::Debug + 'static + Clone> PoolFilter<P> {
    pub async fn new(
        usd_value: f64,
        provider: P,
        block: BlockId,
        protocols: Vec<Box<dyn DiscoverableProtocol<P>>>,
    ) -> anyhow::Result<Self> {
        let usdt = ERC20::new_with_provider(USDT, provider.clone()).await?;
        let usdc = ERC20::new_with_provider(USDC, provider.clone()).await?;
        let weth = ERC20::new_with_provider(WETH, provider.clone()).await?;

        let token_value_cache = TokenValueCache::default();

        let weth_value = token_value_cache
            .get_value(&weth, &usdc, protocols.into(), block, provider)
            .await;

        Ok(Self {
            required: vec![
                RequiredTokenValue::new(usdt, usd_value),
                RequiredTokenValue::new(usdc, usd_value),
                RequiredTokenValue::new(weth, usd_value / weth_value),
            ],
            token_value_cache,
            _marker: PhantomData::default(),
        })
    }

    pub async fn filter_pools(
        self,
        pools: Vec<Box<dyn LiquidityPool<P>>>,
        provider: P,
        protocols: Vec<Box<dyn DiscoverableProtocol<P>>>,
        block: BlockId,
    ) -> anyhow::Result<Vec<Box<dyn LiquidityPool<P>>>>
    where
        P: Clone,
    {
        let pools = pools
            .into_iter()
            .filter(|pool| pool.is_liquidity_valid())
            .collect::<Vec<_>>();

        let token_value_cache = self.token_value_cache;
        let required = Arc::new(self.required);
        let protocols = Arc::new(protocols);

        let semaphore = Arc::new(Semaphore::new(FILTER_TASK_LIMIT));
        let tasks: Vec<_> = pools
            .into_iter()
            .map(|pool| {
                let semaphore = semaphore.clone();
                let token_value_cache = token_value_cache.clone();
                let required = required.clone();
                let provider = provider.clone();
                let protocols = protocols.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire_owned().await.unwrap();

                    let (token0, token1) = pool.tokens();

                    let (locked0, locked1) = pool.tokens_locked();
                    let (locked0, locked1) = (
                        token0.to_float_amount(locked0),
                        token1.to_float_amount(locked1),
                    );

                    for token in required.iter() {
                        let value0 = token_value_cache
                            .get_value(
                                &token0,
                                &token.token,
                                protocols.clone(),
                                block,
                                provider.clone(),
                            )
                            .await;

                        let value1 = token_value_cache
                            .get_value(
                                &token1,
                                &token.token,
                                protocols.clone(),
                                block,
                                provider.clone(),
                            )
                            .await;

                        let total_value0 = value0 * locked0;
                        let total_value1 = value1 * locked1;

                        if total_value0 >= token.value && total_value1 >= token.value {
                            return Some(pool);
                        }
                    }

                    None
                })
            })
            .collect();

        Ok(future::try_join_all(tasks)
            .await?
            .into_iter()
            .filter_map(|x| x)
            .collect())
    }
}
