use std::{
    error::Error,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread, cell::RefCell,
};

use gui::onboarding::SettingOverrides;

use crate::analytics::AnalyticsPackage;

mod graph;
mod gui;
mod resource;
mod simulation;
mod analytics;

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
    pub analytics: analytics::Analytics,
    pub graph: Arc<graph::Graph>,
}

impl Module for Main {
    type ReturnType = ();
    type Configuration = PathBuf;
    type Parameters = SettingOverrides;

    fn get_name(&self) -> &str {
        "ODBRS -- Main"
    }

    fn init(
        &mut self,
        _config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<(), Box<dyn Error>> {
        let timer = std::time::Instant::now();
        println!("{} Starting Up", self.get_name());

        let (gui, sim, gph, adjlist, demand_resources) = self.resource_manager.init(_config, parameters)?;

        let mut graph = graph::Graph::default();
        graph.init(gph, adjlist)?;
        self.graph = Arc::new(graph);

        let analyticstx = self.analytics.init((), ())?;
        analyticstx.send(AnalyticsPackage::None).unwrap();

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
                analysis_tx: analyticstx,
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

    let settings_overrides = Arc::from(RefCell::new(Err(())));
    
    crate::gui::onboarding::Onboarding::run(settings_overrides.clone());
    
    let settings = match &*settings_overrides.borrow() {
        Ok(setting_overrides) => {
            setting_overrides.clone()
        },
        Err(_) => {
            return Ok(()); // Exit the programs
        }
    };

    let mut odbrs = Main::default();
    odbrs.init(PathBuf::from(r#"data/config.toml"#), settings)?;

    let handle = thread::spawn(move || {
        // Simulation start here in other thread
        println!("Simulation Thread Started");
        odbrs.simulation.start();
        println!("Simulation Thread Ended");
    });

    println!("GUI Thread Started");
    odbrs.gui.start()?;
    println!("GUI Thread Ended");

    handle.join().expect("Couldn't join the simulation thread");

    println!("Running analytics");
    odbrs.analytics.run();
    println!("Analytics finished"); 
    
    Ok(())
}
