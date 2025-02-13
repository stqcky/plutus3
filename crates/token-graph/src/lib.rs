use std::{collections::BTreeMap, sync::Arc, time::Instant};

use futures::future;
use hashbrown::{HashMap, HashSet};

use petgraph::{
    dot::Dot,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::IntoNodeReferences,
};
use plutus_defi_erc20::ERC20;
use plutus_defi_protocols_protocol::pool::LiquidityPool;
use plutus_evm::{
    EVM,
    alloy::providers::Provider,
    revm::primitives::{Address, U256, address, map::AddressMap},
};
use tokio::task::spawn_blocking;

mod bellman_ford;
mod finding;
mod spfa;

type InnerTokenGraph = DiGraph<ERC20, WeightedPool>;

pub struct TokenGraph<P: Provider> {
    pub pools: Vec<Box<dyn LiquidityPool<P>>>,
    pool_map: AddressMap<usize>,

    // just store addresses instead of whole objects?
    graph: InnerTokenGraph,
    pool_edge_map: AddressMap<(EdgeIndex, EdgeIndex)>,
    token_map: AddressMap<NodeIndex>,

    amount: f64,
}

#[derive(Debug, Clone)]
pub struct WeightedPool {
    pub pool: Address,
    pub weight: f64,
}

pub struct Step<P: Provider> {
    pub token0: ERC20,
    pub token1: ERC20,
    pub pool: Box<dyn LiquidityPool<P>>,
}

impl std::fmt::Display for WeightedPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.pool, self.weight)
    }
}

impl<P: Provider> TokenGraph<P> {
    pub fn new(mut pools: Vec<Box<dyn LiquidityPool<P>>>, amount: f64, evm: &mut EVM<P>) -> Self {
        let mut graph = InnerTokenGraph::new();

        let tokens = get_unique_tokens(&pools);

        let pool_map = AddressMap::from_iter(
            pools
                .iter()
                .enumerate()
                .map(|(i, pool)| (pool.address(), i)),
        );

        let token_map = init_nodes(tokens, &mut graph);

        let pool_edge_map = init_edges(&mut pools, amount, &mut graph, &token_map, evm);
        tracing::info!("node count: {}", graph.node_count());
        tracing::info!("edge count: {}", graph.edge_count());

        let dot = Dot::with_config(&graph, &[]);
        std::fs::write("graph", format!("{dot}")).unwrap();

        Self {
            pools,
            pool_map,
            graph,
            pool_edge_map,
            token_map,
            amount,
        }
    }

    pub fn apply_state(&mut self, traces: Vec<AddressMap<HashMap<U256, U256>>>, evm: &mut EVM<P>) {
        for trace in traces {
            for (address, changes) in trace {
                let Some(pool_index) = self.pool_map.get(&address) else {
                    continue;
                };

                let pool = self.pools.get_mut(*pool_index).unwrap();
                pool.apply_storage_changes(changes);

                let (edge0, edge1) = self.pool_edge_map[&address];

                self.graph.edge_weight_mut(edge0).unwrap().weight =
                    calculate_edge(pool.as_mut(), true, self.amount, evm).2;

                self.graph.edge_weight_mut(edge1).unwrap().weight =
                    calculate_edge(pool.as_mut(), true, self.amount, evm).2;
            }
        }
    }

    async fn mmbf(&self) -> Vec<Vec<Step<P>>> {
        let now = Instant::now();
        let (line_graph, token_to_nodes) = finding::create_line_graph(&self.graph);
        tracing::info!("line graph created in {:?}", now.elapsed());
        tracing::info!("line graph edges: {}", line_graph.edge_count());

        let graph = Arc::new(self.graph.clone());

        let now = Instant::now();
        let tasks = self.graph.node_references().into_iter().map(|(_, token)| {
            let line_graph = line_graph.clone();
            let graph = graph.clone();
            let token = token.address;
            let token_to_nodes = token_to_nodes.clone();

            spawn_blocking(move || finding::mmbf(&graph, token, line_graph, token_to_nodes))
        });
        tracing::info!("tasks created in {:?}", now.elapsed());

        let now = Instant::now();
        let opportunities = future::join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .filter_map(|x| x)
            .map(|x| {
                x.into_iter()
                    .map(|step| Step {
                        token0: self.graph[self.token_map[&step.token0]].clone(),
                        token1: self.graph[self.token_map[&step.token1]].clone(),
                        pool: self.pools[self.pool_map[&step.pool]].clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        tracing::info!("scanned in {:?}", now.elapsed());

        opportunities
    }

    async fn bellman_ford(&self) -> Vec<Vec<Step<P>>> {
        let graph = Arc::new(self.graph.clone());

        let opportunities: Vec<_> = bellman_ford::bellman_ford(
            &graph,
            address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
        )
        .into_iter()
        .map(|steps| {
            steps
                .into_iter()
                .map(|step| Step {
                    token0: step.token0,
                    token1: step.token1,
                    pool: self.pools[self.pool_map[&step.pool]].clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect();

        opportunities
    }

    fn spfa(&self) -> Vec<Vec<Step<P>>> {
        let result = spfa::find_cycle(&self.graph);

        // let Some(result) = result else {
        //     return vec![];
        // };

        result
            .into_iter()
            .map(|path| {
                path.into_iter()
                    .map(|step| Step {
                        token0: step.token0,
                        token1: step.token1,
                        pool: self.pools[self.pool_map[&step.pool]].clone(),
                    })
                    .collect()
            })
            .collect()

        // vec![
        //     result
        //         .into_iter()
        //         .map(|step| Step {
        //             token0: step.token0,
        //             token1: step.token1,
        //             pool: self.pools[self.pool_map[&step.pool]].clone(),
        //         })
        //         .collect(),
        // ]
    }

    fn simle_paths(&self) {
        for node in self.graph.node_indices() {
            let paths: Vec<_> =
                petgraph::algo::all_simple_paths::<Vec<_>, _>(&self.graph, node, node, 1, Some(4))
                    .collect();
        }
    }

    pub async fn find_opportunities(&self) -> Vec<Vec<Step<P>>> {
        // petgraph::algo
        // self.mmbf().await
        // self.bellman_ford().await
        let now = Instant::now();
        let a = self.spfa();
        // tracing::info!("{:?}", now.elapsed());
        a

        // let now = Instant::now();
        // self.simle_paths();
        // tracing::info!("{:?}", now.elapsed());
        //
        // vec![]
    }
}

fn get_unique_tokens<P: Provider>(pools: &[Box<dyn LiquidityPool<P>>]) -> Vec<ERC20> {
    let mut tokens = HashSet::new();

    for pool in pools {
        let (token0, token1) = pool.tokens();

        tokens.insert(token0);
        tokens.insert(token1);
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

fn init_edges<P: Provider>(
    pools: &mut [Box<dyn LiquidityPool<P>>],
    amount: f64,
    graph: &mut InnerTokenGraph,
    token_map: &AddressMap<NodeIndex>,
    evm: &mut EVM<P>,
) -> AddressMap<(EdgeIndex, EdgeIndex)> {
    let mut edge_map = AddressMap::default();

    for pool in pools {
        let (edge0, edge1) = (
            calculate_edge(pool.as_mut(), true, amount, evm),
            calculate_edge(pool.as_mut(), false, amount, evm),
        );

        let edge0 = graph.add_edge(token_map[&edge0.0], token_map[&edge0.1], WeightedPool {
            pool: pool.address(),
            weight: edge0.2,
        });

        let edge1 = graph.add_edge(token_map[&edge1.0], token_map[&edge1.1], WeightedPool {
            pool: pool.address(),
            weight: edge1.2,
        });

        edge_map.insert(pool.address(), (edge0, edge1));
    }

    edge_map
}

fn calculate_edge<P: Provider>(
    pool: &mut dyn LiquidityPool<P>,
    zero_for_one: bool,
    amount: f64,
    evm: &mut EVM<P>,
) -> (Address, Address, f64) {
    let (token0, token1) = pool.tokens();

    let (token_in, token_out) = if zero_for_one {
        (token0, token1)
    } else {
        (token1, token0)
    };

    let amount_in = token_in.to_token_amount(amount);
    let amount_out = pool.simulate_swap(token_in.address, amount_in, evm);
    let weight = -(f64::from(amount_out) / f64::from(amount_in)).log10();

    (token_in.address, token_out.address, weight)
}
