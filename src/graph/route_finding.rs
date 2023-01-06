use std::{collections::{BinaryHeap, HashMap, VecDeque}, cmp::Ordering};

use super::Graph;

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    node: u128,
    dist: u32
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.dist.cmp(&self.dist).then_with(|| self.node.cmp(&other.node))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Perform dijkstra's algorithm to find the shortest path between two nodes
pub fn find_route(graph: &Graph, source: u128, dest: u128) -> Vec<u128> {
    let mut distances = HashMap::new();
    let mut prev = HashMap::new();
    let mut heap = BinaryHeap::new();

    distances.entry(source).and_modify(|e| *e = 0).or_insert(0);
    prev.entry(source).and_modify(|v| *v = source).or_insert(source);

    heap.push(State {
        node: source,
        dist: distances[&source]
    });

    while let Some(State { node, dist }) = heap.pop() {
        // Found path
        if node == dest {
            break;
        }

        let cost = *distances.entry(node).or_insert(u32::MAX);

        // Better way already exists
        if dist > cost {
            continue;
        }

        for edge in graph.get_adjacency()[&node].iter() {
            let (e_start, e_end) = (graph.get_edgelist()[edge].start_id, graph.get_edgelist()[edge].end_id);

            let next = State {
                node: if e_start == node { e_end } else { e_start },
                dist: dist + graph.get_edgelist()[edge].length as u32
            };

            let next_cost = *distances.entry(next.node).or_insert(u32::MAX);
            if next.dist < next_cost {
                heap.push(next);
                distances.entry(next.node)
                    .and_modify(|v| *v = next.dist)
                    .or_insert(next.dist);
                
                prev.entry(next.node).and_modify(|v| *v = node).or_insert(node);
            }
        }
    }

    let mut path = Vec::new();
    // let mut dist = 0;
    let mut prev_node = dest;

    loop {
        path.push(prev_node);
    
        if prev.contains_key(&prev_node) && prev_node != source {
            prev_node = prev[&prev_node];
        } else {
            break;
        }
    }

    path
}

// approx distance (straight line) between two nodes
pub fn find_distance(graph: &Graph, source: &u128, dest: &u128) -> u32 {
    let src = graph.get_nodelist()[source].point;
    let dest = graph.get_nodelist()[dest].point;

    (f64::abs(src.0 - dest.0).powi(2) + f64::abs(src.1 - dest.1).powi(2)).sqrt() as u32
}

pub fn best_first_route(source: u128, mut nodes: Vec<u128>, graph: &Graph) -> Vec<u128> {
    println!("\tsource: {}", source);
    println!("\tnodes: {:?}", nodes);
    // println!("nodes valid {:?}", nodes.iter().all(|n| graph.get_nodelist().contains_key(n)));

    nodes.sort_by(|a, b| {
        find_distance(graph, &source, a).cmp(&find_distance(graph, &source, b))
    });

    let dest = *nodes.last().unwrap();
    
    let mut route = vec![source];
    
    while !nodes.is_empty() {
        nodes.sort_by(|a, b| {
            find_distance(graph, &dest, a).cmp(&find_distance(graph, &dest, b))
        });
        // println!("closest node {:?}", nodes.first().unwrap());
        route.push(*nodes.first().unwrap());
        nodes.remove(0);
        // println!("remaning nodes {:?}", nodes);
    }
    // route.push(source);
    // route.reverse();

    route
}

pub fn route_length(route: &VecDeque<u128>, graph: &Graph) -> u32 {
    let mut length = 0;
    for i in 0..route.len() - 1 {
        
        let node = &graph.get_nodelist()[&route[i]];
        let edge = graph.get_adjacency()[&node.id].iter().find(|e| {
            let edge = &graph.get_edgelist()[*e];
            edge.start_id == route[i] && edge.end_id == route[i + 1] || edge.start_id == route[i + 1] && edge.end_id == route[i]
        }).unwrap();
        let edge_data = &graph.get_edgelist()[edge];
        
        length = length + edge_data.length as u32;
    }
    length
}

pub fn closest_node(point: (f64, f64), graph: &Graph) -> u128 {
    let mut closest = 0;
    let mut dist = f64::MAX;

    for (id, node) in graph.get_nodelist().iter() {
        let d = (f64::abs(point.0 - node.point.0).powi(2) + f64::abs(point.1 - node.point.1).powi(2)).sqrt();
        if d < dist {
            dist = d;
            closest = *id;
        }
    }

    closest
}