//! Controller which handles the static case, i.e. traditional buses which get demand but do not respond to it.
//! 
//! 

use self::agent::StaticAgent;

use super::{Controller};

pub mod routes;
pub mod agent;

#[derive(Default)]
pub struct StaticController {
    buses: Vec<StaticAgent>,
}

impl Controller for StaticController {
    type Agent = StaticAgent;

    fn get_agents(&self) -> &Vec<Self::Agent> {
        &self.buses
    }

    fn spawn_agent(&mut self, graph: std::sync::Arc<crate::graph::Graph>) -> &Self::Agent {
        self.buses.push(StaticAgent::default()); // TODO: Actually spawn init agent properly
        self.buses.last().expect("Couldn't create new agent")
    }

    fn update_agents(
            &mut self,
            graph: std::sync::Arc<crate::graph::Graph>,
            demand: std::sync::Arc<super::demand::DemandGenerator>,
            time: chrono::DateTime<chrono::Utc>,
        ) {
        
    }
}