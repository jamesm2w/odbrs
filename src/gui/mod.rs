use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    }
};

use chrono::{DateTime, Utc};
use eframe::{
    egui::{CentralPanel, SidePanel, Ui},
    epaint::{vec2, Color32, Shape, Stroke},
    NativeOptions,
};
use serde::Deserialize;

use crate::{
    graph::Graph,
    simulation::{self, SimulationMessage, SimulationState, demand::DemandGenerator},
    Module,
};

use self::{simulation_control::SimulationControl, hover_control::HoverControl};

mod simulation_control;
mod hover_control;

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
            speed: 100
        })];

        if self.config.hover_enabled {
            self.controls.push(Box::new(HoverControl::new(self.graph.clone())));
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
    hover_enabled: bool
}

pub struct AppParameters {
    pub graph: Arc<Graph>,
    pub rx: Receiver<AppMessage>,
    pub sim_tx: Sender<simulation::SimulationMessage>,
}

#[derive(Default, Debug)]
pub struct AppState {
    pub sim_state: (DateTime<Utc>, SimulationState),
    pub agent_pos: Vec<((f64, f64), u128, u128)>,
    pub demand_gen: Option<Arc<DemandGenerator>>
}

#[derive(Debug)]
pub enum AppMessage {
    // Placeholder(()),
    // SimulationState(DateTime<Utc>, SimulationState),
    SimulationStateWithAgents(DateTime<Utc>, SimulationState, Vec<((f64, f64), u128, u128)>),
    NoteDemandGen(Arc<DemandGenerator>)
}

impl App {
    pub(crate) fn start(self) {
        let mut options = NativeOptions::default();
        options.initial_window_size = Some(vec2(800.0, 600.0));

        eframe::run_native("odbrs", options, Box::new(|_cc| Box::new(self)));
    }

    fn handle_message(&mut self, msg: AppMessage) {
        // println!("[GUI] Thread handle message {:?}", msg);
        match msg {
            AppMessage::SimulationStateWithAgents(u, st, agents) => {
                let mut state = self.state.borrow_mut();
                state.sim_state = (u, st);
                state.agent_pos = agents;
                // println!("got agent pos {:?}", state.agent_pos[0]);
            },
            AppMessage::NoteDemandGen(demand_gen) => {
                let mut state = self.state.borrow_mut();
                state.demand_gen = Some(demand_gen);
            }
            // _ => (), // TODO: Uncomment this if other variants added
        }
    }
}

// fn load_image_from_path(path: &std::path::Path) -> Result<eframe::egui::ColorImage, image::ImageError> {
//     let image = image::io::Reader::open(path)?.decode()?;
//     let size = [image.width() as _, image.height() as _];
//     let image_buffer = image.to_rgba8();
//     let pixels = image_buffer.as_flat_samples();
//     Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(
//         size,
//         pixels.as_slice(),
//     ))
// }

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

        CentralPanel::default()
            .frame(eframe::egui::Frame::none().fill(Color32::DARK_GRAY))
            .show(ctx, |ui| {
                // ui.heading("Hello World");
                // self.graph.view(ui);

                // let texture: &eframe::egui::TextureHandle = self.overlay_img.get_or_insert_with(|| {
                //     ui.ctx().load_texture("image", load_image_from_path(&Path::new("./data/img/rgb.png")).unwrap(), eframe::egui::TextureFilter::Nearest)
                // });

                // ui.add(eframe::egui::Image::new(texture, ui.available_size()));
                self.graph.view(ui);

                // Draw the agent positions 
                // TODO: Refactor this to be nicer
                ui.painter().extend(
                    self.state
                        .borrow()
                        .agent_pos
                        .iter()
                        .map(|((x, y), edge, node)| {
                            let agent = Shape::circle_stroke(
                                self.graph.get_transform().read().unwrap().map_to_screen(*x, *y),
                                3.0,
                                Stroke::new(2.0, Color32::YELLOW),
                            );

                            let node_point = &self.graph.get_nodelist().get(node).unwrap().point;
                            let node = Shape::circle_stroke(self.graph.get_transform().read().unwrap().map_to_screen(node_point.0 as _, node_point.1 as _), 2.0, Stroke::new(1.0, Color32::LIGHT_GREEN));
                        
                            let edge_points = &self.graph.get_edgelist().get(edge).unwrap().points;
                            let line = Shape::line(edge_points.iter().map(|(i, j)| {
                                self.graph.get_transform().read().unwrap().map_to_screen(*i, *j)
                            }).collect(), Stroke::new(1.0, Color32::LIGHT_GREEN));

                            Shape::Vec(vec![agent, node, line])
                        })
                        .collect(),
                );

                if let Some(demand_gen) = &self.state.borrow().demand_gen {
                    ui.painter().extend( demand_gen.get_demand_queue().read().expect("GUI Couldn't read demand_gen").iter().map(|demand| {
                        
                        Shape::Vec(vec![
                            // Shape::line_segment([
                            //     self.graph.get_transform().read().unwrap().map_to_screen(demand.0.0 as _, demand.0.1 as _),
                            //     self.graph.get_transform().read().unwrap().map_to_screen(demand.1.0 as _, demand.1.1 as _)
                            // ], Stroke::new(1.5, Color32::LIGHT_GREEN)),
                            Shape::circle_stroke(self.graph.get_transform().read().unwrap().map_to_screen(demand.0.0 as _, demand.0.1 as _), 1.0, Stroke::new(1.5, Color32::LIGHT_GREEN)),
                            Shape::circle_stroke(self.graph.get_transform().read().unwrap().map_to_screen(demand.1.0 as _, demand.1.1 as _), 1.0, Stroke::new(1.5, Color32::LIGHT_RED)),
                            //TODO: tidy up this lol
                        ])
                        
                    }).collect())
                }
            });

        SidePanel::new(eframe::egui::panel::Side::Left, "control_panel")
            .default_width(300.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("On Demand Bus Routing System");

                for control in self.controls.iter_mut() {
                    control.view_control(ui);
                }
            });

        if self.state.borrow().sim_state.1 == SimulationState::Running {
            ctx.request_repaint();
        }
    }
}

pub trait Control {
    fn view_control(&mut self, ui: &mut Ui);
}
