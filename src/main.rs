use std::{
    error::Error,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
};

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
    pub graph: Arc<graph::Graph>,
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

        let (gui, sim, gph, adjlist, demand_resources) = self.resource_manager.init(_config, ())?;

        let mut graph = graph::Graph::default();
        graph.init(gph, adjlist)?;
        self.graph = Arc::new(graph);

        // Send stuff to the Simulation thread
        let (sim_tx, sim_rx) = mpsc::channel();

        // Send stuff to the GUI thread
        let (gui_tx, gui_rx) = mpsc::channel();

        // These two should be running on separate threads
        self.simulation.init(
            sim,
            simulation::SimulationParameters {
                graph: self.graph.clone(),
                rx: sim_rx,
                gui_tx: gui_tx.clone(),
                demand_resources,
            },
        )?;

        self.gui.init(
            gui,
            gui::AppParameters {
                graph: self.graph.clone(),
                rx: gui_rx,
                sim_tx: sim_tx.clone(),
            },
        )?;

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
    odbrs.init(PathBuf::from(r#"data/config.toml"#), ())?;

    thread::spawn(move || {
        // Simulation start here in other thread
        println!("Simulation Thread Started");
        odbrs.simulation.start();
        println!("Simulation Thread Ended");
    });

    println!("GUI Thread Started");
    // GUI start here in main thread
    odbrs.gui.start();
    println!("GUI Thread Ended");

    Ok(())
}
