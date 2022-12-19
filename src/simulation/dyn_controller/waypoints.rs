use std::{collections::{HashSet, HashMap, VecDeque}, sync::Arc};

use crate::graph::Graph;

use super::bus::{Bus, Status};

// Simple representation of waypoints and the actions available at each
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Waypoint {
    Passthrough(u128), // Passthrough -- Just have to visit with no other action
    Pickup(u128), // Pickup -- Pick up a passenger(s) waiting at this node
    Dropoff(u128) // Drop-Off -- Drop off passenger(s) on the bus for this node
}

impl Waypoint {
    pub fn node(&self) -> u128 {
        match self {
            Waypoint::Passthrough(node) => *node,
            Waypoint::Pickup(node) => *node,
            Waypoint::Dropoff(node) => *node
        }
    }
}

// Directed Tree Collection/Forest which models the necessary points for the bus to visit
// and also models the dependencies between pick-ups and drop-offs
#[derive(Debug, Default, Clone)]
pub struct DirForest {
    roots: HashSet<Waypoint>, // Waypoints which can be visited at the current time (no predecessors)
    children: HashMap<Waypoint, HashSet<Waypoint>> // Waypoints which can only be visited after a parent
}

impl DirForest {
    // Insert a new waypoint with a dependency into the forest
    pub fn insert(&mut self, parent: Option<Waypoint>, child: Waypoint) {
        match parent {
            Some(parent) => {
                self.children.get(&parent).unwrap().insert(child);
            },
            None => {
                self.roots.insert(child);
            }
        }
    }

    // Take a waypoint out of the tree because it's been visited and then update the dependencies
    pub fn visit_waypoint(&mut self, waypoint: Waypoint) {
        // Remove the waypoint from the roots
        self.roots.retain(|&x| x != waypoint);

        // put any child waypoints which had this dependence into the roots
        if let Some(children) = self.children.remove(&waypoint) {
            for child in children {
                self.roots.insert(child);
            }
        }
    }

    // Provides a mapping from top level waypoints to the actual nodes to visit including the set of actions to perform at each node
    pub fn get_root_nodes(&self) -> HashMap<u128, HashSet<Waypoint>> {
        let mut root_nodes = HashMap::new();

        for root in self.roots.iter() {
            root_nodes.entry(root.node()).or_insert(HashSet::new()).insert(*root);
        }

        root_nodes
    }
    
    pub fn get_roots(&self) -> &HashSet<Waypoint> {
        &self.roots
    }

    pub fn get_children(&self, parent: Waypoint) -> &HashSet<Waypoint> {
        self.children.get(&parent).unwrap_or(&HashSet::new())
    }
}

pub fn bus_waypoints(bus: &mut Bus) -> DirForest {
    let mut waypoints = DirForest::default();

    // Passengers on the bus only need to go to their destination
    for passenger in bus.passengers.iter() {
        waypoints.insert(None, Waypoint::Dropoff(passenger.dest_node));
    }

    // Passengers not yet on the bus (but in the assignment) need their source and their destination
    for source_node in bus.assignment.keys() {
        
        let mut single_valid_passenger = false;
        for passenger in bus.assignment.get(source_node).unwrap() {
            match passenger.status {
                Status::Waiting(_) | Status::TravelStart(_) => {
                    waypoints.insert(Some(Waypoint::Pickup(*source_node)), Waypoint::Dropoff(passenger.dest_node));
                    single_valid_passenger = true;
                },
                _ => () // Shouldn't add anything if passenger is on bus, or got off bus
            }
        }

        // Insert the source if there's a valid passenger at this source
        if single_valid_passenger {
            waypoints.insert(None, Waypoint::Pickup(*source_node));
        }
    }

    waypoints
}

// Create an ordering of waypoints to visit based of greedy best first search in the graph
// Starting Point == Locking Node of bus (next node it's travelling to)
pub fn create_ordering(starting_point: u128, waypoints: &mut DirForest, graph: Arc<Graph>) -> VecDeque<Waypoint> {
    let mut ordering = VecDeque::new();
    let mut last_position = starting_point;

    ordering.push_back(Waypoint::Passthrough(starting_point));

    while !waypoints.roots.is_empty() {
        let mut best_node = None;
        let mut best_distance = f64::MAX;

        // Finds next best node to travel to based on least squared distance. 
        // TODO: Consider improving this to A* or perhaps take into account number of dependencies
        // satisfied by visiting this node
        let nodes = waypoints.get_root_nodes();
        for (node, actions) in nodes.iter() {
            let distance = graph_distance(graph.clone(), last_position, *node);
            if distance < best_distance {
                best_distance = distance;
                best_node = Some((*node, actions));
            }
        }

        // Add best node to the route and remove it from the waypoinys
        let (node, actions) = best_node.unwrap();
        for action in actions {
            ordering.push_back(*action);
            waypoints.visit_waypoint(*action);
        }
        last_position = node;
    }

    ordering
}

// Currently just squared euclidean distance
// TODO: use a better norm?
pub fn graph_distance(graph: Arc<Graph>, source: u128, dest: u128) -> f64 {
    let source_pos = graph.get_nodelist().get(&source).unwrap().point;
    let dest_pos = graph.get_nodelist().get(&dest).unwrap().point;

    (source_pos.0 - dest_pos.0).powi(2) + (source_pos.1 - dest_pos.1).powi(2)
}

