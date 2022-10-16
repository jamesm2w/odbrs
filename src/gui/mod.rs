use std::sync::{Arc, RwLock};

use eframe::{NativeOptions, epaint::{vec2, Color32}, egui::CentralPanel};
use serde::Deserialize;

use crate::{Module, graph::{Graph, GraphConfig}};

/// Gui contains the GUI for the app obviously
/// - Function for view of the app
/// - Pan and Zoom capabilities
/// - Control over functions
/// - etc. display functionality
///
#[derive(Default, Clone)]
pub struct App {
    graph: Arc<RwLock<Graph>>,
    config: GuiConfig
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

        Ok(println!(
            "[{}] Initialised in {:?}",
            self.get_name(),
            time.elapsed()
        ))
    }
}

#[derive(Default, Clone, Deserialize)]
pub struct GuiConfig {
    test: String,
}

pub struct AppParameters {
    pub graph: Arc<RwLock<Graph>>
}

impl App {
    pub(crate) fn start(self) {
        let mut options = NativeOptions::default();
        options.initial_window_size = Some(vec2(800.0, 600.0));

        eframe::run_native("odbrs", options, Box::new(|_cc| {
            Box::new(self)
        }));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        
        CentralPanel::default().frame(eframe::egui::Frame::none().fill(Color32::LIGHT_GRAY)).show(ctx, |ui| {
            // ui.heading("Hello World");
            // self.graph.view(ui);
            
            match self.graph.write() {
                Ok(mut graph) => {
                    graph.view(ui);
                },
                Err(err) => panic!("{:?}", err)
            }
        });
    }
}