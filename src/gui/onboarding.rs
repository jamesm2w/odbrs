use std::{sync::Arc, cell::RefCell};

use eframe::{egui::{CentralPanel}};

pub struct Onboarding {
    setting_ref: Arc<RefCell<Result<SettingOverrides, ()>>>,
    is_static: bool, 
    num_agents: usize,
    config_file_path: String,
    demand_scale: f64
}

impl Onboarding {
    fn new(setting_ref: Arc<RefCell<Result<SettingOverrides, ()>>>) -> Self {
        Self {
            setting_ref,
            is_static: false,
            num_agents: 50,
            demand_scale: 1.0,
            config_file_path: String::from("data/config.toml")
        }
    }
}

impl eframe::App for Onboarding {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        
        CentralPanel::default().show(ctx, |ui| {
            
            ui.vertical(|ui| {
                
                ui.heading("On Demand Bus Routing Simulation (ODBRS)");
                ui.label("Welcome to ODBRS! Please enter a few parameters before the simulation launchs");
                
                ui.radio_value(&mut self.is_static, false, "Dynamic Agents");
                ui.radio_value(&mut self.is_static, true, "Static Agents");
                
                ui.add(eframe::egui::Slider::new(&mut self.num_agents, 0..=100).text("Number of agents"));
            
                ui.horizontal_top(|ui| {
                    ui.label("Config file path: ");
                    ui.add(eframe::egui::TextEdit::singleline(&mut self.config_file_path).hint_text("Path to config file"));
                });

                ui.horizontal(|ui| {
                    if ui.add(eframe::egui::Button::new("Start Simulation")).clicked() {
                        *self.setting_ref.borrow_mut() = Ok(SettingOverrides {
                            is_static: self.is_static,
                            num_agents: self.num_agents,
                            demand_scale: self.demand_scale,
                            config_file_path: self.config_file_path.clone()
                        });
                        frame.close();
                    }

                    if ui.add(eframe::egui::Button::new("Cancel")).clicked() {
                        // send shutdown. but also make it an error
                        *self.setting_ref.borrow_mut() = Err(());
                        frame.close();
                    }
                });
            });    
        });
    }
}

impl Onboarding {
    pub fn run(settings_overrides: Arc<RefCell<Result<SettingOverrides, ()>>>) {
        let mut options = eframe::NativeOptions::default();
        options.initial_window_size = Some(eframe::egui::vec2(800.0, 600.0));

        // let settings_overrides = Arc::from(RefCell::new(Err(())));

        eframe::run_native("Onboarding", options, 
            Box::new(|_cc| Box::new(Onboarding::new(settings_overrides)))
        );

        // TODO: mutate some state somewhere to record result of application running
        // settings_overrides
    }
}

#[derive(Default, Clone)]
pub struct SettingOverrides {
    pub is_static: bool, // whether to use static (true) or dynamic agents (false)
    pub num_agents: usize, // number of dynamic agents to use
    pub demand_scale: f64, // scale factor for demand
    pub config_file_path: String, // path to the config file for the data
}