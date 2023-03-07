use std::{fs, path::PathBuf};

use crate::{
    graph::{self, AdjacencyList},
    gui::{self, onboarding::SettingOverrides},
    resource::load_image::load_images,
    simulation, Module,
};
use serde::Deserialize;

use self::load_image::{DemandResources, DemandResourcesConfig};

pub mod load_graph;
pub mod load_image;

/// Resources contains the methods for loading and converting data from disk
/// - Configuration
/// - Road Graph Data
/// - Bus Stop Information
///
/// Runs at startup and creates the relevant graph structure and config for the other modules
/// but doesn't do much more (maybe have some functionality to save stuff later)
#[derive(Default, Clone)]
pub struct Resources {}

impl Module for Resources {
    type Configuration = PathBuf;
    type ReturnType = (
        <gui::App as Module>::Configuration,
        <simulation::Simulation as Module>::Configuration,
        <graph::Graph as Module>::Configuration,
        AdjacencyList,
        DemandResources,
    );
    type Parameters = SettingOverrides;

    fn get_name(&self) -> &str {
        "Resources"
    }

    fn init(
        &mut self,
        _config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();
        
        
        let path = if parameters.config_file_path != "" {
            PathBuf::from(parameters.config_file_path)
        } else {
            _config
        };

        let data = fs::read(path)?;
        let config_file: ConfigFile = toml::from_slice(data.as_slice())?;

        let graph = match self.load_graph(&config_file) {
            Some(graph) => Ok(graph),
            None => Err("Error in loading graph"),
        }?;

        let mut sim_cfg = config_file.simulation;

        sim_cfg.static_only = parameters.is_static;
        sim_cfg.dyn_agent_count = parameters.num_agents;
        sim_cfg.demand_scale = parameters.demand_scale;

        let gui_cfg = config_file.app;
        let gph_cfg = config_file.graph;

        let demand_images = load_images(config_file.demand)?;

        println!("[{}] Initialised in {:?}", self.get_name(), time.elapsed());

        Ok((gui_cfg, sim_cfg, gph_cfg, graph, demand_images))
    }
}

#[derive(Default, Deserialize)]
struct ConfigFile {
    pub resources: ResourceConfig,
    pub app: <gui::App as Module>::Configuration,
    pub simulation: <simulation::Simulation as Module>::Configuration,
    pub graph: <graph::Graph as Module>::Configuration,
    pub defaults: Vec<GraphConfig>,
    pub demand: DemandResourcesConfig,
}

// Stores the config for this resource module
#[derive(Default, Deserialize)]
struct ResourceConfig {
    // WHERE ARE SHAPEFILE
    pub shapefile_src: String,

    #[serde(rename = "key")]
    pub graph_key: String,
}

// Stores a particular saved graph configuration
#[derive(Default, Deserialize)]
struct GraphConfig {
    // Name to identify this saved config as
    pub key: String,
    // Two letter OS code to identify the file
    pub os_code: [char; 2],
    // Left bound of the area
    pub left: f64,
    // Right bound of the area
    pub right: f64,
    // Top bound of the area
    pub top: f64,
    // Bottom bound of the area
    pub bottom: f64,
}

impl Resources {
    fn save_file_name(config: &GraphConfig) -> String {
        format!(
            "{}-{}.bin",
            config.key,
            config.os_code.iter().collect::<String>()
        )
    }

    // Load data from source files or whatever into a list of adjacencies
    fn load_graph(&self, config: &ConfigFile) -> Option<AdjacencyList> {
        let key = &config.resources.graph_key;
        let configuration = config.defaults.iter().find(|config| &config.key == key)?;

        let mut save_file_path = PathBuf::from("data/save/");
        save_file_path.push(Self::save_file_name(configuration));

        // Test for pre-comp source file
        if save_file_path.exists() {
            // Load the file into a list of adjacencies
            let adjlist = load_graph::from_file(&save_file_path);
            match adjlist {
                Ok(data) => Some(data),
                Err(err) => {
                    panic!("Error loading from file {:?}", err)
                }
            }
        } else {
            // Else fetch OS file and convert to adj lists
            let adjlist = load_graph::from_shapefiles(
                configuration,
                &PathBuf::from(&config.resources.shapefile_src),
            )?;

            load_graph::copy_to_file(&adjlist, &save_file_path)
                .expect("Error saving adj list out to file");

            Some(adjlist)
        }
    }
}
