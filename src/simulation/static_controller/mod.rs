//! Controller which handles the static case, i.e. traditional buses which get demand but do not respond to it.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, mpsc::Sender},
};

use chrono::{DateTime, Duration, NaiveTime, Utc};
use eframe::epaint::{pos2, Color32, Shape};

use crate::{graph::Graph, analytics::AnalyticsPackage};

use self::{
    agent::{BusPassenger, StaticAgent, PassengerStatus},
    routes::{closest_stop_to_point, NetworkData},
};

use super::{demand::Demand, Agent, Controller};

pub mod agent;
pub mod routes;

#[derive(Default)]
pub struct StaticController {
    buses: HashMap<u32, StaticAgent>, // Each 'bus' gets a trip
    network_data: Arc<NetworkData>,
    passenger_pool: Vec<BusPassenger>,
    analytics: Option<Sender<AnalyticsPackage>>,
    passenger_id: u32,
}

impl Controller for StaticController {
    type Agent = StaticAgent;

    fn get_agents(&self) -> Vec<&Self::Agent> {
        self.buses.values().collect()
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> Option<&Self::Agent> {
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
            })
            .for_each(|(id, trip)| {
                println!(
                    "\tSpawning agent {}\t{:?}\t{:?}",
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

        let demand_queue = demand.generate_scaled_amount(1.0, &time, Err(self.network_data.clone()));
        let demand_queue: VecDeque<_> = demand_queue
            .into_iter()
            .map(|d| {
                let passenger = demand_to_passenger(d, graph.clone(), self.network_data.clone(), time, self.passenger_id, self.analytics.clone());
                self.passenger_id += 1;
                passenger
            })
            .filter(|p| p.is_some()) 
            .map(|p| p.unwrap())
            .collect();
        self.passenger_pool.extend(demand_queue);

        for agent in self.buses.values_mut() {
            let trip_id = agent.trip_id;
            let capacity = agent.get_capacity();
            
            // Fire the agent update function
            agent.move_self(time, |trip, stop, mut drop_off_passengers| {
                
                let mut get_on_passengers = Vec::new();
                let mut i = 0;
                while i < self.passenger_pool.len() {
                    let passenger = self.passenger_pool.get(i).unwrap();
                    
                    // Ensure the passenger wants to get on this bus & that we're not gonna add more passengers than capacity left
                    if passenger.should_get_on(trip, stop, self.network_data.clone()) && get_on_passengers.len() < capacity {
                        get_on_passengers.push(self.passenger_pool.remove(i));
                    } else {
                        i += 1;
                    }
                }

                drop_off_passengers.iter_mut().for_each(|p| {
                    p.get_off_bus(trip_id);
                });

                self.passenger_pool
                    .extend(drop_off_passengers.into_iter());

                get_on_passengers
            });
        }

        // have some passenger update cycle which feeds into the analytics
        self.passenger_pool.iter_mut().for_each(|p| {
            p.update(self.network_data.clone());
        });
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

        shapes.extend(self.passenger_pool.iter().filter(|p| p.status != PassengerStatus::Finished).map(|passenger| {
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
    analytics: Option<Sender<AnalyticsPackage>>,
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

    let control = basic_route_finding(source_bus_stop, destination_bus_stop, (source.0 as f64, source.1 as f64), tick, network_data.clone());

    // let status = match control.first() {
    //     None => PassengerStatus::Finished,
    //     Some(Control::TakeBus { .. }) => {
    //         PassengerStatus::Waiting
    //     },
    //     Some(Control::WalkToStop { destination_stop, .. }) => {
    //         let stop_data = network_data.stops.get(destination_stop).expect("Stop was not a stop");
    //         let dist = distance((source.0 as f64, source.1 as f64), stop_data.position());
    //         PassengerStatus::Walking((dist / (60.0 * 1.4)) as u32)
    //     }
    // };

    Some(BusPassenger {
        id,
        source_pos: (source.0 as f64, source.1 as f64),
        source_stop: source_bus_stop,

        dest_pos: (dest.0 as f64, dest.1 as f64),
        dest_stop: destination_bus_stop,

        instructions: VecDeque::from_iter(control.into_iter()),
        status: PassengerStatus::Generated,
        analytics,
    })
}

// Very basic route finding for passenger
// just get source stop and take next trip closest to destination
pub fn basic_route_finding(source_stop: u32, dest_stop: u32, source_pos: (f64, f64), tick: DateTime<Utc>, network_data: Arc<NetworkData>) -> Vec<Control> {
    let dest_stop_data = network_data.stops.get(&dest_stop).expect("Stop was not a stop");
    let mut control = Vec::new();
    let trips_from_source = network_data.trips_from_stop.get(&source_stop).expect("Stop was not a stop");


    control.push(Control::walk_to_stop(source_stop, source_pos));
    // control.push(Control::walk_to_stop(source_stop, None));

    let mut min_trip_dist = f64::MAX;
    let mut min_trip = 0;
    let mut min_trip_end_stop = 0;

    for trip in trips_from_source.iter().filter(|trip| {
        // Filter for trips which are departing fairly soon-ish
        let trip_data = network_data.trips.get(trip).expect("Trip ID was not a trip");
        let trip_arrival_time = trip_data.timings.get(trip_data.stops.iter().enumerate().find_map(|(i, stop)|if *stop == source_stop { Some(i) } else { None }).unwrap() as usize).unwrap_or_else(|| panic!("Mismatch in length of timings and stop vectors for trip\n\ttimings:  {:?}\n\tstops: {:?}", trip_data.timings, trip_data.stops)).0;
        
        trip_arrival_time >= tick.time() && trip_arrival_time < (tick + Duration::minutes(20)).time()
        // trip_arrival_time.is_some() && trip_arrival_time.unwrap() > &Utc::now().time()
    }) {
        let trip_data = network_data.trips.get(trip).expect("Trip ID was not a trip");
        let trip_stops = &trip_data.stops;
        let mut min_trip_stop_dist = f64::MAX;
        let mut min_trip_stop = 0;

        for stop in trip_stops {
            let stop_data = network_data.stops.get(stop).expect("Stop was not a stop");
            let dist = distance(stop_data.position(), dest_stop_data.position());
            if dist < min_trip_stop_dist {
                min_trip_stop_dist = dist;
                min_trip_stop = *stop;
            }
        }

        if min_trip_stop_dist < min_trip_dist {
            min_trip_dist = min_trip_stop_dist;
            min_trip = *trip;
            min_trip_end_stop = min_trip_stop;
        }
    }

    control.push(Control::take_bus(min_trip, source_stop, min_trip_end_stop));
    control.push(Control { destination_stop: dest_stop, source: Ok(min_trip_end_stop) });
    control
}

// Full route finding for passenger
// try to get to the destination stop exactly
pub fn full_route_finding(source: (f32, f32), dest: (f32, f32), network_data: Arc<NetworkData>, tick: DateTime<Utc>) -> VecDeque<Control> {
    let (source_bus_stop, source_dist) =
        closest_stop_to_point((source.0 as f64, source.1 as f64), network_data.clone());

    let (destination_bus_stop, dest_dist) =
        closest_stop_to_point((dest.0 as f64, dest.1 as f64), network_data.clone());
    let mut control = VecDeque::new();

    let HUMAN_WALKING_SPEED = 1.4; // human walking speed in m/s
    let MAX_DEPTH = 3; // max depth of search for a trip to the destination. If we can't find it within 3 trips, we just walk/reject

    // push walking control to the source bus stop to start
    control.push_back(Control::walk_to_stop(source_bus_stop, (source.0 as _, source.1 as _)));

    let mut stop_neighbourhood = routes::stop_neighbourhood_pos( (source.0 as f64, source.1 as f64) , HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());
    let end_neighbourhood = routes::stop_neighbourhood_pos( (dest.0 as f64, dest.1 as f64) , HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());

    let mut current_stop = source_bus_stop;
    loop {
        if control.len() > MAX_DEPTH * 2 {
            // Reject the trip if we can't find a trip to the destination within 3 trips
            return control;
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
                    control.push_back(Control::take_bus(0, trip_data.0, current_stop));
                }
                control.push_back(Control::take_bus(*trip_data.1, trip_data.0, *trip_data.4));
                current_stop = *trip_data.4;
                stop_neighbourhood = routes::stop_neighbourhood(current_stop, HUMAN_WALKING_SPEED * 30.0 * 60.0, network_data.clone());
            },
            None => {
                // if we can't find a trip to the destination, we just walk
                control.push_back(Control::take_bus(0, destination_bus_stop, current_stop));
                break;
            }
        }
    }

    control
}

#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    pub destination_stop: u32, // The stop we're going to
    pub source: Result<u32, (f64, f64)>, // Give the source stop if we're walking from a stop, or the source position if we're walking from a position
}

impl Control {
    pub fn take_bus(trip_id: u32, source: u32, destination: u32) -> Control {
        Control { destination_stop: destination, source: Ok(source) }
    }

    pub fn walk_to_stop(destination: u32, source: (f64, f64)) -> Control {
        Control { destination_stop: destination, source: Err(source) }
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
