use std::{cell::RefCell, collections::VecDeque, fmt::Display, time::Instant};

use hashbrown::{HashMap, HashSet};
use petgraph::{
    Direction::Outgoing,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::{EdgeRef, IntoNodeReferences},
};
use plutus_evm::revm::primitives::Address;

use crate::{InnerTokenGraph, WeightedPool};

type LineTokenGraph = DiGraph<(Address, Address), WeightedPoolAddress>;

#[derive(Clone, Copy)]
pub struct WeightedPoolAddress {
    pool: Address,
    weight: f64,
}

pub fn create_line_graph(
    graph: &InnerTokenGraph,
) -> (LineTokenGraph, HashMap<Address, Vec<NodeIndex>>) {
    let mut line_graph = LineTokenGraph::default();
    let mut edge_to_node = vec![NodeIndex::new(0); graph.edge_count()];
    let mut token_to_nodes = HashMap::<Address, Vec<NodeIndex>>::new();

    // Add nodes to line graph and build token_to_nodes mapping
    for edge in graph.edge_references() {
        let source = graph[edge.source()].address;
        let target = graph[edge.target()].address;
        let node = line_graph.add_node((source, target));
        edge_to_node[edge.id().index()] = node;
        token_to_nodes.entry(source).or_default().push(node);
    }

    // Add edges to line graph, avoiding cycles (i,j) -> (j,i)
    for edge1 in graph.edge_references() {
        let edge1_source = graph[edge1.source()].address;
        let edge1_target = graph[edge1.target()].address;
        let target_node = edge1.target();

        for edge2 in graph.edges_directed(target_node, Outgoing) {
            let edge2_source = graph[edge2.source()].address;
            let edge2_target = graph[edge2.target()].address;

            // Skip edges that would create a cycle (i,j) -> (j,i) where i != j
            if edge2_target == edge1_source && edge1_source != edge1_target {
                continue;
            }

            let line_node1 = edge_to_node[edge1.id().index()];
            let line_node2 = edge_to_node[edge2.id().index()];
            line_graph.add_edge(line_node1, line_node2, WeightedPoolAddress {
                pool: edge2.weight().pool,
                weight: edge2.weight().weight,
            });
        }
    }

    (line_graph, token_to_nodes)
}

pub fn add_extra_node_and_link(
    line_graph: &mut LineTokenGraph,
    token_to_nodes: &HashMap<Address, Vec<NodeIndex>>,
    graph: &InnerTokenGraph,
    source: Address,
) -> Vec<EdgeIndex> {
    let extra_node = line_graph.add_node((Address::ZERO, source));
    let neighbor_link_vertices = token_to_nodes
        .get(&source)
        .map(|nodes| {
            nodes
                .iter()
                .map(|&ni| (ni, line_graph[ni]))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let original_source_node = graph
        .node_indices()
        .find(|i| graph[*i].address == source)
        .unwrap();

    let mut edges_to_add = vec![];
    for (neighbor_node, (_, target_token)) in neighbor_link_vertices {
        if let Some(edge) = graph
            .edges_directed(original_source_node, Outgoing)
            .find(|e| graph[e.target()].address == target_token)
        {
            edges_to_add.push((extra_node, neighbor_node, WeightedPoolAddress {
                pool: edge.weight().pool,
                weight: edge.weight().weight,
            }));
        }
    }

    edges_to_add
        .into_iter()
        .map(|(src, dst, weight)| line_graph.add_edge(src, dst, weight))
        .collect()
}

pub struct Step {
    pub token0: Address,
    pub token1: Address,
    pub pool: Address,
}

impl Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {} @ {}", self.token0, self.token1, self.pool)
    }
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub fn mmbf(
    graph: &InnerTokenGraph,
    source: Address,
    mut line_graph: LineTokenGraph,
    token_to_nodes: HashMap<Address, Vec<NodeIndex>>,
) -> Option<Vec<Step>> {
    let now = Instant::now();
    let added_edges = add_extra_node_and_link(&mut line_graph, &token_to_nodes, graph, source);
    if added_edges.is_empty() {
        return None;
    }

    let extra_node = line_graph
        .node_indices()
        .find(|ni| line_graph[*ni] == (Address::ZERO, source))
        .unwrap();

    let mut distance = vec![f64::INFINITY; line_graph.node_count()];
    let mut path = vec![vec![]; line_graph.node_count()];
    let mut used_edge = vec![None; line_graph.node_count()];
    let mut visited_tokens = vec![HashSet::new(); line_graph.node_count()];

    // Initialize extra node
    distance[extra_node.index()] = 0.0;
    visited_tokens[extra_node.index()].insert(Address::ZERO);
    visited_tokens[extra_node.index()].insert(source);

    let mut in_queue = VecDeque::new();
    in_queue.push_back(extra_node);
    let mut in_queue_flags = vec![false; line_graph.node_count()];
    in_queue_flags[extra_node.index()] = true;

    // SPFA algorithm
    while let Some(u) = in_queue.pop_front() {
        in_queue_flags[u.index()] = false;

        for edge in line_graph.edges_directed(u, Outgoing) {
            let v = edge.target();
            let (_, l) = line_graph[v];
            let weight = edge.weight().weight;

            let is_included = visited_tokens[u.index()].contains(&l);

            if distance[u.index()] + weight < distance[v.index()] && (!is_included || l == source) {
                let new_distance = distance[u.index()] + weight;
                if new_distance < distance[v.index()] {
                    distance[v.index()] = new_distance;
                    path[v.index()] = path[u.index()].clone();
                    path[v.index()].push(v);
                    used_edge[v.index()] = Some(*edge.weight());

                    visited_tokens[v.index()] = visited_tokens[u.index()].clone();
                    visited_tokens[v.index()].insert(l);

                    if !in_queue_flags[v.index()] {
                        in_queue.push_back(v);
                        in_queue_flags[v.index()] = true;
                    }
                }
            }
        }
    }

    // Rest of the code to extract the path remains the same
    // [Previous code for extracting the path and converting to Steps]
    // ...

    let mut d_token = HashMap::<Address, f64>::default();
    let mut p_token = HashMap::<Address, Vec<NodeIndex>>::default();

    for (u, v) in distance.iter().enumerate() {
        let k = line_graph[NodeIndex::new(u)];
        let t = k.1;

        // tracing::info!("t: {t:?}, v: {v}");

        if !d_token.contains_key(&t) || *v < d_token[&t] {
            d_token.insert(t, *v);
            p_token.insert(t, path[u].clone());
        }
    }

    // tracing::info!("d_token: {d_token:#?}");

    for (k, v) in &d_token {
        if *v >= 0.0 {
            p_token.remove(k);
        }
    }

    let path = p_token.get(&source)?;

    Some(
        path.into_iter()
            .map(|u| {
                let node = line_graph[*u];
                Step {
                    token0: node.0,
                    token1: node.1,
                    pool: used_edge[u.index()].unwrap().pool,
                }
            })
            .collect(),
    )
}
