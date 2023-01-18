use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc, NaiveDateTime, NaiveTime};
use eframe::epaint::{pos2, Color32, Shape, Stroke};
use serde::Deserialize;

use crate::{graph::Graph, gui::AppMessage, resource::load_image::DemandResources, Module};

use self::{demand::DemandGenerator, dyn_controller::bus::CurrentElement, static_controller::routes::NetworkData};

pub mod demand;
pub mod dyn_controller;
pub mod random_controller;
pub mod static_controller;

const STATIC_ONLY: bool = false; // true = static only, false = dynamic only

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
    // Reference to the bus network data we're using
    network_data: Arc<NetworkData>,

    // Recieve Mesages passed in by other threads
    rx: Option<Receiver<SimulationMessage>>,

    // Send Messages to the GUI thread
    gui_tx: Option<Sender<AppMessage>>,

    i: DateTime<Utc>,

    state: SimulationState,
    speed: u64, // Tick speed

    demand_generator: Option<Arc<DemandGenerator>>,

    dyn_controller: dyn_controller::DynamicController,
    static_controller: static_controller::StaticController,
    // agents: Vec<random_controller::RandomAgent>,
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
        _config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();

        // self.i = Utc::now(); // TODO: Move into config?
        self.i = DateTime::from_utc(NaiveDateTime::new(Utc::now().date_naive(), NaiveTime::from_hms(6, 0, 0)), Utc);
        self.rx = Some(parameters.rx);
        self.gui_tx = Some(parameters.gui_tx);

        self.graph = parameters.graph;
        self.speed = 100; // TODO: Config this?

        if !STATIC_ONLY {
            for _ in 0..100 {
                // TODO: Change this number -- config maybe?
                self.dyn_controller.spawn_agent(self.graph.clone());
            }
        } else {

            println!("Loading network data...");
            let timer = std::time::Instant::now();
            self.network_data = Arc::new(static_controller::routes::load_saved_network_data().unwrap());
            println!("Loaded network data in {:?}", timer.elapsed());
            self.static_controller.set_network_data(self.network_data.clone());
            self.static_controller.spawn_agent(self.graph.clone());
        }

        // TODO: Issue with this starting too soon?
        self.demand_generator = Some(DemandGenerator::start(
            parameters.demand_resources,
            self.graph.clone(),
        ));

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
    pub demand_resources: DemandResources,
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
        match self
            .gui_tx
            .as_ref()
            .unwrap()
            .send(AppMessage::SimulationStateWithAgents(
                self.i.clone(),
                self.state.clone(),
                if !STATIC_ONLY {
                    self.dyn_controller
                        .get_agents()
                        .into_iter()
                        .map(|agent| agent.display()) // (agent.position.clone(), agent.cur_edge, agent.prev_node)
                        .collect()
                } else {
                    self.static_controller.get_display()
                },
            )) {
            Ok(_) => (),
            Err(err) => eprintln!("Send Error {:?}", err),
        }
    }

    pub fn send_demand_gen(&self) {
        match self
            .gui_tx
            .as_ref()
            .unwrap()
            .send(AppMessage::NoteDemandGen(
                self.demand_generator.as_ref().unwrap().clone(),
            )) {
            Ok(()) => {}
            Err(err) => eprintln!("Error Sending Demand gen instance: {}", err),
        }
    }

    pub fn handle_message(&mut self, msg: SimulationMessage) {
        println!("[SIM] Thread handle message {:?}", msg);
        match msg {
            SimulationMessage::ShutdownThread => {
                self.state = SimulationState::Stopped;
                self.demand_generator.as_ref().unwrap().shutdown();
            }
            SimulationMessage::ChangeState(state) => {
                self.state = state;
                self.send_state();
            }
            SimulationMessage::ChangeSpeed(speed) => self.speed = speed,
            // _ => (),
        }
    }

    pub fn tick(&mut self) {
        // Do a tick
        self.i = self.i + (chrono::Duration::minutes(1));

        // Despatch Demand Handler to get some more demand
        // self.demand_generator.as_ref().unwrap().tick(self.i);

        // println!("Sim tick {:?}", self.i);
        if !STATIC_ONLY {
            self.dyn_controller.update_agents(
                self.graph.clone(),
                self.demand_generator.as_ref().unwrap().clone(),
                self.i,
            )
        } else {
            self.static_controller.update_agents(
                self.graph.clone(),
                self.demand_generator.as_ref().unwrap().clone(),
                self.i,
            )
        }
    }
}

pub trait Controller {
    type Agent: Agent;

    fn spawn_agent(&mut self, graph: Arc<Graph>) -> Option<&Self::Agent>;

    fn get_agents(&self) -> Vec<&Self::Agent>;

    // fn agents_iter(&self) -> Self::AgentIterator;

    fn update_agents(
        &mut self,
        graph: Arc<Graph>,
        demand: Arc<DemandGenerator>,
        time: DateTime<Utc>,
    );
}

pub trait Agent {
    fn get_graph(&self) -> Arc<Graph>;
    fn get_position(&self) -> (f64, f64);
    fn get_current_element(&self) -> CurrentElement;
    fn get_next_node(&self) -> u128;

    // get a shape representing the agent -- default just based on current position and node/edge information
    fn display(&self) -> Shape {
        default_display(self)
    }
}

pub fn default_display<T: Agent + ?Sized>(agent: &T) -> Shape {
    let position = agent.get_position();
    let element = agent.get_current_element();
    let next_node = agent.get_next_node();
    let graph = agent.get_graph();

    match element {
        // Hasn't been placed on an element yet
        CurrentElement::PreGenerated => Shape::circle_stroke(
            pos2(position.0 as _, position.1 as _),
            3.0,
            Stroke::new(2.0, Color32::LIGHT_GREEN),
        ),
        // Currently Positioned on a node
        CurrentElement::Node(node) => {
            let node_data = graph.get_nodelist().get(&node).expect("Node not found");
            Shape::circle_stroke(
                pos2(node_data.point.0 as _, node_data.point.1 as _),
                3.0,
                Stroke::new(2.0, Color32::LIGHT_GREEN),
            )
        }
        // Currently Positioned some point on an edge
        CurrentElement::Edge { edge, prev_node } => {
            let edge_data = graph.get_edgelist().get(&edge).expect("Edge not found");
            let node_data = graph
                .get_nodelist()
                .get(&prev_node)
                .expect("Node not found");

            Shape::Vec(vec![
                Shape::circle_stroke(
                    pos2(position.0 as _, position.1 as _),
                    3.0,
                    Stroke::new(2.0, Color32::YELLOW),
                ),
                Shape::circle_stroke(
                    pos2(node_data.point.0 as _, node_data.point.1 as _),
                    2.0,
                    Stroke::new(1.0, Color32::LIGHT_GREEN),
                ),
                Shape::line(
                    edge_data
                        .points
                        .iter()
                        .map(|&(x, y)| pos2(x as _, y as _))
                        .collect(),
                    Stroke::new(1.0, Color32::LIGHT_GREEN),
                ),
            ])
        }
    }
}
