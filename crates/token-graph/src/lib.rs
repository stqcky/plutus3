use calculation::{CalculatedOpportunity, calculate_opportunity};
use core::f64;
use futures::future;
use hashbrown::{HashMap, HashSet};
use rayon::prelude::*;
use simple_cycles::{create_simple_cycles, dedup_cycles};
use std::{cmp::Ordering, collections::BTreeMap, sync::Arc, time::Instant};

use petgraph::{
    data::DataMap,
    dot::Dot,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::{EdgeRef, IntoNodeReferences},
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{
    EVM,
    alloy::{eips::BlockId, primitives::BlockNumber, providers::Provider},
    revm::primitives::{
        Address, U256, address,
        map::{AddressMap, AddressSet},
    },
};
use tokio::{sync::Semaphore, task::spawn_blocking};

pub mod calculation;
pub mod simple_cycles;

type InnerTokenGraph = DiGraph<ERC20, WeightedPool>;

pub struct TokenGraph<P: Provider> {
    pub pools: Vec<Box<dyn LiquidityPool<P>>>,
    pool_map: AddressMap<usize>,

    // just store addresses instead of whole objects?
    graph: InnerTokenGraph,
    pool_edge_map: AddressMap<(EdgeIndex, EdgeIndex)>,
    token_map: AddressMap<NodeIndex>,

    cycles: AddressMap<Vec<Vec<NodeIndex>>>,

    amount: f64,
}

#[derive(Debug, Clone)]
pub struct WeightedPool {
    pub pool: Address,
    pub weight: f64,
}

#[derive(Clone)]
pub struct OpportunityLeg<P: Provider> {
    pub token0: ERC20,
    pub token1: ERC20,
    pub pool: Box<dyn LiquidityPool<P>>,
}

pub type Step<P> = OpportunityLeg<P>;

pub type Opportunity<P> = Vec<OpportunityLeg<P>>;

impl std::fmt::Display for WeightedPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.pool, self.weight)
    }
}

impl<P: Provider + Clone + 'static> TokenGraph<P> {
    pub async fn new(
        pools: Vec<Box<dyn LiquidityPool<P>>>,
        amount: f64,
        block: BlockId,
        provider: P,
    ) -> anyhow::Result<Self> {
        let mut graph = InnerTokenGraph::new();

        let tokens = get_unique_tokens(&pools);

        let pool_map = AddressMap::from_iter(
            pools
                .iter()
                .enumerate()
                .map(|(i, pool)| (pool.address(), i)),
        );

        let token_map = init_nodes(tokens, &mut graph);

        let pool_edge_map = init_edges(
            pools.clone(),
            amount,
            &mut graph,
            &token_map,
            block,
            provider,
        )
        .await?;

        tracing::info!("node count: {}", graph.node_count());
        tracing::info!("edge count: {}", graph.edge_count());

        let dot = Dot::with_config(&graph, &[]);
        std::fs::write("graph", format!("{dot}")).unwrap();

        Ok(Self {
            pools,
            pool_map,
            cycles: create_simple_cycles(&graph),
            graph,
            pool_edge_map,
            token_map,
            amount,
        })
    }

    pub async fn apply_state(
        &mut self,
        traces: Vec<AddressMap<HashMap<U256, U256>>>,
        provider: P,
        block: BlockId,
    ) -> AddressSet
    where
        P: Clone,
    {
        let mut changed_addresses = AddressSet::default();
        let mut affected_tokens = AddressSet::default();

        for trace in &traces {
            changed_addresses.extend(trace.keys());
        }

        for address in changed_addresses {
            let Some(pool_index) = self.pool_map.get(&address) else {
                continue;
            };

            let pool = self.pools.get_mut(*pool_index).unwrap();
            pool.update_with_provider(provider.clone(), block)
                .await
                .unwrap();
            // pool.apply_storage_changes(changes);

            let (edge0, edge1) = self.pool_edge_map[&address];

            let tokens = pool.tokens();
            let (token0, token1) = (tokens.0.to_owned(), tokens.1.to_owned());

            affected_tokens.insert(token0.address);
            affected_tokens.insert(token1.address);

            self.graph.edge_weight_mut(edge0).unwrap().weight = calculate_edge(
                pool.as_mut(),
                &token0,
                &token1,
                self.amount,
                block,
                provider.clone(),
            )
            .await
            .pool
            .weight;

            self.graph.edge_weight_mut(edge1).unwrap().weight = calculate_edge(
                pool.as_mut(),
                &token1,
                &token0,
                self.amount,
                block,
                provider.clone(),
            )
            .await
            .pool
            .weight;
        }

        affected_tokens
    }

    pub async fn find_opportunities(
        &self,
        target_tokens: AddressSet,
        block: BlockId,
        provider: P,
    ) -> anyhow::Result<Vec<CalculatedOpportunity<P>>> {
        let now = Instant::now();
        let opportunities = self.simple_finding(target_tokens);
        // tracing::info!("simple_finding took {:?}", now.elapsed());
        // tracing::info!("opportunity count: {}", opportunities.len());

        let now = Instant::now();
        let semaphore = Arc::new(Semaphore::new(24));
        let tasks: Vec<_> = opportunities
            .into_iter()
            .map(|opportunity| {
                let provider = provider.clone();
                let semaphore = semaphore.clone();

                tokio::spawn(async move {
                    let _permit = semaphore.acquire_owned().await.unwrap();
                    calculate_opportunity(opportunity, block, provider).await
                })
            })
            .collect();

        let opportunities: Vec<_> = future::try_join_all(tasks)
            .await?
            .into_iter()
            .filter_map(|x| x)
            .collect();
        // tracing::info!("calculations took {:?}", now.elapsed());

        Ok(opportunities)
    }

    pub async fn find_uncalculated_opportunities(
        &self,
        target_tokens: AddressSet,
    ) -> Vec<Opportunity<P>> {
        self.simple_finding(target_tokens)
    }

    fn simple_finding(&self, target_tokens: AddressSet) -> Vec<Vec<OpportunityLeg<P>>> {
        let cycles: Vec<_> = dedup_cycles(
            target_tokens
                .iter()
                .flat_map(|address| self.cycles[address].clone())
                .collect(),
        );

        // tracing::info!("cycle count: {}", cycles.len());

        let mut opportunities: Vec<_> = cycles
            .par_iter()
            .filter_map(|cycle| {
                let mut steps = vec![];
                let mut profit = 1.0;

                for pair in cycle.windows(2) {
                    let (node0, node1) = (pair[0], pair[1]);

                    let edges = self.graph.edges_connecting(node0, node1);

                    let best_weight = edges
                        .max_by(|&a, &b| a.weight().weight.partial_cmp(&b.weight().weight).unwrap())
                        .unwrap()
                        .weight();

                    profit *= best_weight.weight;

                    steps.push((node0, node1, best_weight.pool));
                }

                if profit > 1.05 {
                    Some((profit, steps))
                } else {
                    None
                }
            })
            .collect();

        opportunities.sort_by(|a, b| match PartialOrd::partial_cmp(&b.0, &a.0) {
            Some(ordering) => ordering,
            None => unreachable!(),
        });

        let opportunities: Vec<_> = opportunities.into_iter().map(|a| a.1).collect();

        let opportunities: Vec<_> = opportunities
            .into_iter()
            .filter(|opportunity| {
                for step in opportunity {
                    let (token0, token1) = (self.graph[step.0].address, self.graph[step.1].address);

                    if target_tokens.contains(&token0) || target_tokens.contains(&token1) {
                        return true;
                    }
                }

                false
            })
            .collect();

        let opportunities = opportunities
            .into_iter()
            .map(|steps| {
                steps
                    .into_iter()
                    .map(|step| OpportunityLeg {
                        token0: self.graph[step.0].clone(),
                        token1: self.graph[step.1].clone(),
                        pool: self.pools[self.pool_map[&step.2]].clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        opportunities
    }
}

fn get_unique_tokens<P: Provider>(pools: &[Box<dyn LiquidityPool<P>>]) -> Vec<ERC20> {
    let mut tokens = HashSet::new();

    for pool in pools {
        let (token0, token1) = pool.tokens();

        tokens.insert(token0.to_owned());
        tokens.insert(token1.to_owned());
    }

    Vec::from_iter(tokens)
}

fn init_nodes(tokens: Vec<ERC20>, graph: &mut InnerTokenGraph) -> AddressMap<NodeIndex> {
    AddressMap::from_iter(
        tokens
            .into_iter()
            .map(|token| (token.address, graph.add_node(token))),
    )
}

async fn init_edges<P: Provider + Clone + 'static>(
    pools: Vec<Box<dyn LiquidityPool<P>>>,
    amount: f64,
    graph: &mut InnerTokenGraph,
    token_map: &AddressMap<NodeIndex>,
    block: BlockId,
    provider: P,
) -> anyhow::Result<AddressMap<(EdgeIndex, EdgeIndex)>> {
    let mut edge_map = AddressMap::default();

    let tasks: Vec<_> = pools
        .into_iter()
        .map(|mut pool| {
            let provider = provider.clone();

            tokio::spawn(async move {
                let tokens = pool.tokens();
                let (token0, token1) = (tokens.0.to_owned(), tokens.1.to_owned());

                (
                    calculate_edge(
                        pool.as_mut(),
                        &token0,
                        &token1,
                        amount,
                        block,
                        provider.clone(),
                    )
                    .await,
                    calculate_edge(
                        pool.as_mut(),
                        &token1,
                        &token0,
                        amount,
                        block,
                        provider.clone(),
                    )
                    .await,
                )
            })
        })
        .collect();

    let edge_pairs: Vec<_> = future::try_join_all(tasks).await?.into_iter().collect();

    for (edge0, edge1) in edge_pairs {
        let pool_address = edge0.pool.pool;

        let edge0 = graph.add_edge(
            token_map[&edge0.token0],
            token_map[&edge0.token1],
            edge0.pool,
        );

        let edge1 = graph.add_edge(
            token_map[&edge1.token0],
            token_map[&edge1.token1],
            edge1.pool,
        );

        edge_map.insert(pool_address, (edge0, edge1));
    }

    Ok(edge_map)
}

struct CalculatedEdge {
    token0: Address,
    token1: Address,
    pool: WeightedPool,
}

async fn calculate_edge<P: Provider>(
    pool: &mut dyn LiquidityPool<P>,
    token0: &ERC20,
    token1: &ERC20,
    amount: f64,
    block: BlockId,
    provider: P,
) -> CalculatedEdge {
    let amount_in = token0.to_token_amount(amount);
    let amount_out = pool
        .simulate_swap(token0.address, amount_in, block, provider)
        .await;
    // let weight = -(f64::from(amount_out) / f64::from(amount_in)).log10();
    let weight = f64::from(amount_out) / f64::from(amount_in);

    CalculatedEdge {
        token0: token0.address,
        token1: token1.address,
        pool: WeightedPool {
            pool: pool.address(),
            weight,
        },
    }
}
