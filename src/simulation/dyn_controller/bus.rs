use std::{collections::{VecDeque, HashMap}, sync::{Arc, mpsc::Sender}};

use chrono::{DateTime, Utc};
use eframe::epaint::{Shape, Stroke, Color32, pos2};
use rand::Rng;

use crate::{graph::{Graph, route_finding}, simulation::{Agent, default_display}, analytics::{AnalyticsPackage, PassengerAnalyticsEvent, VehicleAnalyticsEvent}};

use super::waypoints::{bus_waypoints, create_ordering, Waypoint, bus_waypoints_with_passenger};

const HUMAN_WALKING_SPEED: f64 = 1.4; // m/s

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
    pub id: u32,
    pub source_pos: (f64, f64),
    pub source_node: u128,
    pub dest_pos: (f64, f64),
    pub dest_node: u128,
    pub timeframe: DateTime<Utc>,
    pub status: Status
}

pub fn send_analytics(analytics: &Option<Sender<AnalyticsPackage>>, event: AnalyticsPackage) {
    if let Some(tx) = analytics.as_ref() {
        // println!("[ANALYTICS] Sending analytics event!");
        if let Err(err) = tx.send(event) {
            panic!("[ANALYTICS] Unable to send analytics: {:?}", err);
        } else {
            // println!("[ANALYTICS] Sent analytics event!");
        }
    } else {
        // println!("[ANALYTICS] No analytics channel found!");
    }
}

impl Passenger {
    pub fn update(&mut self, analytics: &Option<Sender<AnalyticsPackage>>) {
        // println!("{:?} Passenger update", self.id);
        match self.status {
            Status::Generated | Status::Expired => {}, // Passenger state necessitates nothing happening
            Status::TravelStart(ticks) => { // start by walking `ticks` to the start node
                send_analytics(analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::StartWalkingTick { id: self.id }));
                if ticks == 0 {
                    self.status = Status::Waiting(0);
                } else {
                    self.status = Status::TravelStart(ticks - 1);
                }
            },
            Status::Waiting(ticks) => {
                send_analytics(analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::WaitingTick { id: self.id, waiting_pos: self.source_pos }));
                self.status = Status::Waiting(ticks + 1);
            },
            Status::OnBus(_) => {
                send_analytics(analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::InTransitTick { id: self.id }));
            },
            Status::TavelDest(ticks) => { // after reaching destination node, walking for `ticks` to end
                send_analytics(analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::EndWalkingTick { id: self.id }));
                if ticks == 0 {
                    self.status = Status::Expired;
                } else {
                    self.status = Status::TavelDest(ticks - 1);
                }
            }
        }
    }

    pub fn set_on_bus(&mut self) {
        self.status = Status::OnBus(Utc::now());
    }

    pub fn set_travel_start(&mut self, graph: Arc<Graph>) {
        let dist = graph.get_nodelist().get(&self.source_node).expect("Node not found");
        let dist = distance(dist.point, self.source_pos);
        let ticks = (dist / 60.0 * HUMAN_WALKING_SPEED) as u8;
        self.status = Status::TravelStart(ticks);
    }

    pub fn set_travel_end(&mut self, graph: Arc<Graph>) {
        let dist = graph.get_nodelist().get(&self.dest_node).expect("Node not found");
        let dist = distance(dist.point, self.dest_pos);
        let ticks = (dist / 60.0 * HUMAN_WALKING_SPEED) as u8;
        self.status = Status::TavelDest(ticks);
    }
}

#[derive(Default)]
pub struct Bus {
    
    pub graph: Arc<Graph>, // Reference to the graph this agent is operating on

    pub agent_id: usize, // ID of this agent
    pub max_capacity: u8, // Maximum capacity of the agent/bus
    pub rem_capacity: u8, // Remaining capacity of the agent/bus
    
    pub passengers: Vec<Passenger>, // List of passengers on the bus (current assignment/solution)
    pub assignment: HashMap<u128, Vec<Passenger>>, // Future passengers to be added to the bus (future assignment/solution)
    
    pub delivered_passengers: Vec<Passenger>, // List of passengers delivered to their destination

    pub path_waypoints: VecDeque<Waypoint>, // List of important nodes which should be in full path // TODO: look into a hashset or some ordering of waypoints
    pub path_full: VecDeque<u128>, // List of nodes to visit to complete the assignment

    pub current_pos: (f64, f64), // Current position of the agent
    pub current_el: CurrentElement, // Current edge the agent is on
    pub next_node: u128, // Next node the agent is travelling to; the "locking node"

    pub analytics: Option<Sender<AnalyticsPackage>>, // Sender to the analytics thread
}

const STROKES: [Stroke; 2] = [
    Stroke { width: 2.0, color: Color32::LIGHT_BLUE }, Stroke {  width: 1.8, color: Color32::LIGHT_BLUE }
];

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

        let mut waypoints = self.path_waypoints.iter().map(|node| {
            let node_data = self.graph.get_nodelist().get(&node.node()).expect("Node not found");
            Shape::circle_filled(pos2(node_data.point.0 as _, node_data.point.1 as _), 3.0, Color32::DEBUG_COLOR)
        }).collect::<Vec<_>>();

        let mut sources = self.assignment.iter().filter(|(_, vec)| vec.len() > 0).map(|(node, _)| {
            let node_data = self.graph.get_nodelist().get(node).expect("Node not found");
            Shape::circle_filled(pos2(node_data.point.0 as _, node_data.point.1 as _), 1.0, Color32::RED)
        }).collect::<Vec<_>>();

        let path = Shape::line(self.path_full.iter().map(|node| {
            if self.graph.get_nodelist().get(node).is_none() {
                println!("Node not found: {}", node);
            }
            let node_data = self.graph.get_nodelist().get(node).expect("Node not found"); // TODO: panic here
            pos2(node_data.point.0 as _, node_data.point.1 as _)
        }).collect(), STROKES[(self.agent_id % 2) as usize]); //Stroke::new(2.0, Color32::LIGHT_BLUE)

        shapes.append(&mut waypoints);
        shapes.push(path);
        shapes.push(base_shape);
        shapes.append(&mut sources);

        Shape::Vec(shapes)
    }
}

/// has some set of passengers with their source, destination and time window
/// has a route of stops to visit
/// 
impl Bus {
    
    fn handle_node(&mut self, node: u128) -> Action {
        
        // Add waiting passengers to the bus
        let passengers_at_this_node = self.assignment.get_mut(&node);
        match passengers_at_this_node {
            Some(passengers) => {

                let mut i = 0;
                while i < passengers.len() {
                    if self.rem_capacity > 0 {
                        let mut passenger = passengers.remove(i);
                        // Passenger has been picked up by the bus
                        passenger.set_on_bus();
                        
                        send_analytics(&self.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::PassengerPickup { id: self.agent_id as u32, passenger_id: passenger.id }));
                        
                        self.passengers.push(passenger);
                        self.rem_capacity -= 1;
                    } else {
                        i += 1;
                    }
                }
                // for i in 0..passengers.len() {
                //     if self.rem_capacity > 0 {
                //         let mut passenger = passengers.swap_remove(i); // TODO: fix panic here for removing something > len
                //         passenger.status = Status::OnBus(Utc::now()); // Passenger has been picked up by the bus
                //         self.passengers.push(passenger);
                //         self.rem_capacity -= 1;
                //     }
                // }
            },
            None => {}
        };

        // loop passengers on bus and remove if they have reached their destination
        // for passenger in self.passengers.iter_mut() {
        //     if passenger.dest_node == node {
        //         // Passenger has now finished bus journey and should move towards their destination 
        //         send_analytics(&self.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::PassengerDropoff { id: self.agent_id as u32, passenger_id: passenger.id }));
                
        //         passenger.set_travel_end(self.graph.clone());
        //         self.rem_capacity += 1;
        //     }
        // }
        // should really not just delete this, move them out & clean up for statistics, etc.
        // self.passengers.retain(|passenger| passenger.dest_node != node);
        let mut getting_off = VecDeque::new();
        let mut i = 0;
        while i < self.passengers.len() {
            let passenger = &self.passengers[i];
            if passenger.dest_node == node {
                let mut passenger = self.passengers.remove(i);

                send_analytics(&self.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::PassengerDropoff { id: self.agent_id as u32, passenger_id: passenger.id }));
                
                passenger.set_travel_end(self.graph.clone());
                self.rem_capacity += 1;

                getting_off.push_back(passenger);
            } else {
                i += 1;
            }
        }
        self.delivered_passengers.extend(getting_off.into_iter());
        
        // TODO: check if there are timeline constraints which means the bus needs to wait at this node
        Action::Continue
    }

    pub fn can_assign_more(&self) -> bool {
        self.rem_capacity > 0
    }

    // TODO: abstract out random initialisation to another function?
    pub fn new(graph: Arc<Graph>, max_capacity: u8, id: usize, analytics: Option<Sender<AnalyticsPackage>>) -> Self {

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
            rem_capacity: max_capacity,
            current_el: CurrentElement::Edge { edge: *edge, prev_node: *random_node },
            current_pos: agent_pos,
            next_node: locking_node,
            analytics,
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
    
    // TODO: needs working tests -- this panics sometimes? not been able to reproduce it.
    pub fn what_if_bus_had_passenger(&self, passenger: &Passenger) -> f64 {
        let mut waypoints = bus_waypoints_with_passenger(self, passenger);
        let path = create_ordering(self.next_node, &mut waypoints, self.graph.clone());
        let mut path_len = 0.0;

        for i in 0..path.len() - 1 { // just comparing straight line dist between waypoints not a full routefinding
            let u = path[i];
            let v = path[i + 1];
            let point_u = self.graph.get_nodelist().get(&u.node()).unwrap().point;
            let point_v = self.graph.get_nodelist().get(&v.node()).unwrap().point;
            path_len += distance(point_u, point_v);
        }

        path_len
    }

    // Adds the passenger to the assignment by placing them in their source node waiting list
    pub fn add_passenger_to_assignment(&mut self, mut passenger: Passenger) {
        // passenger should now be making its way to the bus stop! to get picked up
        passenger.set_travel_start(self.graph.clone());
        self.assignment.entry(passenger.source_node).or_insert_with(|| Vec::new()).push(passenger);
    }

    // Adds a passenger into the solution and updates pathing as appropriate
    // TODO: Assigned passengers need to move towards their pick-up station
    pub fn constructive(&mut self, passenger: Passenger) {
        self.add_passenger_to_assignment(passenger);

        // println!("Constructive");
        // println!("\t[LNS/Agent] Constructive: Bus {} now has {} passengers", self.agent_id, self.passengers.len());
        // println!("\tAssignment: {:?}", self.assignment);

        // Uses GreedyBFS to find an ordering of the waypoints for the bus
        let path = create_ordering(
            self.next_node, 
            &mut bus_waypoints(self), 
            self.graph.clone()
        );
        self.path_waypoints = path;
        
        // println!("Waypoint Path: {:?}", self.path_waypoints);

        // Create the full path between waypoints
        self.create_path();
    }

    // Helper to get the length of the waypoint path (straight line between waypoints)
    pub fn get_waypoint_path_len(&self) -> f64 {
        let mut path_len = 0.0;
        for i in 0..self.path_waypoints.len() - 1 {
            let u = self.path_waypoints[i].node();
            let v = self.path_waypoints[i + 1].node();
            let point_u = self.graph.get_nodelist().get(&u).unwrap().point;
            let point_v = self.graph.get_nodelist().get(&v).unwrap().point;
            path_len += distance(point_u, point_v);
        }
        path_len
    }

    // Destructive function to basically remove some passengers from the bus assignment
    pub fn destructive(&mut self) -> Vec<Passenger> {
        // loop throught assignent and remove 50% which aren't currently passengers
        let mut removed = Vec::with_capacity(self.assignment.len() / 2);
        for (_node, assignment) in self.assignment.iter_mut() {
            let mut rng = rand::thread_rng();

            let mut i = 0;
            while i < assignment.len() {
                let passenger = &assignment[i];
                if !self.passengers.contains(&passenger) && rng.gen_bool(0.5) { // if this assigned passenger is not on the bus currently can remove
                    let passenger = assignment.remove(i);   
                    removed.push(passenger);
                } else {
                    i += 1;
                }
            }

        }
        // println!("\t[LNS/Agent] Destructive removed {:?}", removed.len());
        removed
    }

    // Use the waypoints list to create the full path
    pub fn create_path(&mut self) {
        // println!("Create full path");
        let mut path = VecDeque::new();

        for waypoint in self.path_waypoints.iter() {
            
            match path.back() {
                Some(node) => { // The last node in the path is the source for the next subroute
                    let subroute = route_finding::find_route(&self.graph, *node, waypoint.node());
                    // println!("\tsubroute from {:?} to {:?}: {:?}", node, waypoint.node(), subroute);
                    path.extend(subroute.into_iter().rev().skip(1));
                },
                None => { // No node in the path, so just add the first waypoint 
                    // start with the current node, or the previous node?
                    // dbg!(self.current_el);
                    path.push_back(waypoint.node()) 
                }
            }
        }
        // path.pop_front(); // Remove the first node as it is the current node
        // println!("Full path: {:?}", path);
        self.path_full = path;
    }

    pub fn update_passengers(&mut self) {
        // update passengers on the bus
        self.passengers.iter_mut().for_each(|p| p.update(&self.analytics));
        
        // update passengers which are assigned / waiting for this bus 
        self.assignment.iter_mut().for_each(|(_, passengers)| passengers.iter_mut().for_each(|p| p.update(&self.analytics)));
    
        // update passengers we've "finished" with
        self.delivered_passengers.iter_mut().for_each(|p| p.update(&self.analytics)); 
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

    // Actual movement function which moves the bus one step along the computed path
    // TODO: Maybe run the "handle arrival at node" function somewhere in here..
    // TODO: handle whether the bus is at the final destination and can let the passengers off??
    pub fn move_self(&mut self) {

        self.update_passengers();

        // No need to move agent if no path to follow
        if self.path_full.len() == 0 {
            return; // No path to follow
        }

        // println!("Move self");
        // println!("Current element: {:?}", self.current_el);
        // println!("Next node: {:?}", self.next_node);
        // println!("Path: {:?}", self.path_full);

        let mut move_distance = 804.672; //10.0; 804.672 = 13.4112 * 60.0 (13.4112 m/s * 60s)
        while move_distance > 0.0 {
            // Id of the edge we are currently on, or need to move along
            let moving_edge_id = match self.current_el {
                CurrentElement::PreGenerated => unreachable!("The agent is trying to move before it has been generated"),
                CurrentElement::Edge { edge, .. } => {
                    edge
                }, 
                CurrentElement::Node(node) => {
                    let next_node = self.next_node;
                    // if next_node == node {
                    //     return; // We are at the final destination
                    // }
                    match self.graph.get_adjacency()[&node].iter().find(|edge| {
                        let edge_data = &self.graph.get_edgelist()[*edge];
                        edge_data.start_id == next_node || edge_data.end_id == next_node
                    }) {
                        Some(&edge) => edge,
                        None => return // We are at the final destination, or basically no way to get where we're going
                    }
                }
            };
            let moving_edge_data = &self.graph.get_edgelist()[&moving_edge_id];
            
            let next_node = self.next_node;
            let next_node_data = &self.graph.get_nodelist()[&next_node];

            let line = if next_node_data.point == *moving_edge_data.points.first().unwrap() {
                moving_edge_data.points.iter().rev().map(|x| *x).collect() // if the next node is the first point on the edge, we need to reverse the line
            } else if next_node_data.point == *moving_edge_data.points.last().unwrap() {
                moving_edge_data.points.clone()
            } else {
                unreachable!("The next node is not on the edge we are moving along");
            };

            let mut has_moved = false;
            for i in 0..line.len() - 1 {
                let segment_start = line[i];
                let segment_end = line[i+1];

                if point_on_linesegment(self.current_pos, &segment_start, &segment_end) {
                    // println!("On line segment {}/{}", i, line.len());
                    let distance_remaining = distance(self.current_pos, segment_end);
                    // println!("Distance remaining: {}", distance_remaining);
                    
                    if move_distance > distance_remaining { // if move distance is > distance to end of line segment, move to end of line segment. Will then consider the next segment.
                        self.current_pos = segment_end;
                        move_distance -= distance_remaining;
                        has_moved = true;
                    } else {
                        let dir = normalise((segment_end.0 - segment_start.0, segment_end.1 - segment_start.1));
                        self.current_pos = (self.current_pos.0 + dir.0 * move_distance, self.current_pos.1 + dir.1 * move_distance);
                        send_analytics(&self.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::MovementTick { id: self.agent_id as u32, pos: self.current_pos }));
                        return;
                    }
                } else {
                    // println!("Not on line segment {}/{}", i, line.len());
                }
            }
        
            if !has_moved {
                // println!("Didn't move this iteration distance left {:?}", distance_to_move);
                return;
            }
            
            // If we've moved along the segments and still have distance to traverse, we're moving past the next node.
            if has_moved && move_distance > 0.0 {
                // We have moved the full distance to move along the current edge and are now at "self.next_node"
                // Move to the next edge
 
                let current_node = self.next_node; //self.path_full.pop_front().unwrap(); // Also should be the current self.next_node before we update it
                // TODO: try to fix the destination issue by somehow not popping here, and popping in the handle arrival functiono or something
                self.next_node = match self.path_full.pop_front() {
                    Some(next_node) => {
                        // Find edge which connects current node to the next node in the path
                        let edge_id = self.graph.get_adjacency()[&current_node].iter().find(|e| {
                            let edge = &self.graph.get_edgelist()[*e];
                            edge.start_id == next_node || edge.end_id == next_node
                        }).unwrap();

                        self.current_el = CurrentElement::Edge { edge: *edge_id, prev_node: current_node };
                        next_node
                    },
                    None => {
                        // We have reached the end of the path

                        self.current_el = CurrentElement::Node(current_node);
                        return;
                    }
                };
                
                self.handle_node(current_node);

                // println!("Moving to next node!!");
                // println!("New Current node: {:?}", current_node);

                // self.current_el = match self.path_full.front() {
                //     // There is a next node on the path
                //     Some(next_node) => {
                //         let cur_edge = *self.graph.get_adjacency()[&self.next_node].iter().find(|e| {
                //             let edge = &self.graph.get_edgelist()[*e];
                //             edge.end_id == next_node || edge.start_id == next_node
                //         }).unwrap(); 

                //         // self.next_node = next_node; // we update the next_node, so now current_node is the previous node
                //         CurrentElement::Edge { edge: cur_edge, prev_node: current_node } // this probably panics at the end of a path cause cur_node is empty
                //     },
                //     // No Next Node on the path
                //     None => CurrentElement::Node(next_node)
                // };

                // let current_node_data = &self.graph.get_nodelist()[&current_node];
                // self.current_pos = current_node_data.point;
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