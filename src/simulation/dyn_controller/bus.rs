use std::{collections::{VecDeque, HashSet}, sync::Arc, hash::Hash};

use chrono::{DateTime, Utc};
use eframe::epaint::{Shape, Stroke, Color32, pos2};
use rand::Rng;

use crate::{graph::{Graph, route_finding}, simulation::{Agent, demand::Demand}};


#[derive(Default)]
pub struct Bus {
    pub id: u8,
    graph: Arc<Graph>,
    max_capacity: u8,
    
    current_destinations: HashSet<u128>, // current passengers destinations which we can't remove from the solution
    // TODO: group nodes s.t. can't remove a dest without removing src or a src without the destination
    // Also keep track of the passengers so we know when to pickup and drop-off
    // but also need a decent way to access all passengers at a node so it's not a linear search?
    pub assignment: HashSet<u128>, // current assignment/solution

    passengers: VecDeque<(u128, DateTime<Utc>)>, // List of passengers (destination, latest arrival time window)
    path: VecDeque<u128>, // List of nodes to visit to complete the assignment

    locking_node: u128, // basically start of the path

    prev_node: u128,
    cur_edge: u128,
    agent_pos: (f64, f64)
}

impl Agent for Bus {

    fn get_graph(&self) -> Arc<Graph> {
        self.graph.clone()
    }

    fn get_display_information(&self) -> ((f64, f64), u128, u128) {
        (self.agent_pos, self.cur_edge, self.prev_node)
    }

    fn display(&self) -> eframe::epaint::Shape {
        let (position, edge, prev_node) = self.get_display_information();
        let graph = self.get_graph();
        let edge_data = graph.get_edgelist().get(&edge).expect("Edge not found");
        let node_data = graph.get_nodelist().get(&prev_node).expect("Node not found");

        let locking_data = graph.get_nodelist().get(&self.locking_node).expect("Locking Node not Found");
              
        Shape::Vec(vec![
            Shape::circle_stroke( pos2(position.0 as _, position.1 as _), 3.0, Stroke::new(2.0, Color32::YELLOW)),
            Shape::circle_stroke( pos2(node_data.point.0 as _, node_data.point.1 as _), 2.0, Stroke::new(1.0, Color32::LIGHT_GREEN)),
            Shape::line(edge_data.points.iter().map(|&(x, y)| {
                pos2(x as _, y as _)
            }).collect(), Stroke::new(1.0, Color32::LIGHT_GREEN)),
            Shape::line(self.path.iter().map(|node| {
                let node_data = graph.get_nodelist().get(node).expect("Node not found");
                pos2(node_data.point.0 as _, node_data.point.1 as _)
            }).collect(), Stroke::new(2.0, Color32::LIGHT_BLUE)),
            Shape::circle_stroke( pos2(locking_data.point.0 as _, locking_data.point.1 as _), 2.5, Stroke::new(1.5, Color32::RED))
        ])
    }
}

/// has some set of passengers with their source, destination and time window
/// has a route of stops to visit
/// 
impl Bus {

    pub fn can_assign_more(&self) -> bool {
        true // TODO: actually do a check here lmao
    }

    pub fn new(graph: Arc<Graph>, max_capacity: u8, id: u8) -> Self {
        let random_index = rand::thread_rng().gen_range(0..=graph.get_nodelist().len() - 1);
        let random_node = graph.get_nodelist().keys().nth(random_index).unwrap();
        let adjacency = graph.get_adjacency().get(random_node).unwrap();
        let random_edge_i = rand::thread_rng().gen_range(0..=adjacency.len() - 1);
        let edge = adjacency.get(random_edge_i).unwrap();
        let edge_data = &graph.get_edgelist()[edge];

        let agent_pos = graph.get_nodelist()[random_node].point;
        let locking_node = if edge_data.start_id == *random_node { edge_data.end_id } else { edge_data.start_id };
        Bus { id, max_capacity, graph: graph.clone(), prev_node: *random_node, cur_edge: *edge, agent_pos, locking_node, ..Default::default() }
    }

    pub fn pickup_passengers(&mut self, passenger: (u128, DateTime<Utc>)) {
        self.passengers.push_back(passenger);
        self.current_destinations.insert(passenger.0);
    }

    pub fn dropoff_passengers(&mut self, node: u128) {
        self.passengers.retain(|(dest, _)| *dest != node);
        self.current_destinations.remove(&node);
    }

    pub fn create_path(&mut self) {
        let source = self.locking_node;
        let nodes = self.assignment.iter().map(|e| *e).collect::<Vec<_>>();
        let waypoints = route_finding::best_first_route(source, nodes, &self.graph);
        let mut prev = source;
        self.path = VecDeque::new();

        for pt in waypoints {
            if pt == prev {
                continue;
            }
            // println!("Route to {:?} from {:?}", pt, prev);
            let subroute = route_finding::find_route(&self.graph, prev, pt);
            // println!("Subroute {:?}", subroute);
            self.path.extend(subroute.into_iter().rev().skip(1)); // Reverse and skip the source to stop dupe
            prev = *self.path.back().unwrap();
        }
    }

    pub fn assign_demand(&mut self, (src, dest, dateime): (u128, u128, DateTime<Utc>)) {
        self.assignment.insert(src);
        self.assignment.insert(dest);

        self.create_path();
    }

    // Construct a route with the given requests
    pub fn greedy_construct_route(&mut self, requests: Vec<(u128, DateTime<Utc>)>) {
        // find origin from requests then try to assign such that feasible

    }

    pub fn route_with_node(&self, new_req: u128, dest_req: u128, graph: &Graph) -> VecDeque<u128> {
        let source = self.locking_node;
        let mut nodes = Vec::from_iter(self.assignment.iter().map(|a| *a));
        nodes.push(new_req);
        nodes.push(dest_req);
        println!("\treq src: {:?}\t req dest: {:?}", new_req, dest_req);
        // println!("nodes: {:?}", nodes.iter().map(|x| graph.get_nodelist().contains_key(x)).collect::<Vec<_>>());
        
        let waypoints = route_finding::best_first_route(source, nodes, &graph);
        
        // println!("waypoints: {:?}", waypoints);
        let mut route = VecDeque::new();
        let mut prev = source;

        for pt in waypoints {
            if pt == prev {
                continue;
            }
            // println!("Route to {:?} from {:?}", pt, prev);
            let subroute = route_finding::find_route(&self.graph, prev, pt);
            // println!("Subroute {:?}", subroute);
            route.extend(subroute.into_iter().rev().skip(1)); // Reverse and skip the source to stop dupe
            prev = *route.back().unwrap();
        }

        route
    }

    // Destory part of the route. Stochasticly choose some destinations to remove (also can't remove anything locked)
    // remove a random amount of passengers assigned and return them (back to the central controller)
    // destroy the path so that we have to rebuild it after a constructive phase
    pub fn destroy_route(&mut self) -> VecDeque<Demand> {
        
        self.path.retain(|node| {
            rand::thread_rng().gen_bool(0.5) || self.current_destinations.contains(node)
        });
        
        VecDeque::new()
    }

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
        if self.path.len() == 0 {
            self.assignment = HashSet::new();
            // println!("No path to follow");
            return;
        }

        // Moves itself one tick along the path towards its next path point
        // self.agent_pos = (self.agent_pos.0+1.0, self.agent_pos.1)
        let mut distance_to_move = 13.4112 * 60.0; // TODO: switch to tickspeed * velocity
        while distance_to_move > 0.0 {
            // prev_node, cur_edge, agent_pos
            let edge = &self.graph.get_edgelist()[&self.cur_edge];
            
            let next_path_node = self.path.front();
            let next_node = match next_path_node {
                // Go towards the next path node if it's connected else go to the locking point node
                Some(node) => if edge.start_id == *node || edge.end_id == *node { node } else { &self.locking_node },
                None => &self.locking_node
            };
            // let next_node = self.path.front().unwrap_or(&self.locking_node); //if self.prev_node == edge.start_id { &edge.end_id } else { &edge.start_id });
            let next_node_data = &self.graph.get_nodelist()[next_node];
            
            let line = if next_node_data.point == *edge.points.first().unwrap() {
                edge.points.iter().rev().collect::<Vec<_>>() // if the "start" is the destination flip the line
            } else {
                edge.points.iter().collect::<Vec<_>>()
            };

            let mut has_moved = false;
            
            for i in 0..line.len() - 1 {
                let segment_start = line[i];
                let segment_end = line[i+1];

                if point_on_linesegment(self.agent_pos, segment_start, segment_end) {
                    // println!("ON LINE! pos: {:?}\tsegment start: {:?}\tsegment end: {:?}", self.agent_pos, segment_start, segment_end);
                    let dist_remaining = distance(self.agent_pos, *segment_end);
                    // println!("dist remaining: {:?}", dist_remaining);
                    if distance_to_move > dist_remaining {
                        // Set agent_pos to segment_end and subtract distance moved from total distance to move
                        self.agent_pos = *segment_end;
                        distance_to_move -= dist_remaining;
                        // println!("Moved {} units [To end of segment]", dist_remaining);
                        has_moved = true;
                    } else {
                        // move agent_pos along the line segment by distance_to_move
                        let dir = normalise((segment_end.0 - segment_start.0, segment_end.1 - segment_start.1));
                        self.agent_pos = (self.agent_pos.0 + dir.0 * distance_to_move, self.agent_pos.1 + dir.1 * distance_to_move);
                        distance_to_move = 0.0;
                        // println!("Moved {} units [Along line segment]", distance_to_move);
                        has_moved = true;
                        return;
                    }
                }
            }

            if !has_moved {
                // println!("Didn't move this iteration distance left {:?}", distance_to_move);
                return;
            }

            if has_moved && distance_to_move > 0.0 {
                // We have moved the full distance to move along the current edge
                // Move to the next edge
                // println!("path: {:?}", self.path);
                self.prev_node = *next_node; // Node we're coming from
                // let next_node = self.path.front().unwrap();

                match self.path.front() {
                    Some(&next_node) => {
                        
                        // println!("moving to next node {:?}", next_node);
                        self.cur_edge = *self.graph.get_adjacency()[&self.prev_node].iter().find(|e| {
                            let edge = &self.graph.get_edgelist()[*e];
                            edge.end_id == next_node || edge.start_id == next_node
                        }).unwrap(); // TODO: fix panic here

                        self.path.pop_front();
                        self.locking_node = next_node;
                    },
                    None => {
                        // println!("No next node. Randomising next move and stopping.");
                        self.assignment = HashSet::new();
                        let adj_count = self.graph.get_adjacency()[&self.prev_node].len() - 1;
                        self.cur_edge = self.graph.get_adjacency()[&self.prev_node][rand::thread_rng().gen_range(0..=adj_count)];
                        self.locking_node = self.graph.get_edgelist()[&self.cur_edge].end_id;
                        break;
                    }
                }

            }
        }
    }

}

// fn point_on_linesegment(pos: (f64, f64), start: &(f64, f64), end: &(f64, f64)) -> bool {
//     let crossprod = (pos.1 - start.1) * (end.0 - start.0) - (pos.0 - start.0) * (end.1 - start.1);
//     if crossprod.abs() > 0.00001 {
//         return false;
//     }
//     let dotprod = (pos.0 - start.0) * (end.0 - start.0) + (pos.1 - start.1) * (end.1 - start.1);
//     if dotprod < 0.0 {
//         return false;
//     }
//     let squared_length = (end.0 - start.0) * (end.0 - start.0) + (end.1 - start.1) * (end.1 - start.1);
//     if dotprod > squared_length {
//         return false;
//     }

//     true
// }

fn point_on_linesegment(pos: (f64, f64), start: &(f64, f64), end: &(f64, f64)) -> bool {
    // let xs = (pos.0 - start.0) / (end.0 - start.0);
    // let ys = (pos.1 - start.1) / (end.1 - start.1);
    // println!("pos {:?} start {:?}\tend {:?}\ton_line {:?}\txs {:?}\tys {:?}", pos, start, end, float_eq::float_eq!(xs, ys, abs <= 0.1) && 0.0 <= xs && xs <= 1.0, xs, ys);
    // (float_eq::float_eq!(xs, ys, abs <= 0.1) && 0.0 <= xs && xs <= 1.0) || (pos.0 == start.0 && pos.1 == start.1) || (pos.0 == end.0 && pos.1 == end.1)
    let d1 = distance(pos, *start);
    let d2 = distance(pos, *end);

    let line_len = distance(*start, *end);

    let buffer = 0.1;
    // println!("pos {:?} start {:?}\tend{:?}\ton_line {:?}\tdsum {:?}\tdistance{:?}", pos, start, end, d1 + d2 >= line_len - buffer && d1 + d2 <= line_len + buffer, d1 + d2, line_len);
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