use std::sync::Arc;

use alloy::{
    eips::BlockId,
    primitives::{address, map::AddressSet},
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
};
use criterion::{Criterion, criterion_group, criterion_main};
use dotenvy_macro::dotenv;
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::{Protocol, ProtocolFactory, registry::ProtocolRegistry};
use plutus_defi_protocols_uniswap::v3::UniswapV3Protocol;
use plutus_storage::Storage;
use plutus_token_graph::{
    Opportunity, OpportunityLeg, TokenGraph, calculation::calculate_opportunity,
};

async fn create_opportunity<P: Provider + Clone + 'static>(
    provider: P,
    block: BlockId,
) -> Opportunity<P> {
    let uniswap =
        <UniswapV3Protocol as ProtocolFactory<P>>::new(provider.get_chain_id().await.unwrap())
            .unwrap();

    vec![
        OpportunityLeg {
            token0: ERC20::new_with_provider(
                address!("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
                provider.clone(),
            )
            .await
            .unwrap(),
            token1: ERC20::new_with_provider(
                address!("0x4D22e37Eb4d71D1acc5f4889a65936D2a44A2f15"),
                provider.clone(),
            )
            .await
            .unwrap(),
            pool: uniswap
                .create_pool_with_provider(
                    address!("0x9FCBC372A15E96E85ACF37C84C91DA21dA005398"),
                    provider.clone(),
                    block,
                )
                .await
                .unwrap(),
        },
        OpportunityLeg {
            token0: ERC20::new_with_provider(
                address!("0x4D22e37Eb4d71D1acc5f4889a65936D2a44A2f15"),
                provider.clone(),
            )
            .await
            .unwrap(),
            token1: ERC20::new_with_provider(
                address!("0xadf5DD3E51bF28aB4F07e684eCF5d00691818790"),
                provider.clone(),
            )
            .await
            .unwrap(),
            pool: uniswap
                .create_pool_with_provider(
                    address!("0x553F37D829cD36C050A51BB5dDf26bD1Ec5A57dD"),
                    provider.clone(),
                    block,
                )
                .await
                .unwrap(),
        },
        OpportunityLeg {
            token0: ERC20::new_with_provider(
                address!("0xadf5DD3E51bF28aB4F07e684eCF5d00691818790"),
                provider.clone(),
            )
            .await
            .unwrap(),
            token1: ERC20::new_with_provider(
                address!("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
                provider.clone(),
            )
            .await
            .unwrap(),
            pool: uniswap
                .create_pool_with_provider(
                    address!("0x44c40a6544f29f331720E989Cd2724306b21c0d0"),
                    provider.clone(),
                    block,
                )
                .await
                .unwrap(),
        },
    ]
}

fn criterion_benchmark(c: &mut Criterion) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (provider, pools, token_graph, block, opportunity) = runtime.block_on(async {
        let provider = Arc::new(
            ProviderBuilder::new().with_recommended_fillers().on_client(
                ClientBuilder::default()
                    .ipc(dotenv!("IPC_PROVIDER").to_string().into())
                    .await
                    .unwrap()
                    .boxed(),
            ),
        );

        let protocol_registry = Arc::new(
            ProtocolRegistry::new(provider.clone())
                .await
                .unwrap()
                .with::<UniswapV3Protocol>()
                .unwrap(),
        );

        let block = provider.get_block_number().await.unwrap();

        let storage = Storage::new().await.unwrap();

        let pools = protocol_registry
            .get_cached_filtered_pools(&storage, block.into())
            .await
            .unwrap();

        let token_graph = TokenGraph::new(pools.clone(), 0.001, block.into(), provider.clone())
            .await
            .unwrap();

        // let opportunity: Opportunity<>

        (
            provider.clone(),
            pools,
            token_graph,
            block,
            create_opportunity(provider.clone(), block.into()).await,
        )
    });

    let target_tokens =
        AddressSet::from_iter([address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1")]);

    let target_pools =
        AddressSet::from_iter(pools.iter().map(|pool| pool.address()).collect::<Vec<_>>());

    // c.bench_function("find_uncalculated_opportunities", |b| {
    //     b.iter(|| token_graph.simple_finding(target_tokens.clone(), target_pools.clone()))
    // });

    // c.bench_function("calculate_opportunity", |b| {
    //     b.to_async(&runtime)
    //         .iter(|| calculate_opportunity(opportunity.clone(), block.into(), provider.clone()))
    // });

    // let mut group = c.benchmark_group("small");
    // group.sample_size(10);

    c.bench_function("find_opportunities", |b| {
        b.to_async(&runtime).iter(|| {
            token_graph.find_opportunities(
                target_tokens.clone(),
                target_pools.clone(),
                block.into(),
                provider.clone(),
            )
        })
    });

    Ok(())
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
