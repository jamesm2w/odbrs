use std::sync::Arc;
use chrono::{DateTime, Utc};
// TODO: Fix the issue with movement in random controller
use eframe::epaint::Vec2;
use rand::Rng;

use crate::graph::Graph;

use super::{Controller, Agent, demand::DemandGenerator};

#[derive(Default, Debug)]
pub struct RandomController {
    pub agentc: usize,
    pub agents: Vec<RandomAgent>,
}

#[derive(Debug)]
pub struct RandomAgent {
    pub id: u8,
    pub prev_node: u128,      // Node the agent came from
    pub cur_edge: u128,       // Edge it's currently traversing
    pub velocity: f64, // Velocity of the agent (+ive = from prev->other end, -ive = reverse) IN MS-1
    pub position: (f64, f64), // Current map-coord position of the agent
    pub graph: Arc<Graph>
}

impl Agent for RandomAgent {

    fn get_graph(&self) -> Arc<Graph> {
        self.graph.clone()
    }

    fn get_current_element(&self) -> super::dyn_controller::bus::CurrentElement {
        super::dyn_controller::bus::CurrentElement::Edge { edge: self.cur_edge, prev_node: self.prev_node }
    }

    fn get_next_node(&self) -> u128 {
        let edge_data = self.graph.get_edgelist().get(&self.cur_edge).expect("Edge not found");
        if edge_data.start_id == self.prev_node {
            edge_data.end_id
        } else {
            edge_data.start_id
        }
    }

    fn get_position(&self) -> (f64, f64) {
        self.position
    }
}

impl Controller for RandomController {
    type Agent = RandomAgent;

    fn get_agents(&self) -> &Vec<Self::Agent> {
        &self.agents
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> &Self::Agent {
        self.agentc += 1;
        let mut rng = rand::thread_rng();

        let random_node_i = rng.gen_range(0..=graph.get_nodelist().len() - 1);
        let node = graph.get_nodelist().keys().nth(random_node_i).unwrap();
        let adjacency = graph.get_adjacency().get(node).unwrap();
        let random_edge_i = rng.gen_range(0..=adjacency.len() - 1);
        let edge = adjacency.get(random_edge_i).unwrap();

        let agent = Self::Agent {
            id: self.agentc as _,
            cur_edge: *edge,
            prev_node: *node,
            velocity: 13.4112 * 60.0, // 30 MPH into ms-1 * 60 for 1 minute per tick
            position: graph.get_nodelist().get(node).unwrap().point,
            graph: graph.clone()
        };
        self.agents.push(agent);
        // //println!("Spawned agent {:?}", agent);
        self.agents.last().expect("Error creating random agent")
    }

    fn update_agents(&mut self, graph: std::sync::Arc<crate::graph::Graph>, _demand: Arc<DemandGenerator>, _time: DateTime<Utc>) {
        // self.agents.iter_mut().for_each(|agent| self.move_agent(agent, graph.clone()));
        for agent in self.agents.iter_mut() {
            Self::move_agent(agent, graph.clone());
        }
    }
}

impl RandomController {
    fn move_agent(agent: &mut RandomAgent, graph: Arc<Graph>) {
        let mut distance_to_move = agent.velocity as f32;
        //println!("NEW AGENT agent #{:?} moving {:?}", agent.id, distance_to_move);
        while distance_to_move > 0.0 {
            let current_edge = graph
                .get_edgelist()
                .get(&agent.cur_edge)
                .expect("Current Edge Was not an Edge");

            let prev_node = graph
                .get_nodelist()
                .get(&agent.prev_node)
                .expect("Prev Node was not a Node");

            let next_node_id = if current_edge.end_id == prev_node.id {
                current_edge.start_id
            } else {
                current_edge.end_id
            };
            let next_node = graph
                .get_nodelist()
                .get(&next_node_id)
                .expect("Next Node was not a Node");

            let line_points_iter = current_edge.points.iter().map(|(x, y)| Vec2 {
                x: *x as _,
                y: *y as _,
            });

            let line_points = if *current_edge.points.first().unwrap() == next_node.point {
                line_points_iter.rev().collect::<Vec<_>>()
            } else {
                line_points_iter.collect::<Vec<_>>()
            };

            let mut agent_pos = Vec2 {
                x: agent.position.0 as _,
                y: agent.position.1 as _,
            };

            for i in 0..line_points.len() - 1 {
                let start = line_points[i];
                let end = line_points[i + 1];

                // On this line segment
                if point_on_line(start, end, agent_pos) {
                    let line_distance_remaining = end - agent_pos;
                    if distance_to_move > line_distance_remaining.length() {
                        // Move to end of line segment bit
                        agent_pos = end;
                        distance_to_move -= line_distance_remaining.length(); // reduce distance needed by amount moved
                    } else {
                        // Can move a the given distance along this segment
                        agent_pos += ((end - start) / (end - start).length()) * distance_to_move;
                        return
                    }
                }
            }

            agent.position = (agent_pos.x as _, agent_pos.y as _);

            if distance_to_move > 0.0 {
                // Need to move to next node in graph
                agent.prev_node = next_node_id;
                let adjacency = graph.get_adjacency().get(&next_node_id).unwrap();
                loop {
                    let next_edge_i = rand::thread_rng().gen_range(0..=adjacency.len() - 1);
                    agent.cur_edge = adjacency.get(next_edge_i).unwrap().clone();
                    let current_edge = graph
                        .get_edgelist()
                        .get(&agent.cur_edge)
                        .expect("Current Edge Was not an Edge");

                    let next_node_id = if current_edge.end_id == agent.prev_node {
                        current_edge.start_id
                    } else {
                        current_edge.end_id
                    };

                    if graph.get_nodelist().get(&next_node_id).is_some() {
                        break;
                    }
                }

                agent.position = next_node.point;
            }
        }
    }
}

fn point_on_line(start: Vec2, end: Vec2, test: Vec2) -> bool {
    let d1 = (start - test).length();
    let d2 = (end - test).length();

    let line_len = (end - start).length();
    let buffer = 0.1;

    if d1 + d2 >= line_len - buffer && d1 + d2 <= line_len + buffer {
        true
    } else {
        false
    }
}
