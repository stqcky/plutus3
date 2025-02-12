use hashbrown::HashSet;
use petgraph::prelude::*;
use petgraph::visit::{EdgeRef, VisitMap as _, Visitable};
use plutus_defi_erc20::ERC20;
use plutus_evm::revm::primitives::Address;
use std::collections::VecDeque;

use crate::InnerTokenGraph;

pub struct Step {
    pub token0: ERC20,
    pub token1: ERC20,
    pub pool: Address,
}

pub fn find_cycle(graph: &InnerTokenGraph) -> Vec<Vec<Step>> {
    let node_count = graph.node_count();

    let start_node = NodeIndex::new(0);
    // let mut max_profit = None;

    let (predecessor, enqueued) = spfa(graph, start_node);

    let mut paths = HashSet::new();

    for node in graph.node_indices() {
        if enqueued[node.index()] > node_count {
            if let Some((cycle_edges, profit)) = trace_cycle(graph, &predecessor, node) {
                paths.insert(cycle_edges);
                // match max_profit {
                //     Some((_, current_profit)) if profit < current_profit => {
                //         max_profit = Some((cycle_edges, profit));
                //     }
                //     None => max_profit = Some((cycle_edges, profit)),
                //     _ => {}
                // }
            }
        }
    }

    // tracing::info!("paths: {}", paths.len());

    paths
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|edge| {
                    let (u, v) = graph.edge_endpoints(edge).unwrap();
                    let pool = graph[edge].pool;

                    Step {
                        token0: graph[u].clone(),
                        token1: graph[v].clone(),
                        pool,
                    }
                })
                .collect()
        })
        .collect()

    // max_profit.map(|(edges, _)| {
    //     edges
    //         .into_iter()
    //         .map(|edge| {
    //             let (u, v) = graph.edge_endpoints(edge).unwrap();
    //             let pool = graph[edge].pool;
    //
    //             Step {
    //                 token0: graph[u].clone(),
    //                 token1: graph[v].clone(),
    //                 pool,
    //             }
    //         })
    //         .collect()
    // })
}

fn spfa(graph: &InnerTokenGraph, source: NodeIndex) -> (Vec<Option<EdgeIndex>>, Vec<usize>) {
    let node_count = graph.node_count();

    let mut distance = vec![f64::INFINITY; node_count];
    let mut predecessor = vec![None; node_count];

    let mut in_queue = vec![false; node_count];
    let mut enqueued = vec![0; node_count];
    let mut queue = VecDeque::from([source]);

    distance[source.index()] = 0.0;
    in_queue[source.index()] = true;
    enqueued[source.index()] = 1;

    while let Some(u) = queue.pop_front() {
        in_queue[u.index()] = false;

        for edge in graph.edges(u) {
            let v = edge.target();
            let weight = edge.weight();

            if distance[u.index()] + weight.weight < distance[v.index()] {
                distance[v.index()] = distance[u.index()] + edge.weight().weight;
                predecessor[v.index()] = Some(edge.id());

                if !in_queue[v.index()] {
                    enqueued[v.index()] += 1;

                    if enqueued[v.index()] > node_count {
                        continue;
                    }

                    if !queue.is_empty() && distance[v.index()] < distance[queue[0].index()] {
                        queue.push_front(v);
                    } else {
                        queue.push_back(v);
                    }

                    in_queue[v.index()] = true;
                }
            }
        }
    }

    (predecessor, enqueued)
}

fn trace_cycle(
    graph: &InnerTokenGraph,
    predecessor: &[Option<EdgeIndex>],
    start: NodeIndex,
) -> Option<(Vec<EdgeIndex>, f64)> {
    let mut node = start;
    let mut visited = graph.visit_map();
    let mut path = vec![];

    loop {
        let edge_index = predecessor[node.index()]?;
        let ancestor = graph.edge_endpoints(edge_index)?.0;

        if ancestor == start {
            path.push(edge_index);
            break;
        } else if visited.is_visited(&ancestor) {
            let pos = path
                .iter()
                .position(|&p| graph.edge_endpoints(p).unwrap().1 == ancestor)
                .unwrap();

            path = path[pos..path.len()].to_vec();
            path.push(edge_index);

            break;
        }

        path.push(edge_index);
        visited.visit(ancestor);
        node = ancestor;
    }

    path.reverse();

    let profit = path.iter().map(|&edge| graph[edge].weight).sum();

    Some((path, profit))
}
