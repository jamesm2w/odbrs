use std::{collections::VecDeque, sync::Arc};

use chrono::{DateTime, Utc};

use crate::{graph::{route_finding, transform::convert_point, Graph}, simulation::dyn_controller::bus::Status};

use self::bus::{Bus, Passenger};

use super::{
    demand::{Demand, DemandGenerator},
    Controller,
};

pub mod bus;
pub mod waypoints;

#[derive(Default)]
pub struct DynamicController {
    id: u8,
    buses: Vec<Bus>,
    demands: VecDeque<Passenger>,
}

impl DynamicController {
    // pub fn new() -> Self {
    //     DynamicController { id: 0, buses: vec![], demands: VecDeque::new() }
    // }

    // Construct a new/partial solution -- try assignments and see which minimises
    pub fn constructive(&mut self, graph: Arc<Graph>) {
        println!("[LNS] Run Constructive Heuristic");
        // All passengers in the demand queue are not assigned so shoud be generated
        // TODO: maybe change this to waiting or something based on where passenger is
        self.demands.iter_mut().for_each(|p| {
            p.status = Status::Generated;
        });

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
        println!("[LNS] Demand size: {}", self.demands.len());
        
        while !self.demands.is_empty() && self.buses.iter().any(|b| b.can_assign_more()) {
            println!("[LNS] demand size: {}, can buses assign? {:?}", self.demands.len(), self.buses.iter().any(|b| b.can_assign_more()));
            
            let mut min_assignment: Option<(f64, usize, &Passenger)> = None;
            
            for i in 0..self.buses.len() {
                let bus = &mut self.buses[i];
                println!("[LNS]\tAnalysing with bus: {}", bus.agent_id);

                for demand in self.demands.iter() {
                    println!("[LNS]\t\t Testing assignment to bus: {:?}; demand {:?}", bus.agent_id, demand.dest_pos);
                    // use BFS with heuristic being straigh line distance
                    // try bus route with this demand
                    // if distance < max distance so far: save this as an insertion to use

                    let route_len = bus.what_if_bus_had_passenger(demand);

                    println!("[LNS]\t\t Resultant Route length: {}", route_len);
                    if route_len < min_assignment.map(|(len, _, _)| len).unwrap_or(f64::MAX) {
                        println!("[LNS]\t\t New Minimum Found");
                        // save this as an insertion to use
                        min_assignment = Some((route_len, i, demand));
                    }
                }
            }

            if let Some((_, bus_i, demand)) = min_assignment {
                let bus = &mut self.buses[bus_i];
                println!("[LNS] Performing constructive insertion for bus: {}; demand {:?}", bus.agent_id, demand.dest_pos);
                let index = self.demands.iter().position(|d| d == demand).unwrap();
                let passenger = self.demands.remove(index).unwrap();
                bus.constructive(passenger);
            }
        }
    }

    // destroy a solution
    pub fn destructive(&mut self, _graph: Arc<Graph>) {
        println!("[LNS] Run Destructive Heuristic");
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
    pub fn large_neighbourhood_search(&mut self, graph: Arc<Graph>) {
        let max_iter_count = 1; // TODO: increase this 
        let mut iter_count = 0;

        while iter_count < max_iter_count {
            self.destructive(graph.clone());
            self.constructive(graph.clone());
            iter_count += 1;
        }
    }
}

impl Controller for DynamicController {
    type Agent = Bus;
    
    fn get_agents(&self) -> Vec<&Self::Agent> {
        self.buses.iter().collect()
    }

    fn spawn_agent(&mut self, graph: Arc<crate::graph::Graph>) -> Option<&Self::Agent> {
        println!("Spawning new bus");
        self.id += 1;
        let bus = Bus::new(graph.clone(), 20, self.id);
        self.buses.push(bus);
        Some(self.buses.last().expect("Couldn't create new agent"))
    }

    fn update_agents(
        &mut self,
        graph: Arc<crate::graph::Graph>,
        demand: Arc<DemandGenerator>,
        time: DateTime<Utc>,
    ) {
        println!("Updating agents");
        self.buses.iter_mut().for_each(|b| b.move_self());

        // if self.buses[0].assignment.len() == 0 {
        //     // TODO: just for testing only do the gen once
        //     println!("[LNS] Getting demands");

        let demand_queue = demand.generate_amount(2, &time);
        let mut demand_queue = demand_queue.into_iter().map(|d| demand_to_passenger(d, graph.clone())).collect();
        self.demands.append(&mut demand_queue);
        // }

        println!("[LNS] Running LNS");
        self.large_neighbourhood_search(graph);
    }
}

// convert generated demand object into a passenger object
pub fn demand_to_passenger(demand: Demand, graph: Arc<Graph>) -> Passenger {
    let origin = route_finding::closest_node(convert_point(demand.0), &graph);
    let dest = route_finding::closest_node(convert_point(demand.1), &graph);
    let time = demand.2;
    // Passenger::new(origin, dest, time)
    Passenger {
        source_node: origin,
        source_pos: (demand.0.0 as f64, demand.0.1 as f64),
        dest_node: dest,
        dest_pos: (demand.1.0 as f64, demand.1.1 as f64),
        timeframe: time,
        ..Default::default()
    }
}
