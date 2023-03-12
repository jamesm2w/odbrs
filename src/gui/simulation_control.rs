use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

use eframe::{egui::{Ui, Slider, Context, Window}, epaint::{vec2}};

use crate::simulation::{SimulationMessage, SimulationState};

use super::{AppState, Control, App};

pub struct SimulationControl {
    pub app_state: Rc<RefCell<AppState>>,
    pub sim_tx: Sender<SimulationMessage>,
    pub state: ControlState,
    pub speed: u64,
}

#[derive(PartialEq, Eq)]
pub enum ControlState {
    Running,
    Paused,
    Stopped,
}

impl Control for SimulationControl {
    fn view_control(&mut self, ui: &mut Ui) {
        ui.label(format!(
            "Tick #{}. State: {:?}",
            self.app_state.borrow().sim_state.0.format("%H:%M %d/%m/%Y"),
            self.app_state.borrow().sim_state.1
        ));

        ui.horizontal(|ui| {
            match self.state {
                ControlState::Stopped => {},
                ControlState::Paused => ui.columns(2, |ui| {
                    if ui[0].button("Start").clicked() {
                        self.state = ControlState::Running;
                        match self.sim_tx
                            .send(SimulationMessage::ChangeState(SimulationState::Running)) {
                                Ok(()) => (),
                                Err(err) => eprintln!("Send Error {:?}", err)
                            }
                    }
                    if ui[1].button("Stop").clicked() {
                        self.state = ControlState::Stopped;
                        match self.sim_tx
                            .send(SimulationMessage::ChangeState(SimulationState::Stopped)) {
                                Ok(()) => (),
                                Err(err) => eprintln!("Send Error {:?}", err)
                            };
                    }
                }),
                ControlState::Running => ui.columns(2, |ui| {
                    if ui[0].button("Pause").clicked() {
                        self.state = ControlState::Paused;
                        match self.sim_tx
                            .send(SimulationMessage::ChangeState(SimulationState::Paused)) {
                                Ok(()) => (),
                                Err(err) => eprintln!("Send Error {:?}", err),
                            }
                    }
                    if ui[1].button("Stop").clicked() {
                        self.state = ControlState::Stopped;
                        match self.sim_tx
                            .send(SimulationMessage::ChangeState(SimulationState::Stopped)) {
                                Ok(()) => (),
                                Err(err) => eprintln!("Send Error {:?}", err)
                            };
                    }
                })
            }
        });

        ui.separator();

        let slider = Slider::new(&mut self.speed, 1..=10000).text("Tick Speed");
        let resp = ui.add(slider);
        if resp.changed() {
            match self.sim_tx.send(SimulationMessage::ChangeSpeed(self.speed)) {
                Ok(_) => (),
                Err(err) => eprintln!("Send Error {:?}", err)
            }
        }
    }
}

pub fn render_control(app_state: &mut App, ctx: &Context, _frame: &mut eframe::Frame) {
    Window::new("Simulation Controls").default_size(vec2(300.0, 500.0)).show(ctx, |ui| {
        
        for (i, control) in app_state.controls.iter_mut().enumerate() {
            if i != 0 { ui.separator(); }
            control.view_control(ui);
        }
    });
}