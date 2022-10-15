use std::{error::Error, path::PathBuf};

mod graph;
mod gui;
mod resource;
mod simulation;

pub trait Module: Default {
    type ReturnType;
    type Configuration: Default;
    type Parameters;

    fn get_name(&self) -> &str;

    fn init(
        &mut self,
        config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn Error>>;
}

#[derive(Default)]
struct Main {
    pub resource_manager: resource::Resources,
    pub gui: gui::App,
    pub simulation: simulation::Simulation,
    pub graph: graph::Graph,
}

impl Module for Main {
    type ReturnType = ();
    type Configuration = PathBuf;
    type Parameters = ();

    fn get_name(&self) -> &str {
        "ODBRS -- Main"
    }

    fn init(
        &mut self,
        _config: Self::Configuration,
        _parameters: Self::Parameters,
    ) -> Result<(), Box<dyn Error>> {
        let timer = std::time::Instant::now();
        println!("{} Starting Up", self.get_name());

        let (gui, sim, graph, adjlist) = self.resource_manager.init(_config, ())?;
        self.graph.init(graph, adjlist)?;

        // These two should be running on separate threads
        self.simulation.init(sim, ())?;

        self.gui.init(gui, ())?;

        println!(
            "{} Finished Start up in {:?}",
            self.get_name(),
            timer.elapsed()
        );
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut odbrs = Main::default();
    odbrs.init(PathBuf::from(r#"data/config.toml"#), ())
}
