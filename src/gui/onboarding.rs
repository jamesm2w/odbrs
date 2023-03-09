use std::{sync::Arc, cell::RefCell};

use eframe::{egui::{CentralPanel, Frame, style::Margin}, epaint::Color32};

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
        
        CentralPanel::default().frame(Frame::none().inner_margin(Margin::symmetric(20.0, 20.0)).fill(Color32::from_rgb(20, 20, 20))).show(ctx, |ui| {
            
            ui.vertical_centered(|ui| {
                
                ui.heading("On Demand Bus Routing Simulation (ODBRS)");
                ui.label("Welcome to ODBRS! Please enter a few parameters before the simulation launchs");
                
                ui.separator();
                ui.columns(3, |cols| {
                    cols[0].label("Simulation Type: ");
                    cols[1].radio_value(&mut self.is_static, false, "Dynamic Agents");
                    cols[2].radio_value(&mut self.is_static, true, "Static Agents");    
                });

                ui.separator();
                ui.columns(2, |cols| {
                    cols[0].label("Number of agents: ");
                    cols[1].add(eframe::egui::DragValue::new(&mut self.num_agents).speed(1).clamp_range(0..=500));
                });
                
                ui.separator();
                ui.columns(2, |cols| {
                    cols[0].label("Demand scale: ");
                    cols[1].add(eframe::egui::DragValue::new(&mut self.demand_scale).speed(0.01).clamp_range(0..=1));    
                });

                ui.separator();
                ui.columns(2, |cols| {
                    cols[0].label("Config file path: ");
                    cols[1].add(eframe::egui::TextEdit::singleline(&mut self.config_file_path).hint_text("Path to config file"));
                });

                ui.separator();
                ui.columns(4, |cols| {
                    if cols[3].add(eframe::egui::Button::new("Start Sim")).clicked() {
                        *self.setting_ref.borrow_mut() = Ok(SettingOverrides {
                            is_static: self.is_static,
                            num_agents: self.num_agents,
                            demand_scale: self.demand_scale,
                            config_file_path: self.config_file_path.clone()
                        });
                        frame.close();
                    }

                    if cols[2].add(eframe::egui::Button::new("Cancel")).clicked() {
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
        options.initial_window_size = Some(eframe::egui::vec2(450.0, 300.0));
        options.centered = true;
        options.resizable = false;

        // let settings_overrides = Arc::from(RefCell::new(Err(())));

        eframe::run_native("ODBRS Onboarding", options, 
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