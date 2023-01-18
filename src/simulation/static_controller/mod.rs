//! Controller which handles the static case, i.e. traditional buses which get demand but do not respond to it.
//!
//!

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
};

use chrono::{DateTime, Utc, NaiveTime, Duration};
use eframe::epaint::{Shape, pos2, Color32};

use crate::graph::Graph;

use self::{
    agent::{BusPassenger, StaticAgent},
    routes::{closest_stop_to_point, NetworkData},
};

use super::{demand::Demand, Controller, Agent};

pub mod agent;
pub mod routes;

#[derive(Default)]
pub struct StaticController {
    buses: HashMap<u32, StaticAgent>, // Each 'bus' gets a trip
    network_data: Arc<NetworkData>,
    passengers: Vec<BusPassenger>,
}

impl Controller for StaticController {
    type Agent = StaticAgent;

    fn get_agents(&self) -> Vec<&Self::Agent> {
        self.buses.values().collect()
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> Option<&Self::Agent> {
        self.network_data.trips.iter().for_each(|(id, trip)| {
            if trip.timings[0].0 != NaiveTime::from_hms(0, 0, 0) {
                println!("Agent {} has a start time of {:?}", id, trip.timings[0]);
            }
        });
        // println!("Spawning {} agents", self.network_data.trips.len());
        // self.network_data.trips.iter().for_each(|(id, _)| {
        //     let timer = std::time::Instant::now();
        //     self.buses.insert(
        //         *id,
        //         StaticAgent::new(*id, graph.clone(), self.network_data.clone()),
        //     );
        //     println!("Spawned agent {} in {:?}", id, timer.elapsed());
        // });

        // self.buses.iter().next().unwrap().1

        // // This agent shouldn't actually be doing anything!
        // self.buses.insert(9999, StaticAgent::new(9999, graph.clone(), self.network_data.clone()));
        // self.buses.get(&9999).unwrap()
        None
    }

    fn update_agents(
        &mut self,
        graph: std::sync::Arc<crate::graph::Graph>,
        demand: std::sync::Arc<super::demand::DemandGenerator>,
        time: chrono::DateTime<chrono::Utc>,
    ) {

        // spawn any agents which will be starting this tick
        self.network_data.trips.iter().filter(|trip| {
            // trip begins in this tick
            // if time is less than a minute after the start time, then we should spawn the agent. 
            time.time() - trip.1.timings[0].0 > Duration::zero() && time.time() - trip.1.timings[0].0 < Duration::minutes(1)

            // times_relatively_equal(trip.1.timings[0].0, time.time())
        }).for_each(|(id, trip)| {
            println!("Spawning agent {}\t{:?}\t{:?}", id, time.time(), trip.timings[0]);
            // spawn a new agent
            self.buses.insert(
                *id,
                StaticAgent::new(*id, graph.clone(), self.network_data.clone()),
            );
        });

        // TODO: actually use a sane demand generation which is consistent across static/dynamic controllers
        let demand_queue = demand.generate_amount(2, &time);
        let demand_queue: VecDeque<_> = demand_queue
            .into_iter()
            .map(|d| demand_to_passenger(d, graph.clone(), self.network_data.clone(), time))
            .collect();
        self.passengers.extend(demand_queue);
        // self.demands.append(&mut demand_queue);

        // self.buses.iter_mut().for_each(|(_, agent)| {
        //     agent.move_self(time, self);
        // });

        for agent in self.buses.values_mut() {
            agent.move_self(time, |trip, stop, drop_off_passengers| {
                let mut passengers = Vec::new();
                let mut i = 0;
                while i < self.passengers.len() {
                    let passenger = self.passengers.get(i).unwrap();
                    if passenger.get_next_stop() == stop && passenger.get_next_trip_id() == trip {
                        passengers.push(self.passengers.remove(i));
                    } else {
                        i += 1;
                    }
                }
                self.passengers
                    .extend(drop_off_passengers.into_iter().map(|mut p| {
                        p.get_off_bus();
                        p
                    }));
                passengers
            });
        }

        // TODO: have some passenger update cycle which feeds into the analytics
    }
}

impl StaticController {

    pub fn set_network_data(&mut self, data: Arc<NetworkData>) {
        self.network_data = data;
    }

    pub fn get_display(&self) -> Vec<Shape> {
        let mut shapes = Vec::new();
        self.buses
            .values()
            .for_each(|bus| shapes.push(bus.display()));
        
        shapes.extend(self.passengers.iter().map(|passenger| {
            Shape::circle_filled(pos2(passenger.source_pos.0 as f32, passenger.source_pos.1 as f32), 1.0, Color32::LIGHT_RED)
        }));

        shapes.extend(self.network_data.stops.iter().map(|stop| {
            Shape::circle_filled(pos2(stop.1.easting as f32, stop.1.northing as f32), 1.0, Color32::LIGHT_BLUE)
        }));

        shapes
    }
}

// TODO: try to make passengers more smart in picking the right stops s.t. a bus route actually exists between them?
pub fn demand_to_passenger(
    demand: Demand,
    graph: Arc<Graph>,
    network_data: Arc<NetworkData>,
    tick: DateTime<Utc>,
) -> BusPassenger {
    let source = demand.0;
    let dest = demand.1;

    let source_bus_stop =
        closest_stop_to_point((source.0 as f64, source.1 as f64), network_data.clone());

    let destination_bus_stop =
        closest_stop_to_point((dest.0 as f64, dest.1 as f64), network_data.clone());

    let mut control = Vec::new();

    // Naive trip goes from source to destination
    // TODO: ensure timings match up
    let trip = network_data.trips.iter().find(|(_, stops)| {
        let source_index = stops.stops.iter().position(|stop| stop == &source_bus_stop);
        let dest_index = stops
            .stops
            .iter()
            .position(|stop| stop == &destination_bus_stop);

        if dest_index.is_none() || source_index.is_none() {
            // Return false if doesn't contain either
            false
        } else {
            let source_index = source_index.unwrap();
            let dest_index = dest_index.unwrap();
            let source_time = &stops.timings[source_index].0;

            source_index < dest_index && source_time >= &tick.time() // Return false if source is after dest or source time is after tick
        }
    });

    // If there's not a direct route, do some pathfinding on trips to reach destination
    if let Some((id, trip)) = trip {
        control.push(Control::TakeBus(source_bus_stop, destination_bus_stop, *id));
    } else {
        let destination_stop_position = network_data
            .stops
            .get(&destination_bus_stop)
            .unwrap()
            .position();

        let mut current_stop = source_bus_stop;
        let MAX_DEPTH = 2;
        // while not path to destination
        // order paths by closest stop in path (only include if last stop in path was there & timings match)
        // pick the closest one, and then continue until get to stop

        while current_stop != source_bus_stop {
            // TODO: if no suitable path can be found, just take it to a closer stop and the agent can walk to dest
            if control.len() > MAX_DEPTH {
                break;
            }

            let trip = network_data
                .trips_from_stop
                .get(&current_stop)
                .expect("No Trips from Current Stop Dead End")
                .iter()
                .map(|id| (id, network_data.trips.get(id).unwrap()))
                .min_by(|a_trip, b_trip| {
                    let min_a_dist = a_trip
                        .1
                        .stops
                        .iter()
                        .map(|stop| {
                            distance(
                                network_data.stops.get(stop).unwrap().position(),
                                destination_stop_position,
                            )
                        })
                        .min_by(|a, b| a.total_cmp(b))
                        .unwrap();

                    let min_b_dist = b_trip
                        .1
                        .stops
                        .iter()
                        .map(|stop| {
                            distance(
                                network_data.stops.get(stop).unwrap().position(),
                                destination_stop_position,
                            )
                        })
                        .min_by(|a, b| a.total_cmp(b))
                        .unwrap();

                    // TODO: ensure timings match up
                    min_a_dist.total_cmp(&min_b_dist)
                })
                .unwrap();

            let next_stop = trip
                .1
                .stops
                .iter()
                .map(|stop| {
                    (
                        stop,
                        distance(
                            network_data.stops.get(stop).unwrap().position(),
                            destination_stop_position,
                        ),
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;

            control.push(Control::TakeBus(current_stop, *next_stop, *trip.0));
            current_stop = *next_stop;
        }

        // Could expand this by looking at the neighbourhood of stops and finding the closest stop which has the trip
        // which goes closest to the destination
        // would also look at expanding the source / destination stops?
    }

    BusPassenger {
        source_pos: (source.0 as f64, source.1 as f64),
        source_stop: source_bus_stop,

        dest_pos: (dest.0 as f64, dest.1 as f64),
        dest_stop: destination_bus_stop,

        ..Default::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    TakeBus(u32, u32, u32), // Take the bus from source to destination stops (trip ID for explicitness)
    WalkToStop(u32, u32),   // Walk to the stop (source, destination)
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let xs = (a.0 - b.0).abs();
    let ys = (a.1 - b.1).abs();
    xs.hypot(ys)
}

pub fn times_relatively_equal(time_a: NaiveTime, time_b: NaiveTime) -> bool {
    if time_a > time_b {
        time_a - time_b <= Duration::minutes(1)
    } else {
        time_b - time_a <= Duration::minutes(1)
    }
}