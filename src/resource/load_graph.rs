use std::{error::Error, fs, path::PathBuf};

use shapefile::{
    dbase::{FieldValue, Record},
    Shape,
};
use uuid::Uuid;

use crate::graph::{AdjacencyList, EdgeClass, EdgeMeta, NodeMeta, NodeType, self};

use super::GraphConfig;

// Given a graph config and a path to the shapefiles create an adjacency list (or dont)
pub(super) fn from_shapefiles(config: &GraphConfig, path: &PathBuf) -> Option<AdjacencyList> {
    let mut road_link = path.clone();
    road_link.push(format!(
        "{code}/{code}_RoadLink.shp",
        code = config.os_code.iter().collect::<String>()
    ));

    let mut road_node = path.clone();
    road_node.push(format!(
        "{code}/{code}_RoadNode.shp",
        code = config.os_code.iter().collect::<String>()
    ));

    // TODO: Add motorway support here
    // TODO: Fix error handling here with options and resultss

    let mut adjlist = AdjacencyList {
        ..Default::default()
    };

    let mut reader = shapefile::Reader::from_path(road_link).ok()?;
    for result in reader.iter_shapes_and_records() {
        let (shape, record) = result.ok()?;

        let edge_meta = parse_edge_record(shape, record)?;
        adjlist.edge_map.insert(edge_meta.id, edge_meta);
    }

    let mut reader = shapefile::Reader::from_path(road_node).ok()?;
    for result in reader.iter_shapes_and_records() {
        let (shp, record) = result.ok()?;

        let node_meta = parse_node_record(shp, record)?;
        adjlist.node_map.insert(node_meta.id, node_meta);
    }

    for (id, edge) in adjlist.edge_map.iter() {
        adjlist
            .adjacency
            .entry(edge.start_id)
            .and_modify(|entry| entry.push(*id))
            .or_insert(vec![*id]);

        adjlist
            .adjacency
            .entry(edge.end_id)
            .and_modify(|entry| entry.push(*id))
            .or_insert(vec![*id]);
    }

    Some(graph::bind_adjacencylist(adjlist, config.left, config.right, config.top, config.bottom))
}

// Given a path to a CBOR representation of an adjacency list, return it!
pub(super) fn from_file(path: &PathBuf) -> Result<AdjacencyList, Box<dyn Error>> {
    let time = std::time::Instant::now();
    let data = fs::read(path)?;
    let data = ciborium::de::from_reader::<AdjacencyList, _>(data.as_slice())?;

    println!("\tLoaded Graph from file {:?} in {:?}", path, time.elapsed());
    Ok(data)
}

// Copy the adjacency list to a file in CBOR represenation!
pub(super) fn copy_to_file(list: &AdjacencyList, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let timer = std::time::Instant::now();
    let mut bytes = vec![];

    ciborium::ser::into_writer(list, &mut bytes)?;
    fs::write(path, bytes)?;

    Ok(println!(
        "\tSaving Graph to file {:?} took {:?}",
        path,
        timer.elapsed()
    ))
}

// Parse a shape and record into a node object
fn parse_node_record(shp: Shape, record: Record) -> Option<NodeMeta> {
    let id = get_record_uuid("identifier", &record)?.as_u128();
    let node_type = match record.get("formOfNode")? {
        FieldValue::Character(Some(data)) => match data.as_str() {
            "road end" => NodeType::RoadEnd,
            "junction" => NodeType::Junction,
            _ => NodeType::Unknown(data.to_owned()),
        },
        value => panic!("Form of Node was not a string {:?} got {:?}", id, value),
    };

    let point = match shp {
        Shape::Point(pt) => (pt.x, pt.y),
        Shape::PointM(pt) => (pt.x, pt.y),
        Shape::PointZ(pt) => (pt.x, pt.y),
        _ => panic!(
            "Node {:?} was not a point, it was a {:?}",
            id,
            shp.shapetype()
        ),
    };

    Some(NodeMeta {
        point,
        id,
        node_type,
    })
}

// Parse an shape and edge record into an edge record
fn parse_edge_record(shp: Shape, record: Record) -> Option<EdgeMeta> {
    let start_id = get_record_uuid("startNode", &record)?.as_u128();
    let end_id = get_record_uuid("endNode", &record)?.as_u128();
    let id = get_record_uuid("identifier", &record)?.as_u128();

    let edge_class = match record.get("class")? {
        FieldValue::Character(Some(data)) => match data.as_str() {
            "Unclassified" => EdgeClass::Unclassified,
            "Classified Unnumbered" => EdgeClass::ClassifiedUnnumbered,
            "Unknown" => EdgeClass::Unknown(data.clone()),
            "B Road" => EdgeClass::RoadB,
            "Not Classified" => EdgeClass::NotClassified,
            "A Road" => EdgeClass::RoadA,
            "Motorway" => EdgeClass::Motorway,
            _ => EdgeClass::Unknown(data.to_owned()),
        },
        _ => EdgeClass::Unknown(String::from("Invalid Value Type")),
    };

    let length = match record.get("length")? {
        FieldValue::Numeric(Some(len)) => *len,
        val => panic!(
            "Length of edge {:?} was Not a Number. Instead {:?}",
            id, val
        ),
    };

    let points = match shp {
        Shape::PolylineZ(data) => data
            .parts()
            .concat()
            .into_iter()
            .map(|pt| (pt.x, pt.y))
            .collect(),
        _ => panic!(
            "Shape of edge {:?} was not a PolyLineZ instead got {:?}",
            id,
            shp.shapetype()
        ),
    };

    Some(EdgeMeta {
        points,
        start_id,
        end_id,
        id,
        edge_class,
        length,
    })
}

// Get a uuid from a shapefile record (if it's there)
fn get_record_uuid(field_name: &str, record: &Record) -> Option<Uuid> {
    match record.get(field_name)? {
        FieldValue::Character(Some(data)) => Uuid::parse_str(data.as_str()).ok(),
        _ => None,
    }
}
