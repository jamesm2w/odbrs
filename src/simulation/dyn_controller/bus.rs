use std::{collections::{VecDeque, HashMap}, sync::Arc};

use chrono::{DateTime, Utc};
use eframe::epaint::{Shape, Stroke, Color32, pos2};
use rand::Rng;

use crate::{graph::{Graph}, simulation::{Agent, default_display}};

use super::waypoints::{bus_waypoints, create_ordering, Waypoint};

pub enum Action {
    Wait, // Stay at this node for this tick
    Continue, // Start moving to next node
    Stop // Stop Moving forever(?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentElement {
    PreGenerated, // Haven't placed this agent yet
    Node(u128),
    Edge{ edge: u128, prev_node: u128}
}

impl Default for CurrentElement {
    fn default() -> Self {
        CurrentElement::PreGenerated
    }
}

/// Reflects the current status of the demand which represents an individual passenger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Generated, // This demand has just been generated
    TravelStart(u8), // This demand has started travelling towards the starting node -- countdown of ticks travelling
    Waiting(u8), // This demand is waiting at the starting node for a bus -- timer of ticks waited
    OnBus(DateTime<Utc>), // This demand is on a bus travelling -- timestamp of when got on
    TavelDest(u8), // This demand has reached the destination node and is travelling to the destination pos -- countdown of ticks travelling
    Expired // This demand has gone through the full cycle and is now expired
}

impl Default for Status {
    fn default() -> Self {
        Status::Generated
    }
}

/// Represents the passenger of a generated demand which is on the bus
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Passenger {
    pub source_pos: (f64, f64),
    pub source_node: u128,
    pub dest_pos: (f64, f64),
    pub dest_node: u128,
    pub timeframe: DateTime<Utc>,
    pub status: Status
}

#[derive(Default)]
pub struct Bus {
    
    pub graph: Arc<Graph>, // Reference to the graph this agent is operating on

    pub agent_id: u8, // ID of this agent
    pub max_capacity: u8, // Maximum capacity of the agent/bus
    pub rem_capacity: u8, // Remaining capacity of the agent/bus
    
    pub passengers: Vec<Passenger>, // List of passengers on the bus (current assignment/solution)
    pub assignment: HashMap<u128, Vec<Passenger>>, // Future passengers to be added to the bus (future assignment/solution)
    
    pub path_waypoints: VecDeque<Waypoint>, // List of important nodes which should be in full path // TODO: look into a hashset or some ordering of waypoints
    pub path_full: VecDeque<u128>, // List of nodes to visit to complete the assignment

    pub current_pos: (f64, f64), // Current position of the agent
    pub current_el: CurrentElement, // Current edge the agent is on
    pub next_node: u128, // Next node the agent is travelling to; the "locking node"
}

impl Agent for Bus {

    fn get_graph(&self) -> Arc<Graph> {
        self.graph.clone()
    }

    fn get_current_element(&self) -> CurrentElement {
        self.current_el
    }

    fn get_position(&self) -> (f64, f64) {
        self.current_pos
    }

    fn get_next_node(&self) -> u128 {
        self.next_node
    }

    fn display(&self) -> eframe::epaint::Shape {
        let mut shapes = vec![];
        let base_shape = default_display(self);

        let mut demand = self.path_waypoints.iter().map(|node| {
            let node_data = self.graph.get_nodelist().get(&node.node()).expect("Node not found");
            Shape::circle_filled(pos2(node_data.point.0 as _, node_data.point.1 as _), 3.0, Color32::LIGHT_YELLOW)
        }).collect::<Vec<_>>();

        let path = Shape::line(self.path_full.iter().map(|node| {
            let node_data = self.graph.get_nodelist().get(node).expect("Node not found");
            pos2(node_data.point.0 as _, node_data.point.1 as _)
        }).collect(), Stroke::new(2.0, Color32::LIGHT_BLUE));

        shapes.append(&mut demand);
        shapes.push(path);
        shapes.push(base_shape);

        Shape::Vec(shapes)
    }
}

/// has some set of passengers with their source, destination and time window
/// has a route of stops to visit
/// 
impl Bus {
    
    fn handle_node(&mut self, node: u128) -> Action {
        let passengers_at_this_node = self.assignment.get(&node).unwrap_or(&vec![]);
        // loop waiting passengers and add to bus
        for passenger in passengers_at_this_node {
            if self.rem_capacity > 0 {
                self.passengers.push(passenger.clone());
                self.rem_capacity -= 1;
            }
        }

        // loop passengers on bus and remove if they have reached their destination
        for passenger in self.passengers.iter() {
            if passenger.dest_node == node {
                self.rem_capacity += 1;
            }
        }
        self.passengers.retain(|passenger| passenger.dest_node != node);

        // TODO: check if there are timeline constraints which means the bus needs to wait at this node
        Action::Continue
    }

    pub fn can_assign_more(&self) -> bool {
        self.rem_capacity > 0
    }

    // TODO: abstract out random initialisation to another function?
    pub fn new(graph: Arc<Graph>, max_capacity: u8, id: u8) -> Self {
        let random_index = rand::thread_rng().gen_range(0..=graph.get_nodelist().len() - 1);
        let random_node = graph.get_nodelist().keys().nth(random_index).unwrap();
        let adjacency = graph.get_adjacency().get(random_node).unwrap();
        let random_edge_i = rand::thread_rng().gen_range(0..=adjacency.len() - 1);
        let edge = adjacency.get(random_edge_i).unwrap();
        let edge_data = &graph.get_edgelist()[edge];
        let agent_pos = graph.get_nodelist()[random_node].point;
        let locking_node = if edge_data.start_id == *random_node { edge_data.end_id } else { edge_data.start_id };
        
        Bus {
            graph: graph.clone(),
            agent_id: id,
            max_capacity,
            current_el: CurrentElement::Edge { edge: *edge, prev_node: *random_node },
            current_pos: agent_pos,
            next_node: locking_node,
            ..Default::default()
        }
    }

    // Constructive Function
        // Assign a new passenger
        // Create a list of waypoints
        // Order waypoints and construct full path inbetween
    // Destructive Function
        // Find passengers we can remove
        // Remove a random amount of them
        // Update waypoinys and paths?
    
    pub fn constructive_dont_mutate(&self, passenger: Passenger) {}

    pub fn add_passenger_to_assignment(&mut self, passenger: Passenger) {
        self.assignment.entry(passenger.source_node).or_insert_with(|| Vec::new()).push(passenger);
    }

    pub fn constructive(&mut self, passenger: Passenger) {
        self.add_passenger_to_assignment(passenger);

        // Uses GreedyBFS to find an ordering of the waypoints for the bus
        let mut path = create_ordering(
            self.next_node, 
            &mut bus_waypoints(self), 
            self.graph.clone()
        );
        self.path_waypoints = path;
        
        // TODO: Create the full path between waypoints
        // self.create_path();
    }

    // TODO: maybe optimise loops in this mostly AI generated function
    pub fn destructive(&mut self) -> Vec<Passenger> {
        // loop throught assignent and remove 50% which aren't currently passengers
        let mut removed = vec![];
        for (node, passengers) in self.assignment.iter_mut() {
            let mut rng = rand::thread_rng();
            let mut to_remove = vec![];
            
            for passenger in passengers.iter() {
                if !self.passengers.contains(passenger) {
                    if rng.gen_bool(0.5) {
                        to_remove.push(passenger.clone());
                    }
                }
            }

            for passenger in to_remove {
                passengers.retain(|p| p != &passenger);
                removed.push(passenger);
            }
        }
        removed
    }

    // pub fn create_path(&m ut self) {
    //     let source = self.locking_node;
    //     let nodes = self.assignment.iter().map(|e| *e).collect::<Vec<_>>();
    //     let waypoints = route_finding::best_first_route(source, nodes, &self.graph);
    //     let mut prev = source;
    //     self.path = VecDeque::new();

    //     for pt in waypoints {
    //         if pt == prev {
    //             continue;
    //         }
    //         // println!("Route to {:?} from {:?}", pt, prev);
    //         let subroute = route_finding::find_route(&self.graph, prev, pt);
    //         // println!("Subroute {:?}", subroute);
    //         self.path.extend(subroute.into_iter().rev().skip(1)); // Reverse and skip the source to stop dupe
    //         prev = *self.path.back().unwrap();
    //     }

    //     self.path.push_front(source);
    // }

    // pub fn assign_demand(&mut self, (src, dest, dateime): (u128, u128, DateTime<Utc>)) {
    //     self.assignment.insert(src);
    //     self.assignment.insert(dest);

    //     self.create_path();
    // }

    // // Construct a route with the given requests
    // pub fn greedy_construct_route(&mut self, requests: Vec<(u128, DateTime<Utc>)>) {
    //     // find origin from requests then try to assign such that feasible

    // }

    // pub fn route_with_node(&self, new_req: u128, dest_req: u128, graph: &Graph) -> VecDeque<u128> {
    //     let source = self.locking_node;
    //     let mut nodes = Vec::from_iter(self.assignment.iter().map(|a| *a));
    //     nodes.push(new_req);
    //     nodes.push(dest_req);
    //     println!("\treq src: {:?}\t req dest: {:?}", new_req, dest_req);
    //     // println!("nodes: {:?}", nodes.iter().map(|x| graph.get_nodelist().contains_key(x)).collect::<Vec<_>>());
        
    //     let waypoints = route_finding::best_first_route(source, nodes, &graph);
        
    //     // println!("waypoints: {:?}", waypoints);
    //     let mut route = VecDeque::new();
    //     let mut prev = source;

    //     for pt in waypoints {
    //         if pt == prev {
    //             continue;
    //         }
    //         // println!("Route to {:?} from {:?}", pt, prev);
    //         let subroute = route_finding::find_route(&self.graph, prev, pt);
    //         // println!("Subroute {:?}", subroute);
    //         route.extend(subroute.into_iter().rev().skip(1)); // Reverse and skip the source to stop dupe
    //         prev = *route.back().unwrap();
    //     }

    //     route
    // }

    // // Destory part of the route. Stochasticly choose some destinations to remove (also can't remove anything locked)
    // // remove a random amount of passengers assigned and return them (back to the central controller)
    // // destroy the path so that we have to rebuild it after a constructive phase
    // pub fn destroy_route(&mut self) -> VecDeque<Demand> {
        
    //     self.path.retain(|node| {
    //         rand::thread_rng().gen_bool(0.5) || self.current_destinations.contains(node)
    //     });
        
    //     VecDeque::new()
    // }

    /// for every stop s in route b
    ///     if arrival time at stop s < current time
    ///         locking point = s
    ///     else 
    ///         break
    /// if locking point is not last scheduled stop in route then
    ///     locking point += 1
    /// if locking point is not last scheduled stop in route -1 then
    ///     for ever stop s between lockin gpoint and last scheduled stop in route - 1 do
    ///     if someone gets on bus at stop s then
    ///         bool breaknow = true
    ///         if departure time at stop s - stop time - walking time < current time then
    ///             breaknow = false;
    ///             lockpoint += 1
    ///         if breaknow then 
    ///             break out of loop
    /// return the lockpoint (index of the route)

    pub fn move_self(&mut self) {
        // No need to move agent if no path to follow
        if self.path_full.len() == 0 {
            return; // No path to follow
        }

        let mut move_distance = 10.0;
        while move_distance > 0.0 {
            // Id of the edge we are currently on, or need to move along
            let moving_edge_id = match self.current_el {
                CurrentElement::PreGenerated => unreachable!("The agent is trying to move before it has been generated"),
                CurrentElement::Edge { edge, prev_node } => {
                    edge
                }, 
                CurrentElement::Node(node) => {
                    let next_node = self.next_node;
                    *self.graph.get_adjacency()[&node].iter().find(|edge| {
                        let edge_data = &self.graph.get_edgelist()[edge];
                        edge_data.start_id == next_node || edge_data.end_id == next_node
                    }).unwrap()
                }
            };
            let moving_edge_data = &self.graph.get_edgelist()[&moving_edge_id];
            
            let next_node = self.next_node;
            let next_node_data = &self.graph.get_nodelist()[&next_node];

            let line = if next_node_data.point == *moving_edge_data.points.first().unwrap() {
                moving_edge_data.points.clone()
            } else {
                moving_edge_data.points.iter().rev().map(|x| *x).collect()
            };

            let mut has_moved = false;
            for i in 0..line.len() - 1 {
                let segment_start = line[i];
                let segment_end = line[i+1];
                if point_on_linesegment(self.current_pos, &segment_start, &segment_end) {
                    let distance_remaining = distance(self.current_pos, segment_end);
                    
                    if move_distance > distance_remaining {
                        self.current_pos = segment_end;
                        move_distance -= distance_remaining;
                        has_moved = true;
                    } else {
                        let dir = normalise((segment_end.0 - segment_start.0, segment_end.1 - segment_start.1));
                        self.current_pos = (self.current_pos.0 + dir.0 * move_distance, self.current_pos.1 + dir.1 * move_distance);
                        return;
                    }
                }
            }
        
            if !has_moved {
                // println!("Didn't move this iteration distance left {:?}", distance_to_move);
                return;
            }

            if has_moved && move_distance > 0.0 {
                // We have moved the full distance to move along the current edge
                // Move to the next edge
                let current_node = self.path_full.pop_front(); // Also should be self.next_node

                self.current_el = match self.path_full.front() {
                    // There is a next node on the path
                    Some(&next_node) => {
                        let cur_edge = *self.graph.get_adjacency()[&self.next_node].iter().find(|e| {
                            let edge = &self.graph.get_edgelist()[*e];
                            edge.end_id == next_node || edge.start_id == next_node
                        }).unwrap(); // TODO: fix panic here

                        self.next_node = next_node;
                        CurrentElement::Edge { edge: cur_edge, prev_node: self.next_node }
                    },
                    // No Next Node on the path
                    None => CurrentElement::Node(next_node)
                };
            }
        }
    }

}

// Based on collision detection for a point and a line. Point is on a line if the distance to each point is equal to lenght
fn point_on_linesegment(pos: (f64, f64), start: &(f64, f64), end: &(f64, f64)) -> bool {
    let d1 = distance(pos, *start);
    let d2 = distance(pos, *end);
    let line_len = distance(*start, *end);
    let buffer = 0.1;

    if d1 + d2 >= line_len - buffer && d1 + d2 <= line_len + buffer {
        true
    } else {
        false
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let xs = (a.0 - b.0).abs();
    let ys = (a.1 - b.1).abs();
    xs.hypot(ys)
}

fn normalise(a: (f64, f64)) -> (f64, f64) {
    let mag = ((a.0).powi(2) + (a.1).powi(2)).sqrt();
    (a.0 / mag, a.1 / mag)
}

        //  Moves itself one tick along the path towards its next path point
        // // self.agent_pos = (self.agent_pos.0+1.0, self.agent_pos.1)
        // let mut distance_to_move = 10.0;//13.4112 * 60.0; // TODO: switch to tickspeed * velocity
        // while distance_to_move > 0.0 {
        //     // prev_node, cur_edge, agent_pos
        //     let edge = &self.graph.get_edgelist()[&self.cur_edge];
            
        //     let next_path_node = self.path.front();
        //     let next_node = match next_path_node {
        //         // Go towards the next path node if it's connected else go to the locking point node
        //         Some(node) => if edge.start_id == *node || edge.end_id == *node { node } else { &self.locking_node },
        //         None => &self.locking_node
        //     };
        //     // let next_node = self.path.front().unwrap_or(&self.locking_node); //if self.prev_node == edge.start_id { &edge.end_id } else { &edge.start_id });
        //     let next_node_data = &self.graph.get_nodelist()[next_node];
            
        //     let line = if next_node_data.point == *edge.points.first().unwrap() {
        //         edge.points.iter().rev().collect::<Vec<_>>() // if the "start" is the destination flip the line
        //     } else {
        //         edge.points.iter().collect::<Vec<_>>()
        //     };

        //     let mut has_moved = false;
            
        //     for i in 0..line.len() - 1 {
        //         let segment_start = line[i];
        //         let segment_end = line[i+1];

        //         if point_on_linesegment(self.agent_pos, segment_start, segment_end) {
        //             // println!("ON LINE! pos: {:?}\tsegment start: {:?}\tsegment end: {:?}", self.agent_pos, segment_start, segment_end);
        //             let dist_remaining = distance(self.agent_pos, *segment_end);
        //             // println!("dist remaining: {:?}", dist_remaining);
        //             if distance_to_move > dist_remaining {
        //                 // Set agent_pos to segment_end and subtract distance moved from total distance to move
        //                 self.agent_pos = *segment_end;
        //                 distance_to_move -= dist_remaining;
        //                 // println!("Moved {} units [To end of segment]", dist_remaining);
        //                 has_moved = true;
        //             } else {
        //                 // move agent_pos along the line segment by distance_to_move
        //                 let dir = normalise((segment_end.0 - segment_start.0, segment_end.1 - segment_start.1));
        //                 self.agent_pos = (self.agent_pos.0 + dir.0 * distance_to_move, self.agent_pos.1 + dir.1 * distance_to_move);
        //                 return;
        //             }
        //         }
        //     }
