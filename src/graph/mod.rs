use std::{sync::RwLock, collections::HashMap};

use eframe::{
    egui::{Ui, Response, Painter},
    epaint::{Shape, Stroke},
};

use serde::Deserialize;

use crate::Module;

pub use bounding::*;
pub use types::*;

pub mod bounding;
pub mod transform;
pub mod types;
pub mod route_finding;

/// Graph is the underlying data that the display and simulation use
/// It's loaded with data by the resource loader
///  - a primitive based list of Shapes is created from it on each display tick
///  - on each simulation tick the algorithms should be able to read from it
///
///  - module should be able to respond to controls from the gui to mutate itself
#[derive(Default, Debug)]
pub struct Graph {
    graph: AdjacencyList,
    transform: RwLock<transform::Transform>,
    config: GraphConfig,
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
        self.config = config;

        match self.transform.write() {
            Ok(mut transform) => {
                *transform = transform::Transform::new(&self.graph);
            }
            Err(err) => {
                panic!("Error Writing Transform {:?}", err);
            }
        };

        // TODO: Build view & cache etc for the GUI
        println!("[{}] Initialised in {:?}", self.get_name(), time.elapsed());

        Ok(())
    }
}

#[derive(Default, Deserialize, Debug)]
pub struct GraphConfig {
    node_colour: String,

    #[serde(default = "default_radius")]
    node_radius: f32,

    edge_colour: String,

    #[serde(default = "default_radius")]
    edge_thickness: f32,
}

fn default_radius() -> f32 {
    1.0
}

impl Graph {

    pub fn get_nodelist(&self) -> &HashMap<u128, NodeMeta> {
        &self.graph.node_map
    }

    pub fn get_edgelist(&self) -> &HashMap<u128, EdgeMeta> {
        &self.graph.edge_map
    }

    pub fn get_adjacency(&self) -> &HashMap<u128, Vec<u128>> {
        &self.graph.adjacency
    }

    pub fn get_transform(&self) -> &RwLock<transform::Transform> {
        &self.transform
    }

    pub fn view(&self, response: &mut Response, painter: &mut Painter, ui: &mut Ui) {
        let drag_delta = response.drag_delta();
        let scroll_delta = ui.input(|i| i.zoom_delta()); //* 50.0; //ui.input().scroll_delta.y;

        match self.transform.try_write() {
            Ok(mut transform) => {
                transform.set_map_offet(ui.max_rect().min);
                transform.drag(drag_delta);
                transform.zoom(scroll_delta);
                transform.scale(ui.available_width());
            }
            Err(err) => println!("{:?}", err),
        }
        
        painter.extend(self.create_paint_shapes())
    }

    pub fn create_paint_shapes(&self) -> Vec<Shape> {
        let mut shapes = Vec::with_capacity(self.graph.node_map.len() + self.graph.edge_map.len());

        for (_, node_meta) in self.graph.node_map.iter() {
            shapes.push(Shape::circle_filled(
                self.transform
                    .read()
                    .unwrap()
                    .map_to_screen(node_meta.point.0, node_meta.point.1),
                self.config.node_radius,
                str_as_colour(&self.config.node_colour),
            ))
        }

        for (_, edge_meta) in self.graph.edge_map.iter() {
            shapes.push(Shape::line(
                edge_meta
                    .points
                    .iter()
                    .map(|point| {
                        self.transform
                            .read()
                            .unwrap()
                            .map_to_screen(point.0, point.1)
                    })
                    .collect(),
                Stroke::new(self.config.edge_thickness, str_as_colour(&self.config.edge_colour)),
            ))
        }
        shapes
    }
}