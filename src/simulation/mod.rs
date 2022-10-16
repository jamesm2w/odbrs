use serde::Deserialize;

use crate::Module;

/// Simulation controls the running of the simulation
/// - Simluation tick does stuff at intervals
/// - List of agents which are active and do something each tick
/// - etc
///
/// - should be able to stop/start itself and other controls from the gui thread

#[derive(Default, Clone)]
pub struct Simulation {}

// The current state of the simulation
// Stopped - pre-start-up and post-stop
// Paused - mid execution and has agents on it just not calling the tick function
// Running - calling the tick function
#[derive(Debug, Clone)]
pub enum SimulationState {
    Stopped,
    Paused,
    Running,
}

impl Module for Simulation {
    type Configuration = SimulationConfig;
    type ReturnType = ();
    type Parameters = ();

    fn get_name(&self) -> &str {
        "Simulation"
    }

    fn init(
        &mut self,
        config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();

        Ok(println!(
            "[{}] Initialised in {:?}",
            self.get_name(),
            time.elapsed()
        ))
    }
}

#[derive(Default, Deserialize)]
pub struct SimulationConfig {
    test: String,
}
