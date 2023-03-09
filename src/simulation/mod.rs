use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use eframe::epaint::{pos2, Color32, Shape, Stroke};
use serde::Deserialize;

use crate::{graph::Graph, gui::AppMessage, resource::load_image::DemandResources, Module, analytics::{AnalyticsPackage, SimulationAnalyticsEvent}};

use self::{
    demand::DemandGenerator, dyn_controller::bus::{CurrentElement, send_analytics},
    static_controller::routes::NetworkData,
};

pub mod demand;
pub mod dyn_controller;
pub mod random_controller;
pub mod static_controller;

//const STATIC_ONLY: bool = true; // true = static only, false = dynamic only

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

    // Send Messages to the Analytics thread
    analytics_tx: Option<Sender<AnalyticsPackage>>,

    i: DateTime<Utc>,

    state: SimulationState,
    speed: u64, // Tick speed

    demand_generator: Option<Arc<DemandGenerator>>,

    dyn_controller: dyn_controller::DynamicController,
    static_controller: static_controller::StaticController,
    // agents: Vec<random_controller::RandomAgent>,

    static_only: bool,
    dynamic_agent_count: usize,
    demand_scale: f64
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

        self.static_only = config.static_only;
        self.dynamic_agent_count = config.dyn_agent_count;
        self.demand_scale = config.demand_scale;

        self.i = DateTime::from_utc(
            NaiveDateTime::new(Utc::now().date_naive(), NaiveTime::from_hms(5, 0, 0)),
            Utc,
        );
        self.rx = Some(parameters.rx);
        self.gui_tx = Some(parameters.gui_tx);

        self.analytics_tx = Some(parameters.analysis_tx);
        println!("[ANALYTICS] Received analytics {}", self.analytics_tx.is_some());

        self.graph = parameters.graph;
        self.speed = 100;

        if !self.static_only {
            self.dyn_controller.set_analytics(self.analytics_tx.clone());
            self.dyn_controller.set_demand_scale(self.demand_scale);

            for _ in 0..self.dynamic_agent_count {
                self.dyn_controller.spawn_agent(self.graph.clone());
            }
        } else {
            println!("Loading network data...");
            let timer = std::time::Instant::now();
            self.network_data =
                Arc::new(static_controller::routes::load_saved_network_data().unwrap());
            println!("Loaded network data in {:?}", timer.elapsed());
            self.static_controller
                .set_network_data(self.network_data.clone());
            self.static_controller.set_demand_scale(self.demand_scale);
            self.static_controller.set_analytics(self.analytics_tx.clone());
            self.static_controller.spawn_agent(self.graph.clone());
        }

        self.demand_generator = Some(DemandGenerator::start(
            parameters.demand_resources,
            self.graph.clone(),
            if !self.static_only {
                Ok(self.graph.clone())
            } else {
                Err(self.network_data.clone())
            }
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
    pub static_only: bool, // true = static only, false = dynamic only
    pub dyn_agent_count: usize,
    pub demand_scale: f64,
}

pub struct SimulationParameters {
    pub graph: Arc<Graph>,
    pub rx: Receiver<SimulationMessage>,
    pub gui_tx: Sender<AppMessage>,
    pub analysis_tx: Sender<AnalyticsPackage>,
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
                    let timer = std::time::Instant::now();
                    self.tick();
                    let time = timer.elapsed();
                    self.send_state();
                    
                    send_analytics(&self.analytics_tx, AnalyticsPackage::SimulationEvent( SimulationAnalyticsEvent::TickTime { tick: 0, time: time.as_secs_f64() } ));
                    if time > Duration::from_millis(self.speed) {
                        println!(
                            "[SIMULATION] Tick took longer than the speed! {:?} > {:?}",
                            time,
                            Duration::from_millis(self.speed)
                        );
                    } else {
                        thread::sleep(Duration::from_millis(self.speed));
                    }

                    if self.i.time() > NaiveTime::from_hms(23, 0, 0) {
                        println!("[SIMULATION] Stopping at 23:00:00");
                        self.state = SimulationState::Stopped;
                    }
                }
                SimulationState::Paused => {}
                SimulationState::Stopped => break,
            }

            // println!("Sending {:?}", AppMessage::SimulationState(self.i, self.state));
        }

        return;
    }

    pub fn send_state(&self) {
        match self
            .gui_tx
            .as_ref()
            .unwrap()
            .send(AppMessage::SimulationStateWithAgents(
                self.i.clone(),
                self.state.clone(),
                if !self.static_only {
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
        if !self.static_only {
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
    let _next_node = agent.get_next_node();
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
