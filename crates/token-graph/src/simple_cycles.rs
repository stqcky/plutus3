use std::hash::{Hash, Hasher};

use fxhash::{FxHashSet, FxHasher, FxHasher32};
use hashbrown::{HashMap, HashSet};
use petgraph::graph::NodeIndex;
use plutus_evm::alloy::primitives::Address;
use plutus_evm::revm::primitives::map::AddressMap;
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};

use crate::{InnerTokenGraph, Opportunity};

const MAX_HOPS: usize = 3;

pub fn create_simple_cycles(graph: &InnerTokenGraph) -> AddressMap<Vec<Vec<NodeIndex>>> {
    let cycles: AddressMap<Vec<Vec<NodeIndex>>> = HashMap::from_iter(
        graph
            .node_indices()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|node| {
                (
                    graph[node].address,
                    Vec::from_iter(
                        petgraph::algo::all_simple_paths::<Vec<_>, _>(
                            graph,
                            node,
                            node,
                            1,
                            Some(MAX_HOPS),
                        )
                        .collect::<HashSet<_>>(),
                    ),
                )
            })
            .collect::<Vec<_>>(),
    );

    let mut cycle_count = 0;

    for cycle in cycles.values() {
        cycle_count += cycle.len();
    }

    tracing::info!("cycle count: {cycle_count}");

    cycles
}

#[derive(Hash, PartialEq, Eq)]
struct CycleKey {
    fingerprint: u64,
    len: usize,
}

fn create_cycle_key(path: &[NodeIndex]) -> CycleKey {
    let mut hasher = FxHasher32::default();

    let mut nodes = path
        .iter()
        .collect::<FxHashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    nodes.sort_unstable();

    nodes.hash(&mut hasher);

    let fingerprint = hasher.finish();

    CycleKey {
        fingerprint,
        len: path.len(),
    }
}

fn create_opportunity_key(opportunity: &[(NodeIndex, NodeIndex, Address)]) -> CycleKey {
    let mut hasher = FxHasher32::default();

    let mut nodes = opportunity
        .iter()
        .map(|opportunity| opportunity.0)
        .collect::<FxHashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    nodes.sort_unstable();

    nodes.hash(&mut hasher);

    CycleKey {
        fingerprint: hasher.finish(),
        len: opportunity.len(),
    }
}

pub fn dedup_cycles(cycles: Vec<Vec<NodeIndex>>) -> Vec<Vec<NodeIndex>> {
    // let mut seen = FxHashSet::default();
    // let mut filtered = Vec::with_capacity(cycles.len());
    //
    // for cycle in cycles {
    //     let key = create_cycle_key(&cycle);
    //
    //     if seen.insert(key) {
    //         filtered.push(cycle);
    //     }
    // }
    //
    // filtered
    cycles
        .into_par_iter()
        .fold(
            || (FxHashSet::default(), Vec::new()),
            |(mut seen, mut filtered), cycle| {
                if seen.insert(create_cycle_key(&cycle)) {
                    filtered.push(cycle);
                }
                (seen, filtered)
            },
        )
        .map(|(_, filtered)| filtered)
        .flatten()
        .collect()
}

pub fn dedup_opportunities(
    opportunities: Vec<Vec<(NodeIndex, NodeIndex, Address)>>,
) -> Vec<Vec<(NodeIndex, NodeIndex, Address)>> {
    let mut seen = FxHashSet::default();
    let mut filtered = Vec::with_capacity(opportunities.len());

    for opportunity in opportunities {
        if seen.insert(create_opportunity_key(&opportunity)) {
            filtered.push(opportunity);
        }
    }

    filtered
}
