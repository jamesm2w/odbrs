//! Controller which handles the static case, i.e. traditional buses which get demand but do not respond to it.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, mpsc::Sender},
};

use chrono::{DateTime, Duration, NaiveTime, Utc};
use eframe::epaint::{pos2, Color32, Shape};

use crate::{graph::Graph, analytics::{AnalyticsPackage, VehicleAnalyticsEvent}};

use self::{
    agent::{BusPassenger, StaticAgent},
    routes::{closest_stop_to_point, NetworkData},
};

use super::{demand::Demand, Agent, Controller};

pub mod agent;
pub mod routes;

#[derive(Default)]
pub struct StaticController {
    buses: HashMap<u32, StaticAgent>, // Each 'bus' gets a trip
    network_data: Arc<NetworkData>,
    passengers: Vec<BusPassenger>,
    analytics: Option<Sender<AnalyticsPackage>>,
    passenger_id: u32,
}

impl Controller for StaticController {
    type Agent = StaticAgent;

    fn get_agents(&self) -> Vec<&Self::Agent> {
        self.buses.values().collect()
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> Option<&Self::Agent> {
        self.network_data.trips.iter().for_each(|(id, trip)| {
            // if trip.timings[0].0 != NaiveTime::from_hms(0, 0, 0) {
            //     println!("Agent {} has a start time of {:?}", id, trip.timings[0]);
            // }
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
        self.network_data
            .trips
            .iter()
            .filter(|trip| {
                // trip begins in this tick
                // if time is less than a minute after the start time, then we should spawn the agent.
                time.time() - trip.1.timings[0].0 >= Duration::zero()
                    && time.time() - trip.1.timings[0].0 < Duration::minutes(1)

                // times_relatively_equal(trip.1.timings[0].0, time.time())
            })
            .for_each(|(id, trip)| {
                println!(
                    "Spawning agent {}\t{:?}\t{:?}",
                    id,
                    time.time(),
                    trip.timings[0]
                );
                // Spawn a new agent
                self.buses.insert(
                    *id,
                    StaticAgent::new(*id, graph.clone(), self.network_data.clone(), self.analytics.clone()),
                );
            });

        // TODO: actually use a sane demand generation which is consistent across static/dynamic controllers
        let demand_queue = demand.generate_amount(2, &time, Err(self.network_data.clone()));
        let demand_queue: VecDeque<_> = demand_queue
            .into_iter()
            .map(|d| {
                let passenger = demand_to_passenger(d, graph.clone(), self.network_data.clone(), time, self.passenger_id);
                self.passenger_id += 1;
                passenger
            })
            .filter(|p| p.is_some()) // TODO: perhaps count the rejections
            .map(|p| p.unwrap())
            .collect();
        self.passengers.extend(demand_queue);

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
                        let id = p.id.clone();
                        agent::send_analytics(&self.analytics, AnalyticsPackage::VehicleEvent(VehicleAnalyticsEvent::PassengerDropoff { id: trip, passenger_id: id }));
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

    pub fn set_analytics(&mut self, tx: Option<Sender<AnalyticsPackage>>) {
        println!("[ANALYTICS] Set analytics to {:?}", tx.is_some());
        self.analytics = tx;
    }

    pub fn set_network_data(&mut self, data: Arc<NetworkData>) {
        self.network_data = data;
    }

    pub fn get_display(&self) -> Vec<Shape> {
        let mut shapes = Vec::new();
        self.buses
            .values()
            .for_each(|bus| shapes.push(bus.display()));

        shapes.extend(self.passengers.iter().map(|passenger| {
            Shape::circle_filled(
                pos2(passenger.source_pos.0 as f32, passenger.source_pos.1 as f32),
                1.0,
                Color32::LIGHT_RED,
            )
        }));

        shapes.extend(self.network_data.stops.iter().map(|stop| {
            Shape::circle_filled(
                pos2(stop.1.easting as f32, stop.1.northing as f32),
                1.0,
                Color32::LIGHT_BLUE,
            )
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
    id: u32, 
) -> Option<BusPassenger> {
    let source = demand.0;
    let dest = demand.1;

    // simple pathfinding plan:
    // 1. find the closest bus stop to the demand source (Control: Passenger walks to this bus stop)
    //  (a) if the distance to source stop is >30m the trip is rejected
    // 2. find the next trip from that bus stop which takes the passenger closest to the destination position (Control: Passenger takes trip to destination)
    //  (a) wrt to timings say there's a 20 minute max wait for a trip to get close to the destination
    // 3. at the next stop if it's say within 30 min walk the passenger can just walk (Control: walk to destination)
    //  (a) if the distance is longer try looking again at trips from this stop or the neighbourhood which take the passenger
    //      closer to the destination applying a smaller wait rule
    //  (b) if the distance is still too far, then the passenger will just walk to the destination

    let (source_bus_stop, source_dist) =
        closest_stop_to_point((source.0 as f64, source.1 as f64), network_data.clone());

    let (destination_bus_stop, dest_dist) =
        closest_stop_to_point((dest.0 as f64, dest.1 as f64), network_data.clone());

    let mut control = VecDeque::new();

    let HUMAN_WALKING_SPEED = 1.3; // human walking speed in m/s
    let MAX_DEPTH = 3; // max depth of search for a trip to the destination. If we can't find it within 3 trips, we just walk/reject

    if source_dist > HUMAN_WALKING_SPEED * 30.0 * 60.0 {
        // Reject the trip if its over 30 minutes from the source stop
        return None;
    } else {
        // push walking control to the source bus stop to start
        control.push_back(Control::walk_to_stop(source_bus_stop, None));
    }

    let mut stop_neighbourhood = routes::stop_neighbourhood_pos( (source.0 as f64, source.1 as f64) , HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());

    let end_neighbourhood = routes::stop_neighbourhood_pos( (dest.0 as f64, dest.1 as f64) , HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());
    if end_neighbourhood.is_empty() {
        // Reject the trip if the destination is not within 30 minutes of a bus stop
        return None;
    }

    let mut current_stop = source_bus_stop;
    loop {
        if control.len() > MAX_DEPTH * 2 {
            // Reject the trip if we can't find a trip to the destination within 3 trips
            return None;
        }

        // find a possible next trip
        let trip = stop_neighbourhood.iter().map(|id| {
            network_data.trips_from_stop.get(id).expect("Stop was not a stop").iter().map(|tid| {
                let trip = network_data.trips.get(tid).expect("Trip ID was not a trip");
                let stop_index = trip.stops.iter().position(|stop| *stop == *id).expect("Stop was not in trip");
                let stop_time = trip.timings[stop_index].0;
                (*id, tid, trip, stop_time)
            }).filter(|(_, _, _, stop_time)| {
                // filter for buses departing in the next 20 minutes
                stop_time >= &tick.time() && stop_time < &(tick + Duration::minutes(20)).time()
            }).map(|(sid, tid, trip, stop_time)| {
                // find the closest stop on the trip to the destination (and the arrival time)
                let closest_stop_info = trip.stops.iter().zip(trip.timings.iter()).filter(|(other_id, other_time)| {
                    other_time.0 > stop_time && *id != **other_id
                }).map(|(stop, timings)| {
                    let stop_data = network_data.stops.get(stop).expect("Stop was not a stop");
                    let dist = distance(stop_data.position(), (dest.0 as f64, dest.1 as f64));
                    (stop, timings.0, dist)
                // now find the closest stop to the destination
                }).min_by(|(_, _, dist_a), (_, _, dist_b)| dist_a.total_cmp(dist_b))
                .expect("No stops on trip were closer to destination than current stop");
                (sid, tid, trip, stop_time, closest_stop_info.0, closest_stop_info.1, closest_stop_info.2)
            })
        }).flatten().min_by(|first, second| {
            // find the trip which gets the passenger closest to the destination
            let min_dist = first.6.min(second.6); // Minimum distance to the destination out of both trips
            let timing_first = first.5 + Duration::seconds(((first.6 - min_dist) / HUMAN_WALKING_SPEED) as i64); 
            let timing_second = second.5 + Duration::seconds(((second.6 - min_dist) / HUMAN_WALKING_SPEED) as i64); 
            timing_first.cmp(&timing_second)
        });

        match trip {
            Some(trip_data) => {
                if current_stop != trip_data.0 { // if the start stop is not the current stop, we need to walk to it
                    control.push_back(Control::walk_to_stop(trip_data.0, Some(current_stop)));
                }
                control.push_back(Control::take_bus(*trip_data.1, trip_data.0, *trip_data.4));
                current_stop = *trip_data.4;
                stop_neighbourhood = routes::stop_neighbourhood(current_stop, HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());
            },
            None => {
                // if we can't find a trip to the destination, we just walk
                control.push_back(Control::walk_to_stop(destination_bus_stop, Some(current_stop)));
                break;
            }
        }
    }

    Some(BusPassenger {
        id,
        source_pos: (source.0 as f64, source.1 as f64),
        source_stop: source_bus_stop,

        dest_pos: (dest.0 as f64, dest.1 as f64),
        dest_stop: destination_bus_stop,

        instructions: control,

        ..Default::default()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    TakeBus {trip_id: u32, source: u32, destination: u32},
    WalkToStop { destination_stop: u32, source_stop: Option<u32> },
}

impl Control {
    pub fn take_bus(trip_id: u32, source: u32, destination: u32) -> Control {
        Control::TakeBus { trip_id, source, destination }
    }

    pub fn walk_to_stop(destination: u32, source: Option<u32>) -> Control {
        Control::WalkToStop { destination_stop: destination, source_stop: source }
    }
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
