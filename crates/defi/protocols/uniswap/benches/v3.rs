use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{U256, address},
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
};
use criterion::{Bencher, BenchmarkId, Criterion, criterion_group, criterion_main};
use dotenvy_macro::dotenv;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_defi_protocols_uniswap::v3::pool::UniswapV3Pool;

fn swap_benchmark(c: &mut Criterion) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (provider, pool, token0, block) = runtime.block_on(async {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await
                    .unwrap()
                    .boxed(),
            ),
        );

        let block: BlockId = provider.get_block_number().await.unwrap().into();

        let pool = UniswapV3Pool::new_with_provider(
            address!("0xC24f7d8E51A64dc1238880BD00bb961D54cbeb29"),
            provider.clone(),
            block,
        )
        .await
        .unwrap();

        let token0 = pool.token0.clone();

        (provider.clone(), pool, token0, block)
    });

    let mut group = c.benchmark_group("v3_simulate_swap");

    let weth = address!("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1");

    let amounts = [
        token0.to_token_amount(0.0001),
        token0.to_token_amount(0.001),
        token0.to_token_amount(0.01),
        token0.to_token_amount(0.1),
        token0.to_token_amount(1.0),
        token0.to_token_amount(10.0),
        token0.to_token_amount(100.0),
        token0.to_token_amount(1000.0),
    ];

    for amount in amounts {
        group.bench_with_input(
            BenchmarkId::from_parameter(amount),
            &amount,
            |b: &mut Bencher, amount: &U256| {
                b.iter(|| pool.exact_input_of(token0.address, *amount, block));
            },
        );
    }

    group.finish();

    Ok(())
}

fn create_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (provider, block) = runtime.block_on(async {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await
                    .unwrap()
                    .boxed(),
            ),
        );

        let block: BlockId = provider.get_block_number().await.unwrap().into();

        (provider.clone(), block)
    });

    let mut group = c.benchmark_group("v3_pool_create");
    group.sample_size(10);

    group.bench_function("v3_pool_create", |b| {
        b.to_async(&runtime).iter(async || {
            UniswapV3Pool::new_with_provider(
                address!("0xC24f7d8E51A64dc1238880BD00bb961D54cbeb29"),
                provider.clone(),
                block,
            )
            .await
            .unwrap()
        })
    });
}

criterion_group!(benches, swap_benchmark);
criterion_main!(benches);
