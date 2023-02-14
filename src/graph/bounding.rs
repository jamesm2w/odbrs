use super::AdjacencyList;

// Return an adjacency list with all the points within the given dimensions
pub fn bind_adjacencylist(
    list: AdjacencyList,
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
) -> AdjacencyList {

    let (node_map, edge_map) = (list.node_map, list.edge_map);

    let mut listprime = AdjacencyList {
        node_map: node_map.iter().filter(|(_, node)| point_within_bounds(node.point, left, right, top, bottom)).map(|(k, v)| (*k, v.to_owned())).collect(),
        edge_map: edge_map
            .into_iter()
            .filter(|(_, edge)| 
                edge.points.iter().any(|point| 
                    point_within_bounds(*point, left, right, top, bottom))
                ).collect(),
        adjacency: Default::default(),
    };

    for (id, edge) in listprime.edge_map.iter() {
        listprime.adjacency
            .entry(edge.start_id)
            .and_modify(|entry| entry.push(*id))
            .or_insert(vec![*id]);

        listprime.adjacency
            .entry(edge.end_id)
            .and_modify(|entry| entry.push(*id))
            .or_insert(vec![*id]);
        
        // Add the nodes to the node map if they are not already there (i.e. technically outside bounds, but an edge ends at one of them)
        if !listprime.node_map.contains_key(&edge.start_id) {
            listprime.node_map.insert(edge.start_id, node_map.get(&edge.start_id).expect("Edge Start should be in original map").clone());
        }

        if !listprime.node_map.contains_key(&edge.end_id) {
            listprime.node_map.insert(edge.end_id, node_map.get(&edge.end_id).expect("Edge End should be in original map").clone());
        }
    }

    listprime
}

pub fn point_within_bounds(point: (f64, f64), left: f64, right: f64, top: f64, bottom: f64) -> bool {
    left < point.0 && point.0 < right && bottom < point.1 && point.1 < top
}

// Return a minimal size "bounding box" around a graph so nothing is excluded
pub fn minimal_bounding(list: &AdjacencyList) -> (f64, f64, f64, f64) {
    let mut left = f64::MAX;
    let mut right = f64::MIN;

    let mut top = f64::MIN;
    let mut bottom = f64::MAX;

    for (_, edge) in list.edge_map.iter() {
        for pt in edge.points.iter() {
            left = left.min(pt.0);
            right = right.max(pt.0);

            top = top.max(pt.1);
            bottom = bottom.min(pt.1);
        }
    }

    for (_, node) in list.node_map.iter() {
        let pt = node.point;
        left = left.min(pt.0);
        right = right.max(pt.0);

        top = top.max(pt.1);
        bottom = bottom.min(pt.1);
    }

    (left, right, top, bottom)
}