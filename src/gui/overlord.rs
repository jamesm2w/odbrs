use eframe::{egui::{Window, Ui, Context}, Frame};

use super::{App, onboarding::Onboarding};

enum OverlordState {
    Onboarding, // The app is currently in the onboarding state
    Simulation, // The app is currently simulating something
    Statistics // The app has finished simulating and is showing the statistics
}

struct Overlord {
    pub map_window: App,
    pub onboarding_window: Onboarding,
}

impl Overlord {
    pub fn new() -> Self {
        unimplemented!()
    }
}

impl eframe::App for Overlord {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        let window = Window::new("new_window");
        window.show(ctx, |ui| {
            ui.label("Hello, world!");
        });
    }
}

pub trait WindowedApp {
    // use the UI base to create a window and show it in the context!
    fn update(&mut self, ctx: &Context, frame: &mut Frame);
}