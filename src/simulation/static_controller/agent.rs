use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::{Arc, mpsc::Sender}};

use chrono::Utc;
use eframe::epaint::{Shape, pos2, Stroke, Color32};

use crate::{
    graph::Graph,
    simulation::{
        dyn_controller::bus::CurrentElement,
        Agent,
    }, analytics::{AnalyticsPackage, PassengerAnalyticsEvent, VehicleAnalyticsEvent},
};

use super::{
    routes::{self, get_graph_edge_from_stop, NetworkData},
    Control,
};

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

#[derive(Debug, Clone, PartialEq)]
pub enum PassengerStatus {
    Generated,
    Waiting,
    OnBus,
    Finished,
}

impl Default for PassengerStatus {
    fn default() -> Self {
        PassengerStatus::Generated
    }
}

/// Represents the passenger of a generated demand which is on the bus
#[derive(Default, Debug, Clone)]
pub struct BusPassenger {
    pub id: u32,
    pub source_pos: (f64, f64),
    pub source_stop: u32,

    pub dest_pos: (f64, f64),
    pub dest_stop: u32,

    pub instructions: VecDeque<Control>,

    pub status: PassengerStatus,
    pub analytics: Option<Sender<AnalyticsPackage>>,
}

impl BusPassenger {
    // get stop to get off at, or stop which will be getting on at (waiting)
    pub fn get_next_stop(&self) -> u32 {
        println!("passenger instructions {:?} status {:?}", self.instructions, self.status);
        match self.instructions[0] {
            Control::TakeBus{source, destination, trip_id} => match self.status {
                PassengerStatus::Waiting => source,
                PassengerStatus::OnBus => destination,
                _ => panic!("Invalid passenger status"),
            },
            Control::WalkToStop{destination_stop, source_stop} => destination_stop,
        }
    }

    pub fn get_next_trip_id(&self) -> u32 {
        match self.instructions[0] {
            Control::TakeBus{ trip_id, .. } => trip_id,
            _ => panic!("Invalid passenger status"),
        }
    }

    pub fn get_off_bus(&mut self) {
        self.instructions.pop_front();
        self.status = PassengerStatus::Finished;
    }

    pub fn get_on_bus(&mut self) {
        // self.instructions.pop_front(); // Maybe don't need to do this since on bus you should have intruction of taking bus
        self.status = PassengerStatus::OnBus;
    }

    pub fn update(&mut self) {
        match self.status {
            PassengerStatus::Waiting => {
                send_analytics(&self.analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::WaitingTick { id: self.id, waiting_pos: self.source_pos }))
            },
            PassengerStatus::OnBus => {
                send_analytics(&self.analytics, AnalyticsPackage::PassengerEvent(PassengerAnalyticsEvent::InTransitTick { id: self.id }))
            },
            PassengerStatus::Generated | PassengerStatus::Finished => {},
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BusStatus {
    Active,
    Unactive,
}

impl Default for BusStatus {
    fn default() -> Self {
        BusStatus::Unactive
    }
}

type StopAccessFunction = Box<dyn Fn(u32, u32) -> Vec<Rc<RefCell<BusPassenger>>>>;

pub struct StaticAgent {
    pub position: (f64, f64),
    pub trip_id: u32,
    pub status: BusStatus,

    // Positioning and movement information
    pub current_element: crate::simulation::dyn_controller::bus::CurrentElement,
    pub next_node: u128,
    pub remaining_route: VecDeque<u128>, // TODO: build this route of graph nodes from network data
    pub trip_route: Vec<u128>,

    pub trip_stop_edges: Vec<(u128, f64)>,

    // Passengers
    pub passengers: Vec<BusPassenger>, // list of passengers on the bus right now

    // Simulation information
    pub graph: Arc<Graph>,
    pub network_data: Arc<NetworkData>,

    // Analytics
    pub analytics: Option<Sender<AnalyticsPackage>>
}

impl Agent for StaticAgent {
    fn display(&self) -> eframe::epaint::Shape {
        if self.status == BusStatus::Unactive {
            eframe::epaint::Shape::Noop
        } else {
            // let shape = default_display(self);

            Shape::Vec(vec![
                Shape::circle_stroke(pos2(self.position.0 as f32, self.position.1 as f32), 3.0, Stroke::new(1.5, Color32::YELLOW)),
                match self.current_element {
                    CurrentElement::Edge{ edge, prev_node } => {
                        let edge_data = self.graph.get_edgelist().get(&edge).expect("Edge not found");
                        let node_data = self.graph
                            .get_nodelist()
                            .get(&prev_node)
                            .expect("Node not found");

                        Shape::line(
                            edge_data
                        .points
                        .iter()
                        .map(|&(x, y)| pos2(x as _, y as _))
                        .collect(),
                            Stroke::new(1.0, Color32::LIGHT_GREEN),
                        )
                    }
                    CurrentElement::Node(node_id) => {
                        let node_data = self.graph.get_nodelist().get(&node_id).unwrap();
                        Shape::circle_stroke(
                            pos2(node_data.point.0 as _, node_data.point.1 as _),
                            3.0,
                            Stroke::new(2.0, Color32::LIGHT_GREEN),
                        )
                    },
                    CurrentElement::PreGenerated => Shape::Noop,
                },
                Shape::line(self.network_data.trips.get(&self.trip_id).unwrap().stops.iter().map(|stop| {
                    let stop_data = self.network_data.stops.get(stop).unwrap();
                    pos2(stop_data.position().0 as _, stop_data.position().1 as _)
                }).collect::<Vec<_>>(), Stroke::new(1.0, Color32::GREEN)),
                
                Shape::line(self.remaining_route.iter().map(|node| {
                    let node_data = self.graph.get_nodelist().get(node).unwrap();
                    pos2(node_data.point.0 as _, node_data.point.1 as _)
                }).collect::<Vec<_>>(), Stroke::new(0.5, Color32::LIGHT_YELLOW))
                ,
                Shape::Vec( self.trip_stop_edges.iter().map(|edge| {
                    let edge_data = self.graph.get_edgelist().get(&edge.0).expect("Edge not found");

                    Shape::line(edge_data.points.iter().map(|&(x, y)| pos2(x as _, y as _)).collect(),
                        Stroke::new(2.0, Color32::DARK_GREEN),
                    )
                }).collect::<Vec<_>>() )
            ])

        }
    }

    fn get_current_element(&self) -> crate::simulation::dyn_controller::bus::CurrentElement {
        self.current_element
    }

    fn get_graph(&self) -> Arc<Graph> {
        self.graph.clone()
    }

    fn get_next_node(&self) -> u128 {
        self.next_node
    }

    fn get_position(&self) -> (f64, f64) {
        self.position
    }
}

impl StaticAgent {
    pub fn new(trip_id: u32, graph: Arc<Graph>, network_data: Arc<NetworkData>, analytics: Option<Sender<AnalyticsPackage>>) -> Self {
        let (trip_route, trip_stop_edges) =
            routes::convert_trip_to_graph_path(trip_id, graph.clone(), network_data.clone());

        // println!("{}\t{:?}\t{:?}", trip_id, trip_route, trip_stop_edges);
        // println!("\t{:?}", network_data.trips.get(&trip_id).unwrap().stops);

        let route_beginning_node = *trip_route.first().unwrap();
        let route_beginning_edge = *trip_stop_edges.first().unwrap();
        let route_beginning_stop = *network_data
            .trips
            .get(&trip_id)
            .expect("Agent has invlaid trip ID")
            .stops
            .first()
            .unwrap();

        let route_beginning_position = {
            let stop_data = network_data
                .stops
                .get(&route_beginning_stop)
                .expect("This agent has an invalid trip ID");
            let edge = get_graph_edge_from_stop(stop_data, graph.clone());
            let edge_offset = closest_point_on_edge_to_stop(
                edge,
                graph.clone(),
                (stop_data.easting, stop_data.northing),
            ); // length along edge from start -> point closet to stop easting / northing;
            edge_offset.0
        };

        let edge_data = graph
            .get_edgelist()
            .get(&route_beginning_edge)
            .expect("This agent has an invalid trip ID");
        let prev_node = if edge_data.start_id == route_beginning_node {
            edge_data.end_id
        } else {
            edge_data.start_id
        };
        let current_element = CurrentElement::Edge {
            edge: route_beginning_edge,
            prev_node,
        };

        let trip_data = network_data
            .trips
            .get(&trip_id)
            .expect("This agent has an invalid trip ID");
        let trip_stop_edges = trip_data
            .stops
            .iter()
            .map(|stop| {
                let stop_data = network_data
                    .stops
                    .get(stop)
                    .expect("This agent has an invalid trip ID");
                let edge = get_graph_edge_from_stop(stop_data, graph.clone());
                let (_, offset) = closest_point_on_edge_to_stop(
                    edge,
                    graph.clone(),
                    (stop_data.easting, stop_data.northing),
                );
                (edge, offset)
            })
            .collect();

        Self {
            trip_id,
            graph,
            network_data,
            remaining_route: VecDeque::from_iter(trip_route.clone().into_iter()),
            trip_route,
            current_element,
            trip_stop_edges,
            next_node: route_beginning_node.clone(),
            position: route_beginning_position.clone(),
            status: BusStatus::Unactive,
            passengers: Vec::new(),
            analytics
        }
    }

    // Move self for 1 tick
    pub fn move_self<G>(
        &mut self,
        tick: chrono::DateTime<Utc>,
        mut pick_up_and_drop_off_passengers: G,
    ) where
        G: FnMut(u32, u32, Vec<BusPassenger>) -> Vec<BusPassenger>,
    {
        // if time tick is before trip start => bus is non-active
        let start_time = self
            .network_data
            .trips
            .get(&self.trip_id)
            .expect("This agent has an invalid trip ID")
            .timings
            .first()
            .unwrap()
            .0;
        if tick.time() < start_time {
            println!("agent {} is not active", self.trip_id);
            self.status = BusStatus::Unactive;
            return;
        } else {
            println!("agent {} is active", self.trip_id);
            self.status = BusStatus::Active;
        }
        // when time tick is in trip => bus is active and moves along the trip route
        // trying to stick to timings as much as possible

        self.passengers.iter_mut().for_each(|passenger| {
            passenger.update();
        });

        // TODO: move code
        move_agent(self, tick, |trip_id, stop_id, agent| {
            // TODO: stop checking codeclaer
            println!("=== ### agent {} is at stop {} ### ===", trip_id, stop_id);

            let mut passengers_to_drop = Vec::new();
            let mut i = 0;
            while i < agent.passengers.len() {
                let passenger = agent.passengers.get(i).unwrap();
                if passenger.get_next_stop() == stop_id {
                    // TODO: check if this is their stop
                    passengers_to_drop.push(agent.passengers.remove(i));
                } else {
                    i += 1;
                }
            }

            // TODO: get the stop ID if at a stop
            let passengers_to_pick_up =
                pick_up_and_drop_off_passengers(trip_id, stop_id, passengers_to_drop);
            agent
                .passengers
                .extend(passengers_to_pick_up.into_iter().map(|mut p| {
                    send_analytics(&agent.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::PassengerPickup { id: agent.trip_id, passenger_id: p.id }));
                    p.get_on_bus();
                    p
                }));
        });
    }

    pub fn destroy_self(&mut self) {
        // destroy the bus and drop off all remaining passengers at the last stop
        // this will then remove the bus from the simulation, etc
        self.status = BusStatus::Unactive;
    }
}

pub fn move_agent(
    agent: &mut StaticAgent,
    tick: chrono::DateTime<Utc>,
    mut stop_check: impl FnMut(u32, u32, &mut StaticAgent),
) {
    // No need to move agent if no path to follow
    if agent.remaining_route.is_empty() {
        // println!("{} Agent has no path", agent.trip_id);
        agent.destroy_self();
        return; // No path to follow
    }

    println!("{} Move self", agent.trip_id);
    println!("{} Current element: {:?}", agent.trip_id, agent.current_element);
    println!("{} Next node: {:?}", agent.trip_id, agent.next_node);
    // println!("Path: {:?}", self.path_full);

    let mut move_distance = 804.672; //10.0; 804.672 = 13.4112 * 60.0 (13.4112 m/s * 60s)
    while move_distance > 0.0 {
        // Id of the edge we are currently on, or need to move along
        let moving_edge_id = match agent.current_element {
            CurrentElement::PreGenerated => {
                unreachable!("The agent is trying to move before it has been generated")
            }
            CurrentElement::Edge { edge, .. } => edge,
            CurrentElement::Node(node) => {
                let next_node = agent.next_node;
                *agent.graph.get_adjacency()[&node]
                    .iter()
                    .find(|edge| {
                        let edge_data = &agent.graph.get_edgelist()[edge];
                        edge_data.start_id == next_node || edge_data.end_id == next_node
                    })
                    .unwrap()
            }
        };
        let moving_edge_data = &agent.graph.get_edgelist()[&moving_edge_id];

        let next_node = agent.next_node;
        let next_node_data = &agent.graph.get_nodelist()[&next_node];

        let line = if next_node_data.point == *moving_edge_data.points.first().unwrap() {
            moving_edge_data.points.iter().rev().map(|x| *x).collect() // if the next node is the first point on the edge, we need to reverse the line
        } else if next_node_data.point == *moving_edge_data.points.last().unwrap() {
            moving_edge_data.points.clone()
        } else {
            // println!("{} Moving edge: start: {:?} end: {:?}; next_node {:?}", agent.trip_id, moving_edge_data.start_id, moving_edge_data.end_id, next_node_data.id);
            unreachable!("The next node is not on the edge we are moving along");
        };

        let mut has_moved = false;
        for i in 0..line.len() - 1 {
            let segment_start = line[i];
            let segment_end = line[i + 1];

            if point_on_linesegment(agent.position, &segment_start, &segment_end) {
                let prev_offset = (0..i).map(|i| distance(line[i], line[i + 1])).sum::<f64>()
                    + distance(segment_start, agent.position);

                // println!("On line segment {}/{}", i, line.len());
                let distance_remaining = distance(agent.position, segment_end);
                // println!("Distance remaining: {}", distance_remaining);
                if move_distance > distance_remaining {
                    // if move distance is > distance to end of line segment, move to end of line segment. Will then consider the next segment.
                    agent.position = segment_end;
                    move_distance -= distance_remaining;
                    has_moved = true;
                } else {
                    let dir = normalise((
                        segment_end.0 - segment_start.0,
                        segment_end.1 - segment_start.1,
                    ));
                    agent.position = (
                        agent.position.0 + dir.0 * move_distance,
                        agent.position.1 + dir.1 * move_distance,
                    );

                    send_analytics(&agent.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::MovementTick { id: agent.trip_id, pos: agent.position }));
                    return;
                }

                let new_offset = (0..i).map(|i| distance(line[i], line[i + 1])).sum::<f64>()
                    + distance(segment_start, agent.position);

                // if this is the edge which contains a stop && the offset before was before the stop and the offset after was after the stop then we've passed the stop and can do the thing
                for i in 0..agent.trip_stop_edges.len() {
                    let (edge, offset) = agent.trip_stop_edges[i];
                    if edge == moving_edge_id && offset > prev_offset && offset <= new_offset {
                        stop_check(
                            agent.trip_id,
                            agent
                                .network_data
                                .trips
                                .get(&agent.trip_id)
                                .expect("Invalid Trip ID on agent")
                                .stops[i],
                            agent,
                        );
                    }
                }
            } else {
                // println!("{} Not on line segment {}/{}", agent.trip_id, i, line.len());
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
            let current_node = agent.next_node; //self.path_full.pop_front().unwrap(); // Also should be the current self.next_node before we update it
            agent.next_node = match agent.remaining_route.pop_front() {
                Some(next_node) => {
                    // Find edge which connects current node to the next node in the path
                    let edge_id = agent.graph.get_adjacency()[&current_node]
                        .iter()
                        .find(|e| {
                            let edge = &agent.graph.get_edgelist()[e];
                            edge.start_id == next_node || edge.end_id == next_node
                        })
                        .unwrap(); // TODO: fix potential panic here?

                    agent.current_element = CurrentElement::Edge {
                        edge: *edge_id,
                        prev_node: current_node,
                    };
                    next_node
                }
                None => {
                    // We have reached the end of the path
                    agent.current_element = CurrentElement::Node(current_node);
                    return;
                }
            };
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

pub fn closest_point_on_line_segment_to_point(
    segment: [(f64, f64); 2],
    point: (f64, f64),
) -> (f64, f64) {
    // let edge_u = segment[0];
    // let edge_v = segment[1];

    // let u_v = (edge_v.0 - edge_u.0, edge_v.1 - edge_u.1);
    // let u_p = (point.0 - edge_u.0, point.1 - edge_u.1);

    // let proj = (u_v.0 * u_p.0 + u_v.1 * u_p.1) / (u_v.0.powi(2) + u_v.1.powi(2));
    // let u_v_len2 = u_v.0.powi(2) + u_v.1.powi(2);
    // let distance = proj / u_v_len2;

    // (edge_u.0 + distance * u_v.0, edge_u.1 + distance * u_v.1)
    let p1@(p1_x, p1_y) = segment[0];
    let p2@(p2_x, p2_y) = segment[1];
    let (p3_x, p3_y) = point;

    let u = ((p3_x - p1_x) * (p2_x - p1_x) + (p3_y - p1_y) * (p2_y - p1_y))
        / ((p2_x - p1_x).powi(2) + (p2_y - p1_y).powi(2));

    if u < 0.0 {
        p1
    } else if u > 1.0 {
        p2
    } else {
        (p1_x + u * (p2_x - p1_x), p1_y + u * (p2_y - p1_y))
    }
}

// Taken from Paul Bourke
fn dist_point_linesegment_2(segment: [(f64, f64); 2], point: (f64, f64)) -> f64 {
    let p1@(p1_x, p1_y) = segment[0];
    let p2@(p2_x, p2_y) = segment[1];
    let (p3_x, p3_y) = point;

    let u = ((p3_x - p1_x) * (p2_x - p1_x) + (p3_y - p1_y) * (p2_y - p1_y))
        / ((p2_x - p1_x).powi(2) + (p2_y - p1_y).powi(2));

    let (proj_x, proj_y) = if u < 0.0 {
        p1
    } else if u > 1.0 {
        p2
    } else {
        (p1_x + u * (p2_x - p1_x), p1_y + u * (p2_y - p1_y))
    };

    (p3_x - proj_x).powi(2) + (p3_y - proj_y).powi(2)
}

// returns the closest (point, offset) for the edge in the graph
pub fn closest_point_on_edge_to_stop(
    edge: u128,
    graph: Arc<Graph>,
    point: (f64, f64),
) -> ((f64, f64), f64) {
    let edge_data = &graph.get_edgelist()[&edge];
    let mut closest_point = (0.0, 0.0);
    let mut closest_offset = 0.0;
    let mut closest_distance = std::f64::MAX;

    for i in 0..edge_data.points.len() - 1 {
        let segment = [edge_data.points[i], edge_data.points[i + 1]];
        let point_on_segment = closest_point_on_line_segment_to_point(segment, point);
        let pt_distance = distance(point_on_segment, point);

        // offset is the length of the edge up to the point on the segment
        let offset = (0..i)
            .map(|j| distance(edge_data.points[j], edge_data.points[j + 1]))
            .sum::<f64>()
            + distance(edge_data.points[i], point_on_segment);

        if pt_distance < closest_distance {
            closest_distance = pt_distance;
            closest_point = point_on_segment;
            closest_offset = offset;
        }
    }

    (closest_point, closest_offset)
}
