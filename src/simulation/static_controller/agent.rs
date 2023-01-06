use std::sync::Arc;

use crate::{simulation::{Agent, default_display}, graph::Graph};

#[derive(Default)]
pub struct StaticAgent {
    postion: (f64, f64),
    current_element: crate::simulation::dyn_controller::bus::CurrentElement,
    next_node: u128,
    graph: Arc<Graph>,
}

impl Agent for StaticAgent {
    fn display(&self) -> eframe::epaint::Shape {
        default_display(self)
    }

    fn get_current_element(&self) -> crate::simulation::dyn_controller::bus::CurrentElement {
        self.current_element
    }

    fn get_graph(&self) -> Arc<Graph> {
        self.graph.clone()
    }

    fn get_next_node(&self) -> u128 {
        self.next_node
    }

    fn get_position(&self) -> (f64, f64) {
        self.postion
    }
}

impl StaticAgent {

    // Move self for 1 tick
    pub fn move_self(&mut self) {

    }

}
