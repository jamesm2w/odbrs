use std::collections::HashMap;

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