use std::sync::RwLock;

use crate::Module;

pub mod bounding;
pub mod types;
pub mod transform;

pub use bounding::*;
use eframe::{
    egui::{Ui, Id, Sense, Key},
    epaint::{Color32, Shape, Stroke},
};
use serde::Deserialize;
pub use types::*;

/// Graph is the underlying data that the display and simulation use
/// It's loaded with data by the resource loader
///  - a primitive based list of Shapes is created from it on each display tick
///  - on each simulation tick the algorithms should be able to read from it
///
///  - module should be able to respond to controls from the gui to mutate itself
#[derive(Default)]
pub struct Graph {
    graph: AdjacencyList,
    transform: RwLock<transform::Transform>
}

impl Module for Graph {
    type Configuration = GraphConfig;
    type ReturnType = ();
    type Parameters = AdjacencyList;

    fn get_name(&self) -> &str {
        "Graph"
    }

    fn init(
        &mut self,
        config: Self::Configuration,
        parameters: Self::Parameters,
    ) -> Result<Self::ReturnType, Box<dyn std::error::Error>> {
        let time = std::time::Instant::now();

        self.graph = parameters;

        match self.transform.write() {
            Ok(mut transform) => {
                *transform = transform::Transform::new(&self.graph);
            },
            Err(err) => {
                panic!("Error Writing Transform {:?}", err);
            }
        };
    
        // TODO: Build view & cache etc for the GUI
        println!("[{}] Initialised in {:?}", self.get_name(), time.elapsed());

        Ok(())
    }
}

#[derive(Default, Deserialize)]
pub struct GraphConfig {
    colour: String,
}

impl Graph {
    pub fn view(&mut self, ui: &mut Ui) {

        let drag_delta = ui.interact(ui.clip_rect(), Id::null(), Sense::drag()).drag_delta();
        let scroll_delta = (ui.input().zoom_delta() - 1.0) * 50.0; //ui.input().scroll_delta.y;

        match self.transform.try_write() {
            Ok(mut transform) => {
                transform.drag(drag_delta);
                transform.zoom(scroll_delta);
                transform.scale(ui.available_width());
            },
            Err(err) => println!("{:?}", err)
        }

        ui.painter().extend(self.create_paint_shapes())
    }

    pub fn create_paint_shapes(&self) -> Vec<Shape> {
        let mut shapes = Vec::with_capacity(self.graph.node_map.len() + self.graph.edge_map.len());

        for (_, node_meta) in self.graph.node_map.iter() {
            shapes.push(Shape::circle_filled(
                self.transform.read().unwrap().map_to_screen(node_meta.point.0, node_meta.point.1),
                1.0,
                Color32::RED,
            ))
        }

        for (_, edge_meta) in self.graph.edge_map.iter() {
            shapes.push(Shape::line(
                edge_meta
                    .points
                    .iter()
                    .map(|point| self.transform.read().unwrap().map_to_screen(point.0, point.1))
                    .collect(),
                Stroke::new(1.5, Color32::BLACK),
            ))
        }
        shapes
    }
}
