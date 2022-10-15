use crate::Module;

pub mod types;
pub mod bounding;

pub use bounding::*;
use serde::Deserialize;
pub use types::*;

/// Graph is the underlying data that the display and simulation use
/// It's loaded with data by the resource loader
///  - a primitive based list of Shapes is created from it on each display tick
///  - on each simulation tick the algorithms should be able to read from it
/// 
///  - module should be able to respond to controls from the gui to mutate itself 
#[derive(Default, Clone)]
pub struct Graph {
    graph: AdjacencyList
}

impl Module for Graph {
    type Configuration = GraphConfig;
    type ReturnType = ();
    type Parameters = AdjacencyList;

    fn get_name(&self) -> &str {
        "Graph"
    }

    fn init(&mut self, config: Self::Configuration, parameters: Self::Parameters) -> Result<(), Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();

        self.graph = parameters;
        
        // TODO: Build view & cache etc for the GUI
        
        Ok(println!("[{}] Initialised in {:?}", self.get_name(), time.elapsed()))
    }
}

#[derive(Default, Deserialize)]
pub struct GraphConfig {
    colour: String
}