use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration, collections::VecDeque,
};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::{graph::Graph, gui::AppMessage, Module, resource::load_image::DemandResources};

use self::demand::DemandGenerator;

pub mod random_controller;
pub mod demand;

/// Simulation controls the running of the simulation
/// - Simluation tick does stuff at intervals
/// - List of agents which are active and do something each tick
/// - etc
///
/// - should be able to stop/start itself and other controls from the gui thread

#[derive(Default)]
pub struct Simulation {
    // Reference to the graph struct we're using
    graph: Arc<Graph>,

    // Recieve Mesages passed in by other threads
    rx: Option<Receiver<SimulationMessage>>,

    // Send Messages to the GUI thread
    gui_tx: Option<Sender<AppMessage>>,

    i: DateTime<Utc>,

    state: SimulationState,
    speed: u64, // Tick speed

    demand_generator: Option<Arc<DemandGenerator>>,

    controller: random_controller::RandomController,
    agents: Vec<random_controller::RandomAgent>,
}

// The current state of the simulation
// Stopped - pre-start-up and post-stop
// Paused - mid execution and has agents on it just not calling the tick function
// Running - calling the tick function
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum SimulationState {
    Stopped,
    Paused,
    Running,
}

impl Default for SimulationState {
    fn default() -> Self {
        SimulationState::Paused
    }
}

impl Module for Simulation {
    type Configuration = SimulationConfig;
    type ReturnType = ();
    type Parameters = SimulationParameters;

    fn get_name(&self) -> &str {
        "Simulation"
    }

    fn init(
        &mut self,
        config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();
        
        self.i = Utc::now(); // TODO: Move into config?
        self.rx = Some(parameters.rx);
        self.gui_tx = Some(parameters.gui_tx);

        self.graph = parameters.graph; 
        self.speed = 100; // TODO: Config this?

        for _ in 0..1 { // TODO: Change this number -- config maybe?
            self.agents.push(self.controller.spawn_agent(self.graph.clone()));
        }

        // TODO: Issue with this starting too soon?
        self.demand_generator = Some(DemandGenerator::start(parameters.demand_resources, self.graph.clone()));

        self.send_state();
        self.send_demand_gen();
        
        Ok(println!(
            "[{}] Initialised in {:?}",
            self.get_name(),
            time.elapsed()
        ))
    }
}

#[derive(Debug)]
pub enum SimulationMessage {
    ShutdownThread,
    ChangeState(SimulationState),
    ChangeSpeed(u64), // Change the simulation tick speed. ms value.
}

#[derive(Default, Deserialize)]
pub struct SimulationConfig {
    test: String,
}

pub struct SimulationParameters {
    pub graph: Arc<Graph>,
    pub rx: Receiver<SimulationMessage>,
    pub gui_tx: Sender<AppMessage>,
    pub demand_resources: DemandResources
}

impl Simulation {
    pub fn start(&mut self) {

        loop {
            match self.rx.as_ref().unwrap().try_recv() {
                Ok(msg) => self.handle_message(msg),
                Err(_) => (),
            };

            match self.state {
                SimulationState::Running => {
                    match self.demand_generator.as_ref().unwrap().get_demand_queue().write() {
                        Ok(mut queue) => {
                            queue.clear();
                        },
                        Err(err) => panic!("demand error {}", err)
                    }
                    
                    self.tick();
                    self.send_state();

                    thread::sleep(Duration::from_millis(self.speed));
                }
                SimulationState::Paused => {}
                SimulationState::Stopped => break,
            }

            // println!("Sending {:?}", AppMessage::SimulationState(self.i, self.state));
        }

        return;
    }

    // TODO: Basically just add some logic that this is caled when the state has actually changed (to stop flooding the GUI thread)
    pub fn send_state(&self) {
        match self.gui_tx
            .as_ref()
            .unwrap()
            .send(AppMessage::SimulationStateWithAgents(
                self.i.clone(),
                self.state.clone(),
                self.agents
                    .iter()
                    .map(|agent| (agent.position.clone(), agent.cur_edge, agent.prev_node))
                    .collect(),
            )) {
                Ok(_) => (),
                Err(err) => eprintln!("Send Error {:?}", err)
            }
    }

    pub fn send_demand_gen(&self) {
        match self.gui_tx.as_ref().unwrap().send(AppMessage::NoteDemandGen(self.demand_generator.as_ref().unwrap().clone())) {
            Ok(()) => {},
            Err(err) => eprintln!("Error Sending Demand gen instance: {}", err)
        }
    }

    pub fn handle_message(&mut self, msg: SimulationMessage) {
        println!("[SIM] Thread handle message {:?}", msg);
        match msg {
            SimulationMessage::ShutdownThread => {
                self.state = SimulationState::Stopped;
                self.demand_generator.as_ref().unwrap().shutdown();
            },
            SimulationMessage::ChangeState(state) => {
                self.state = state;
                self.send_state();
            },
            SimulationMessage::ChangeSpeed(speed) => self.speed = speed,
            // _ => (),
        }
    }

    pub fn tick(&mut self) {
        // Do a tick
        self.i = self.i + (chrono::Duration::minutes(1));

        // Despatch Demand Handler to get some more demand
        self.demand_generator.as_ref().unwrap().tick(self.i);

        // println!("Sim tick {:?}", self.i);
        // self.controller
        //     .update_agents(&mut self.agents, self.graph.clone())
    }
}

pub trait Controller {
    type Agent;

    fn spawn_agent(&mut self, graph: Arc<Graph>) -> Self::Agent;

    fn update_agents(&mut self, agents: &mut Vec<Self::Agent>, graph: Arc<Graph>);
}