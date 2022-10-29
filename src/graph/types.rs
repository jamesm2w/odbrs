use std::collections::HashMap;

use eframe::epaint::Color32;
use serde::{Serialize, Deserialize};

pub type NodeId = u128;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct NodeMeta {
    pub point: (f64, f64),
    pub id: NodeId,
    pub node_type: NodeType
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    RoadEnd, 
    Junction,
    Unknown(String)
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Unknown(String::from(""))
    }
}

pub type EdgeId = u128;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMeta {
    pub points: Vec<(f64, f64)>,
    pub start_id: NodeId,
    pub end_id: NodeId,
    pub id: EdgeId,
    pub edge_class: EdgeClass,
    pub length: f64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeClass {
    NotClassified,
    Unclassified,
    ClassifiedUnnumbered,
    RoadB,
    RoadA,
    Motorway,
    Unknown(String)
}

impl Default for EdgeClass {
    fn default() -> Self {
        EdgeClass::Unknown(String::from(""))
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AdjacencyList {
    pub node_map: HashMap<NodeId, NodeMeta>,
    pub edge_map: HashMap<EdgeId, EdgeMeta>,

    pub adjacency: HashMap<NodeId, Vec<EdgeId>>,
}

pub fn str_as_colour(c: &String) -> Color32 {
    match c.to_uppercase().as_str() {
        "TRANSPARENT" => Color32::TRANSPARENT,
        "BLACK" => Color32::BLACK,
        "DARK_GRAY" => Color32::DARK_GRAY,
        "GRAY" => Color32::GRAY,
        "LIGHT_GRAY" => Color32::LIGHT_GRAY,
        "WHITE" => Color32::WHITE,
        "BROWN" => Color32::BROWN,
        "DARK_RED" => Color32::DARK_RED,
        "RED" => Color32::RED,
        "LIGHT_RED" => Color32::LIGHT_RED,
        "YELLOW" => Color32::YELLOW,
        "LIGHT_YELLO" => Color32::LIGHT_YELLOW,
        "KHAKI" => Color32::KHAKI,
        "DARK_GREEN" => Color32::DARK_GREEN,
        "GREEN" => Color32::GREEN,
        "LIGHT_GREEN" => Color32::LIGHT_GREEN,
        "DARK_BLUE" => Color32::DARK_BLUE,
        "BLUE" => Color32::BLUE,
        "LIGHT_BLUE" => Color32::LIGHT_BLUE,
        "GOLD" => Color32::GOLD,
        _ => Color32::TEMPORARY_COLOR
    }
}