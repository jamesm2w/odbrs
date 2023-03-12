use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
};

use chrono::{DateTime, Utc};
use eframe::{
    egui::{CentralPanel, Ui, TopBottomPanel, Frame},
    epaint::{vec2, Shape},
    NativeOptions,
};
use serde::Deserialize;

use crate::{
    graph::Graph,
    simulation::{self, demand::DemandGenerator, SimulationMessage, SimulationState},
    Module,
};

use self::{hover_control::HoverControl, simulation_control::{SimulationControl, render_control}, map::render_map};

mod hover_control;
mod simulation_control;
pub mod onboarding;
mod map;
pub mod analytics;

/// Gui contains the GUI for the app obviously
/// - Function for view of the app
/// - Pan and Zoom capabilities
/// - Control over functions
/// - etc. display functionality
///
#[derive(Default)]
pub struct App {
    graph: Arc<Graph>,

    // Immutable config of the application loaded at startup
    config: GuiConfig,

    // List of controls to put on the application
    controls: Vec<Box<dyn Control>>,

    // Mutable state of the application (for internal / control use)
    state: Rc<RefCell<AppState>>,

    // Recieve messages passed in by other threads
    rx: Option<Receiver<AppMessage>>,

    // Send messages to the simulation thread
    sim_tx: Option<Sender<SimulationMessage>>,
}

impl Module for App {
    type Configuration = GuiConfig;
    type ReturnType = ();
    type Parameters = AppParameters;
    fn get_name(&self) -> &str {
        "App"
    }

    fn init(
        &mut self,
        config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();

        self.config = config;
        self.graph = parameters.graph;
        self.rx = Some(parameters.rx);
        self.sim_tx = Some(parameters.sim_tx);

        self.controls = vec![Box::new(SimulationControl {
            app_state: self.state.clone(),
            sim_tx: self.sim_tx.clone().unwrap(),
            state: simulation_control::ControlState::Paused,
            speed: 100,
        })];

        if self.config.hover_enabled {
            self.controls
                .push(Box::new(HoverControl::new(self.graph.clone())));
        }

        Ok(println!(
            "[{}] Initialised in {:?}",
            self.get_name(),
            time.elapsed()
        ))
    }
}

#[derive(Default, Clone, Deserialize)]
pub struct GuiConfig {
    hover_enabled: bool,
}

pub struct AppParameters {
    pub graph: Arc<Graph>,
    pub rx: Receiver<AppMessage>,
    pub sim_tx: Sender<simulation::SimulationMessage>,
}

#[derive(Default, Debug)]
pub struct AppState {
    pub sim_state: (DateTime<Utc>, SimulationState),
    pub agent_display_data: Vec<Shape>,
    pub demand_gen: Option<Arc<DemandGenerator>>,
}

#[derive(Debug)]
pub enum AppMessage {
    // Placeholder(()),
    // SimulationState(DateTime<Utc>, SimulationState),
    SimulationStateWithAgents(DateTime<Utc>, SimulationState, Vec<Shape>),
    NoteDemandGen(Arc<DemandGenerator>),
}

impl App {
    pub(crate) fn start(self) -> Result<(), eframe::Error> {
        let mut options = NativeOptions::default();
        options.initial_window_size = Some(vec2(1920.0, 1080.0));
        eframe::run_native("odbrs", options, Box::new(|_cc| Box::new(self)))
    }

    fn handle_message(&mut self, msg: AppMessage) {
        // println!("[GUI] Thread handle message {:?}", msg);
        match msg {
            AppMessage::SimulationStateWithAgents(u, st, agents) => {
                let mut state = self.state.borrow_mut();
                state.sim_state = (u, st);
                state.agent_display_data = agents;
                // println!("got agent pos {:?}", state.agent_pos[0]);
            }
            AppMessage::NoteDemandGen(demand_gen) => {
                let mut state = self.state.borrow_mut();
                state.demand_gen = Some(demand_gen);
            } // _ => (), // TODO: Uncomment this if other variants added
        }
    }
}

impl eframe::App for App {
    fn on_close_event(&mut self) -> bool {
        match self
            .sim_tx
            .as_ref()
            .unwrap()
            .send(SimulationMessage::ShutdownThread)
        {
            Ok(()) => (),
            Err(err) => eprintln!("Couldn't send shutdown thread {:?}", err),
        };

        true
    }

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        match self.rx.as_ref().unwrap().try_recv() {
            Ok(msg) => self.handle_message(msg),
            Err(_) => (),
        };

        TopBottomPanel::top("top_menu").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label("On Demand Bus Routing Simulator");
            });
        });

        CentralPanel::default().frame(Frame::central_panel(&ctx.style())).show(ctx, |_| {

        });
        
        render_control(self, ctx, _frame);
        render_map(self, ctx, _frame);

        if self.state.borrow().sim_state.1 == SimulationState::Running {
            ctx.request_repaint();
        }
    }
}

pub trait Control {
    fn view_control(&mut self, ui: &mut Ui);
}
