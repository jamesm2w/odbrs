use std::sync::Arc;
// TODO: Fix the issue with movement in random controller
use eframe::epaint::Vec2;
use rand::Rng;

use crate::graph::Graph;

use super::Controller;

#[derive(Default, Debug)]
pub struct RandomController {
    pub agentc: usize,
}

#[derive(Debug)]
pub struct RandomAgent {
    pub id: u8,
    pub prev_node: u128,      // Node the agent came from
    pub cur_edge: u128,       // Edge it's currently traversing
    pub velocity: f64, // Velocity of the agent (+ive = from prev->other end, -ive = reverse) IN MS-1
    pub position: (f64, f64), // Current map-coord position of the agent
}

impl Controller for RandomController {
    type Agent = RandomAgent;

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> Self::Agent {
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
        };
        // //println!("Spawned agent {:?}", agent);
        agent
    }

    fn update_agents(
        &mut self,
        agents: &mut Vec<Self::Agent>,
        graph: std::sync::Arc<crate::graph::Graph>,
    ) {
        for agent in agents.iter_mut() {
            self.move_agent(agent, graph.clone());
        }
    }
}

impl RandomController {
    fn move_agent(&mut self, agent: &mut RandomAgent, graph: Arc<Graph>) {
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
            //println!("NEW MOVE current pos {:?} distance left {:?}", agent_pos, distance_to_move);

            for i in 0..line_points.len() - 1 {
                let start = line_points[i];
                let end = line_points[i + 1];

                //println!("NEW SEGMENT start {:?} end {:?}", start, end);

                // On this line segment
                if point_on_line(start, end, agent_pos) {
                    //println!("\tAgent is on this line segment");
                    let line_distance_remaining = end - agent_pos;
                    if distance_to_move > line_distance_remaining.length() {
                        // Move to end of line segment bit
                        //println!("\tAgent move to end of this line segment");
                        agent_pos = end;
                        distance_to_move -= line_distance_remaining.length(); // reduce distance needed by amount moved
                        //println!("\t\tnow pos={:?} move left={:?}", agent_pos, distance_to_move);
                    } else {
                        //println!("\tAgent can move along this line segment");
                        // Can move a the given distance along this segment
                        agent_pos += ((end - start) / (end - start).length()) * distance_to_move;
                        distance_to_move = 0.0; // no need to move any more distance
                        //println!("\t\tnow pos={:?} move left={:?}", agent_pos, distance_to_move);
                        // break;
                    }
                } else {
                    //println!("\tAgent is not on this segment");
                }
            }

            agent.position = (agent_pos.x as _, agent_pos.y as _);

            if distance_to_move > 0.0 {
                //println!("\tNeed to move to next graph edge pos {:?} end {:?}", agent.position, next_node.point );

                let dist_left = Vec2 { x: agent.position.0 as _, y: agent.position.1 as _ } - Vec2 { x: next_node.point.0 as _, y: next_node.point.1 as _ };
                //println!("\t remaining dist {:?}", dist_left.length());

                // Need to move to next node in graph
                agent.prev_node = next_node_id;
                let adjacency = graph.get_adjacency().get(&next_node_id).unwrap();
                // //println!("adjacent edges {:?}", adjacency);
                loop {
                    let next_edge_i = rand::thread_rng().gen_range(0..=adjacency.len() - 1);
                    // //println!("random index {:?} out of {:?}", next_edge_i, adjacency.len());
                    agent.cur_edge = adjacency.get(next_edge_i).unwrap().clone();
                    let current_edge = graph
                        .get_edgelist().get(&agent.cur_edge).expect("Current Edge Was not an Edge");

                    let next_node_id = if current_edge.end_id == agent.prev_node {
                        current_edge.start_id
                    } else {
                        current_edge.end_id
                    };

                    if graph.get_nodelist().get(&next_node_id).is_some() {
                        break;
                    }
                }
                
                // DEBUG:  This shouldn't be necessary really
                agent.position = next_node.point;
            }
            
            //println!("pos={:?} distance_left={:?}", agent.position, distance_to_move);
            //println!();
        }
    }
}

// TODO: Handle straight hoizontal and vertical lines maybe (doubt we'd see those though)
fn point_on_line(start: Vec2, end: Vec2, test: Vec2) -> bool {
    // //println!("\tpoint on line start {:?} end {:?} test {:?}", start, end, test);
    let xs = (test.x - start.x) / (end.x - start.x);
    let ys = (test.y - start.y) / (end.y - start.y);


    // let AB = test - start;
    // let AC = end - start;

    // let cross = AB.x * AC.y - (AB.y * AC.x)
    // cross == 0  

    // //println!("\txs: {:?} ys: {:?}", xs, ys);
        
    float_eq::float_eq!(xs, ys, abs <= 0.1) && 0.0 <= xs && xs <= 1.0 
}