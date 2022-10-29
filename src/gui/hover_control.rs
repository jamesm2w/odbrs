use std::sync::Arc;

use eframe::epaint::{Pos2, pos2};

use crate::graph::Graph;

use super::Control;


pub struct HoverControl {
    last_pos: Pos2,
    mapped_pos: (f64, f64),
    graph: Arc<Graph>
}

impl HoverControl {
    pub fn new(graph: Arc<Graph>) -> Self {
        HoverControl { last_pos: pos2(0.0, 0.0), mapped_pos: (0.0, 0.0), graph }
    }
}


impl Control for HoverControl {
    fn view_control(&mut self, ui: &mut eframe::egui::Ui) {

        self.last_pos = ui.input().pointer.hover_pos().unwrap_or(self.last_pos);

        self.mapped_pos = match self.graph.get_transform().read() {
            Ok(transform) => {
                transform.screen_to_map(self.last_pos)
            },
            Err(err) => panic!("Unable to read transform: {}", err)
        };

        ui.label(format!("Pos: {:?}", self.last_pos));
        ui.label(format!("Map: {:?}", self.mapped_pos));
    }
}
