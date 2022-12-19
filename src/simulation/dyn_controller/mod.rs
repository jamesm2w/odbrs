use std::{sync::Arc, collections::VecDeque};

use chrono::{DateTime, Utc};

use crate::graph::{route_finding, transform::convert_point, Graph};

use self::bus::{Bus, Passenger};

use super::{Controller, demand::{DemandGenerator, Demand}};

pub mod bus;
pub mod waypoints;

#[derive(Default)]
pub struct DynamicController {
    id: u8,
    buses: Vec<Bus>,
    demands: VecDeque<Passenger>
}

impl DynamicController {
    // pub fn new() -> Self {
    //     DynamicController { id: 0, buses: vec![], demands: VecDeque::new() }
    // }

    // Construct a new/partial solution -- try assignments and see which minimises 
    pub fn constructive(&mut self, graph: &Graph) {
        println!("Run Constructive Heuristic");
        // add one request p:
        // for each bus b do
        //  for each position n in the bus do
        //    find origin station that causes the smallest increase in route duration
        //    check feasiblity (time windows and capacity violations)
        //    if feasible origin insertion then
        //       for every position >= n in bus b do
        //         find arrival station that causes the smallest increase in route duration
        //         check feasibility (time window and capacity violations)
        //         insertion criterion = ride time(p) + delta ride time + Penalty
        //         if feasible and insertion criterion < best insertion criterion found then
        //            save this insertion;
        // if feasible insertion found:
        //     preform best insertion

        // while demands && a bus can have insertions 
        println!("Demand size: {}", self.demands.len());
        while !self.demands.is_empty() && self.buses.iter().any(|b| b.can_assign_more()) {
            for bus in self.buses.iter_mut() {
                println!("Starting again from bus: {}", bus.agent_id);
                let mut min_increase = u32::MAX;
                let mut min_demand = (0, 0, DateTime::<Utc>::MIN_UTC);
                let mut min_demand_raw = Demand((0.0, 0.0), (0.0, 0.0), DateTime::<Utc>::MIN_UTC);

                for demand in self.demands.iter() {
                    println!("\tAssigning to bus: {:?}; demand [...]", bus.agent_id);
                    // use BFS with heuristic being straigh line distance
                    // try bus route with this demand
                    // if distance < max distance so far: save this as an insertion to use
                    let origin = route_finding::closest_node(convert_point(demand.0), graph);
                    let dest = route_finding::closest_node(convert_point(demand.1), graph);
                    
                    let route = bus.route_with_node(origin, dest, graph);
                    // println!("Route: {:?}", route); 
                    let route_len = route_finding::route_length(&route, graph);
                    println!("\tRoute length: {}", route_len);
                    if route_len < min_increase {
                        // save this as an insertion to use
                        min_increase = route_len;
                        min_demand = (origin, dest, demand.2);
                        min_demand_raw = demand.clone();
                    }
                }
                
                self.demands.retain(|d| d != &min_demand_raw);
                bus.add_passenger_to_assignment(passenger)
            }
        }
    }

    // destroy a solution
    pub fn destructive(&mut self, _graph: &Graph) {
        println!("Run Destructive Heuristic");
        // Go through and destroy the solutions and reclaim the demand into the main demand list
        for bus in self.buses.iter_mut() {
            self.demands.extend(&mut bus.destructive().into_iter());
        }
    
    }

    /// do any static assignments first (we shouldnt have any)
    /// for each dynamic request r
    ///     current time = e.early_depart - lead time - stop time
    ///     count of Rs += 1
    ///     LP = locking point(current time)
    ///     insert r in the solution after the Locking Point using the greedy constructive heuristic
    ///     iterations = 0
    ///     if passenger == count of Rs then
    ///         while iterations < dynamic iterations constant do
    ///             destroy and repair (optimising minswurt) after the LP
    ///             iterations += 1
    ///             if p == count of Rs then
    ///                 local search to optimise
    ///     else
    ///         while p < count of Rs and until stop criterion
    ///             destroy and repair (optimising max passengers) after the LP
    ///         if p == count of Rs then
    ///             while iterations < dynamic iterations constant do
    ///                 destroy and repair (optimising minswurt) after the LP
    ///                 iterations += 1
    ///             if p == count of Rs then
    ///                 local search to optimise
    ///         else
    ///             go back to the solution before trying to insert r
    /// 
    pub fn large_neighbourhood_search(&mut self, graph: &Graph) {
        
        let max_iter_count = 10;
        let mut iter_count = 0;

        while iter_count < max_iter_count {
            dbg!("LNS iter {}", iter_count);
            self.destructive(graph);
            self.constructive(graph);
            iter_count += 1;
        }
    }
}

impl Controller for DynamicController {
    type Agent = Bus;

    fn get_agents(&self) -> &Vec<Self::Agent> {
        &self.buses
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> &Self::Agent {
        println!("Spawning new bus");
        self.id += 1;
        let bus = Bus::new(graph.clone(), 20, self.id);
        self.buses.push(bus);
        self.buses.last().expect("Couldn't create new agent")
    }

    fn update_agents(&mut self, graph: std::sync::Arc<crate::graph::Graph>, demand: Arc<DemandGenerator>, time: DateTime<Utc>) {
        println!("Updating agents");
        self.buses.iter_mut().for_each(|b| b.move_self());

        if self.buses[0].assignment.len() == 0 { // TODO: just for testing only do the gen once 
            println!("Getting demands");
        
            let mut demand_queue = demand.generate_amount(2, &time);
            self.demands.append(&mut demand_queue);    
        }

        println!("Running LNS");
        self.large_neighbourhood_search(&graph);
    }
}