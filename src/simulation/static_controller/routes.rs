//! Define a bunch of stuff for handling GTFS data of bus routes and stops

use chrono::NaiveTime;
use gtfs_structures::{Gtfs, RouteType, Stop, Trip};
use proj::Proj;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::Arc,
};

use crate::{
    graph::{route_finding, Graph}
};

use super::distance;

// Load the GTFS data and create an serialised version for quick loading in the application
pub fn load_routes() {
    let data = Gtfs::new("data/gtfs/tfwm_gtfs/").unwrap();
    println!("load time: {:?}", data.read_duration);

    data.print_stats();

    let proj_instance = Proj::new_known_crs("EPSG:4326", "EPSG:27700", None).unwrap();

    let left = 425174.28;
    let right = 439679.25;
    let top = 286113.25;
    let bottom = 273637.59;
    let mut i = 0;

    let mut stop_id = 0_u32;
    let mut trip_id = 0_u32;

    let valid_stops: HashSet<String> = HashSet::from_iter(
        data.stops
            .iter()
            .filter(|(_, stop)| {
                let lat = stop.latitude.expect("Stop has no latitude");
                let lng = stop.longitude.expect("Stop as no longitude");
                let (easting, northing) = proj_instance.convert((lng, lat)).unwrap();
                left < easting && easting < right && bottom < northing && northing < top
            })
            .map(|(id, _)| id.clone()),
    );

    println!("Valid stops inside bounding box: {}", valid_stops.len());

    let mut valid_routes: HashSet<String> = HashSet::new(); // Valid routes are those that have at least one valid trip

    // All stops which are used and valid
    let mut used_stops: HashMap<String, u32> = HashMap::new();

    let mut trips_from_stop: HashMap<String, Vec<String>> = HashMap::new();

    // Set of trip ids which are valid (all within the bounding box)
    let valid_trips: HashMap<String, u32> = HashMap::from_iter(
        data.trips
            .iter()
            .filter(|(_, trip)| {
                i += 1;
                print!("processed trip {:?}\r", i);
                trip.stop_times.iter().all(|stop| {
                    valid_stops.contains(&stop.stop.id) && stop.arrival_time.unwrap() < 86400//21600 //86400
                }) && data.get_route(&trip.route_id).unwrap().route_type == RouteType::Bus
            })
            .map(|(id, trip)| {
                valid_routes.insert(trip.route_id.clone());

                used_stops.extend(trip.stop_times.iter().map(|stop| {
                    stop_id += 1;
                    (stop.stop.id.clone(), stop_id)
                }));

                trip.stop_times.iter().for_each(|stop| {
                    trips_from_stop
                        .entry(stop.stop.id.clone())
                        .or_insert_with(Vec::new)
                        .push(id.clone());
                });

                trip_id += 1;
                (id.clone(), trip_id)
            }),
    );

    println!("Valid trips inside bounding box: {}", valid_trips.len());
    println!("Valid route inside bounding box: {}", valid_routes.len());
    println!("Used stops inside bounding box: {}", used_stops.len());

    // println!("{:?}", valid_trips.iter().take(5).map(|id| {
    //     let trip = data.get_trip(id).unwrap();
    //     (trip.stop_times[0].arrival_time, trip.stop_times[0].departure_time)
    // }).collect::<Vec<_>>())

    // Collect a list of Vec<( coords, timings )> basically summarises the bus network
    // Can then match coords to the graph. Need to do some route finding s.t. buses follow the nodes
    // but stop at the stops which might not be "at" nodes. Plus realising the bus stop is on a given edge
    // then when travelling if bus within epsilon of stop coord then can basically deal with stop.
    // also need to do routing for the passengers (just one bus or multiple?) and making them wait, get on, get off,
    // transfer, and making sure they can walk the last bits.
    // or could just ensure their destination is a bus stop :shrug:

    let mut network_data = NetworkData::default();

    network_data.trips = HashMap::from_iter(valid_trips.iter().map(|(id, num)| {
        let trip = data.get_trip(id).unwrap();
        (*num, make_network_trip(&trip, &used_stops))
    }));

    network_data.stops = HashMap::from_iter(used_stops.iter().map(|(id, num)| {
        let stop = data.get_stop(id).unwrap();
        (*num, Arc::new(make_network_stop(stop, &proj_instance)))
    }));

    network_data.trips_from_stop =
        HashMap::from_iter(trips_from_stop.iter().map(|(str_stop_id, str_trip_ids)| {
            let stop = used_stops.get(str_stop_id).unwrap();
            let trips = str_trip_ids
                .iter()
                .map(|str_trip_id| *valid_trips.get(str_trip_id).unwrap())
                .collect();
            (*stop, trips)
        }));

    println!("Finished creating new network data. Writing to file...");

    // Serialise the network data with ciborium
    let file = std::fs::File::create("data/gtfs/tfwm_gtfs/network_data.bin").unwrap();
    // ciborium::to_writer(&mut file, &network_data).unwrap();
    ciborium::ser::into_writer(&network_data, file).expect("Failed to serialise network data");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStop {
    pub easting: f64,
    pub northing: f64,
    pub stop_id: String,
}

impl NetworkStop {
    pub fn position(&self) -> (f64, f64) {
        (self.easting, self.northing)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStop {
    pub eastnorthing: (f64, f64),
    pub stop_id: String,
    pub edge_id: u32,
    pub edge_offset: f64, // Length along the edge from start -> stop
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTrip {
    pub trip_id: String,
    pub stops: Vec<u32>, // vector of stop id
    pub timings: Vec<(NaiveTime, NaiveTime)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
// Represent the valid and important GTFS data
pub struct NetworkData {
    pub trips: HashMap<u32, NetworkTrip>, // Map trip ID to trip data,
    pub stops: HashMap<u32, Arc<NetworkStop>>, // Map stop ID to stop reference
    pub trips_from_stop: HashMap<u32, Vec<u32>>, // Map stop ID to trip IDs
}

pub fn make_network_stop(stop: &Stop, proj_instance: &Proj) -> NetworkStop {
    let (easting, northing) = proj_instance
        .convert((stop.longitude.unwrap(), stop.latitude.unwrap()))
        .unwrap();
    NetworkStop {
        easting,
        northing,
        stop_id: stop.id.clone(),
    }
}

pub fn make_network_trip(trip: &Trip, stop_map: &HashMap<String, u32>) -> NetworkTrip {
    let mut stops = Vec::new();
    let mut timings = Vec::new();

    trip.stop_times.iter().for_each(|stop| {
        stops.push(stop_map.get(&stop.stop.id).unwrap().clone());

        let arrival = timeint_to_time(stop.arrival_time.unwrap());
        let departure = timeint_to_time(stop.departure_time.unwrap());

        timings.push((arrival, departure));
    });

    NetworkTrip {
        trip_id: trip.id.clone(),
        stops,
        timings,
    }
}

pub fn closest_stop_to_point(point: (f64, f64), network_data: Arc<NetworkData>) -> (u32, f64) {
    let mut min_distance = f64::MAX;
    let mut closest_stop = None;

    for (id, stop) in network_data.stops.iter() {
        let distance = (stop.easting - point.0).powi(2) + (stop.northing - point.1).powi(2);
        if distance < min_distance {
            min_distance = distance;
            closest_stop = Some(id);
        }
    }

    (closest_stop.unwrap().clone(), min_distance)
}

pub fn get_graph_edge_from_stop(stop: &NetworkStop, graph: Arc<Graph>) -> u128 {
    let mut min_distance = f64::MAX;
    let mut closest_edge = None;

    let stop_point = stop.position();

    for (id, edge) in graph.get_edgelist() {
        // println!("GGEFS: Examining {:?}", id);
        let edge_u = edge.points.first().unwrap();
        let edge_v = edge.points.last().unwrap();
        // println!("GGEFS: Edge u: {:?}\t v: {:?}", edge_u, edge_v);

        // let u_v = (edge_v.0 - edge_u.0, edge_v.1 - edge_u.1);
        // let u_p = (stop.easting - edge_u.0, stop.northing - edge_u.1);

        // println!("GGEFS: u_v: {:?}\t u_p: {:?}", u_v, u_p);

        // let proj = (u_v.0 * u_p.0 + u_v.1 * u_p.1) / (u_v.0.powi(2) + u_v.1.powi(2));
        // let u_v_len2 = u_v.0.powi(2) + u_v.1.powi(2);
        // let distance = proj / u_v_len2;

        // println!("GGEFS: proj: {:?}\t u_v_len2: {:?}\t distance: {:?}", proj, u_v_len2, distance);

        let distance = dist_point_linesegment_2([*edge_u, *edge_v], stop_point);

        if distance < min_distance {
            min_distance = distance;
            closest_edge = Some(id);
        }
    }

    *closest_edge.unwrap()
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

// Converts a trip to a vector of nodes, returns (vector of nodes (path), vector of edges (stop edges))
pub fn convert_trip_to_graph_path(
    trip: u32,
    graph: Arc<Graph>,
    network_data: Arc<NetworkData>,
) -> (Vec<u128>, Vec<u128>) {
    // list of the stop edges which need to be joined by edges inbetween
    let trip = network_data.trips.get(&trip).expect("Trip not found");
    let mut edges = Vec::new();

    for stop in trip.stops.iter() {
        let stop = network_data.stops.get(stop).unwrap();
        let edge = get_graph_edge_from_stop(stop, graph.clone());
        edges.push(edge);
    }

    let mut route = Vec::new();

    // for i in 0..edges.len() - 1 {
    //     let edge = edges[i];
    //     let next_edge = edges[i + 1];
    //     let partial_route = route_finding::best_first_edge_route(edge, next_edge, graph.clone());
    //     route.extend(partial_route);
    // }
    for i in 0..edges.len() {
        let edge_id = edges[i];
        let edge_data = graph.get_edgelist().get(&edge_id).expect("Edge referenced in trip does not exist");

        let start_node_id = edge_data.start_id;
        let start_node_data = graph.get_nodelist().get(&start_node_id).expect("Node referenced in trip does not exist");

        let end_node_id = edge_data.end_id;
        let end_node_data = graph.get_nodelist().get(&end_node_id).expect("Node referenced in trip does not exist");

        match route.last() {
            Some(prev_node) => {
                let prev_node_data = graph.get_nodelist().get(prev_node).expect("Node referenced in trip does not exist");
                
                let start_distance = distance(start_node_data.point, prev_node_data.point);
                let end_distance = distance(end_node_data.point, prev_node_data.point);
                
                let target_node = if start_distance <= end_distance {
                    start_node_id
                } else {
                    end_node_id
                };

                let subroute = route_finding::find_route(&graph, *prev_node, target_node);
                route.extend(subroute.into_iter().rev()); //TODO: might need to skip 1 or add destination on at end
            },
            None if i == 0 => {
                let next_stop = trip.stops[i + 1];
                let next_stop_position = network_data.stops.get(&next_stop).expect("Stop referenced in trip does not exist").position();
                               
                let start_distance = distance(start_node_data.point, next_stop_position);
                let end_distance = distance(end_node_data.point, next_stop_position);
                
                if start_distance <= end_distance {
                    route.push(edge_data.start_id);
                } else {
                    route.push(edge_data.end_id);
                }
            },
            None => unreachable!("The first edge in the trip should be the first node we add in the route")
        }
    }

    (route, edges)
}

pub fn load_saved_network_data() -> Option<NetworkData> {
    ciborium::de::from_reader(fs::File::open("data/gtfs/tfwm_gtfs/network_data.bin").unwrap()).ok()
}

pub fn timeint_to_time(time: u32) -> chrono::NaiveTime {
    // let (time, sec) = (time / 60, time % 60);
    // let (time, min) = (time / 60, time % 60);
    // let hours = time / 60;

    NaiveTime::from_num_seconds_from_midnight(time, 0)
    // chrono::NaiveTime::from_hms(hours as u32, min as u32, sec as u32)
}

pub fn stop_neighbourhood_pos(pos: (f64, f64), threshold: f64, network_data: Arc<NetworkData>) -> Vec<u32> {
    network_data.stops.iter().filter(|(_, stop)| {
        let stop_pos = stop.position();
        distance(pos, stop_pos) <= threshold
    }).map(|(id, _)| *id).collect()
}

pub fn stop_neighbourhood(stop: u32, threshold: f64, network_data: Arc<NetworkData>) -> Vec<u32> {
    let stop = network_data.stops.get(&stop).expect("Stop not found");
    let pos = stop.position();
    stop_neighbourhood_pos(pos, threshold, network_data)
}

#[cfg(test)]
mod test {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_load_routes() {
        load_routes();

        let timer = Instant::now();
        let data = load_saved_network_data().unwrap();
        println!("Loaded network data in {}ms", timer.elapsed().as_millis());
        println!("data tip len: {}", data.trips.len());
    }
}
