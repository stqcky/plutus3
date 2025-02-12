use petgraph::visit::{
    EdgeRef as _, IntoNodeIdentifiers as _, NodeIndexable as _, VisitMap as _, Visitable as _,
};
use plutus_defi_erc20::ERC20;
use plutus_evm::revm::primitives::Address;

use crate::InnerTokenGraph;

#[derive(Debug, Clone)]
pub struct Step {
    pub token0: ERC20,
    pub token1: ERC20,
    pub pool: Address,
}

pub fn bellman_ford(graph: &InnerTokenGraph, source: Address) -> Vec<Vec<Step>> {
    let mut distance = vec![f64::INFINITY; graph.node_count()];

    let source = graph
        .node_indices()
        .find(|n| graph[*n].address == source)
        .unwrap();

    distance[source.index()] = 0.0;

    let mut predecessor = vec![None; graph.node_count()];
    let mut edge_used = vec![None; graph.node_count()];

    let ix = |i| graph.to_index(i);

    for _ in 1..graph.node_count() {
        let mut updated = false;

        for i in graph.node_identifiers() {
            for edge in graph.edges(i) {
                let j = edge.target();
                let w = edge.weight();

                if distance[ix(i)] + w.weight < distance[ix(j)] {
                    distance[ix(j)] = distance[ix(i)] + w.weight;
                    predecessor[ix(j)] = Some(i);
                    edge_used[ix(j)] = Some(w.pool);

                    updated = true;
                }
            }
        }

        if !updated {
            break;
        }
    }

    let mut paths = vec![];

    'outer: for i in graph.node_identifiers() {
        let mut path = vec![];
        for edge in graph.edges(i) {
            let j = edge.target();
            let w = edge.weight();

            if distance[ix(i)] + w.weight < distance[ix(j)] {
                // Step 3: negative cycle found
                let start = j;
                let mut node = start;
                let mut visited = graph.visit_map();
                path.push(start);
                // Go backward in the predecessor chain
                loop {
                    let ancestor = match predecessor[ix(node)] {
                        Some(predecessor_node) => predecessor_node,
                        None => node, // no predecessor, self cycle
                    };
                    // We have only 2 ways to find the cycle and break the loop:
                    // 1. start is reached
                    if ancestor == start {
                        path.push(ancestor);
                        break;
                    }
                    // 2. some node was reached twice
                    else if visited.is_visited(&ancestor) {
                        // Drop any node in path that is before the first ancestor
                        let pos = path
                            .iter()
                            .position(|&p| p == ancestor)
                            .expect("we should always have a position");
                        path = path[pos..path.len()].to_vec();
                        path.push(ancestor);

                        break;
                    }

                    // None of the above, some middle path node
                    path.push(ancestor);
                    visited.visit(ancestor);
                    node = ancestor;
                }
                // We are done here
                paths.push(path);
                continue 'outer;
            }
        }
    }

    paths
        .into_iter()
        .map(|mut path| {
            path.reverse();
            path.windows(2)
                .map(|pair| Step {
                    token0: graph[pair[0]].clone(),
                    token1: graph[pair[1]].clone(),
                    pool: edge_used[ix(pair[1])].unwrap(),
                })
                .collect()
        })
        .collect()

    // // tracing::info!("{path:?}");
    // if !path.is_empty() {
    //     path.reverse();
    //
    //     Some(
    //         path.windows(2)
    //             .map(|pair| Step {
    //                 token0: graph[pair[0]].clone(),
    //                 token1: graph[pair[1]].clone(),
    //                 pool: edge_used[ix(pair[1])].unwrap(),
    //             })
    //             .collect(),
    //     )
    // } else {
    //     None
    // }
}
