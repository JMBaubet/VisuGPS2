//! Lecture des fichiers `.osm.pbf` en Rust pur.
//!
//! Le prototype approuvé extrayait les ways tunnel avec `osmium`. Pour rendre
//! le paquet autoportant, la même sélection est faite ici avec
//! `osmpbfreader` :
//! - le filtre ne retient que les **ways** portant à la fois `tunnel` et
//!   `highway` — le matching ignore de toute façon les ways sans `highway` ;
//! - `get_objs_and_deps` charge uniquement les nœuds référencés par ces ways ;
//! - une way est conservée si au moins un de ses sommets tombe dans la bbox
//!   élargie de la trace, puis **entièrement** conservée (`complete_ways`).
//!
//! Les coordonnées sont stockées en `[lon, lat]`, convention GeoJSON reprise
//! par `geometry`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use osmpbfreader::{NodeId, OsmObj, OsmPbfReader};

use super::geometry;

/// Une way OSM retenue pour le matching.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmWay {
    /// Identifiant OSM préfixé : `w123456`.
    pub id: String,
    pub tags: BTreeMap<String, String>,
    /// Sommets ordonnés, `[lon, lat]`.
    pub coords: Vec<[f64; 2]>,
}

/// Lit les ways tunnel d'un PBF et ne garde que celles qui intersectent la
/// bbox `[minLon, minLat, maxLon, maxLat]`.
pub fn read_tunnel_ways(path: &Path, bbox: [f64; 4]) -> Result<Vec<OsmWay>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Ouverture du PBF ({}): {}", path.display(), e))?;
    let mut reader = OsmPbfReader::new(file);
    let objects = reader
        .get_objs_and_deps(|object| {
            object.is_way()
                && object.tags().get("tunnel").is_some()
                && object.tags().get("highway").is_some()
        })
        .map_err(|e| format!("Lecture du PBF ({}): {}", path.display(), e))?;

    let mut nodes: HashMap<NodeId, [f64; 2]> = HashMap::new();
    for object in objects.values() {
        if let OsmObj::Node(node) = object {
            nodes.insert(node.id, [node.lon(), node.lat()]);
        }
    }

    let mut ways = Vec::new();
    for object in objects.values() {
        let OsmObj::Way(way) = object else {
            continue;
        };
        let mut coords = Vec::with_capacity(way.nodes.len());
        let mut complete = true;
        for node_id in &way.nodes {
            match nodes.get(node_id) {
                Some(point) => coords.push(*point),
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if !complete || coords.len() < 2 {
            continue;
        }
        if !coords
            .iter()
            .any(|point| geometry::bbox_contains_point(bbox, *point))
        {
            continue;
        }

        let tags = way
            .tags
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();

        ways.push(OsmWay {
            id: format!("w{}", way.id.0),
            tags,
            coords,
        });
    }

    Ok(ways)
}

/// Fusionne les ways de plusieurs PBF et déduplique par identifiant OSM.
///
/// La déduplication est critique pour les traces frontalières : deux extraits
/// peuvent contenir la même way, et un tunnel ne doit pas être compté deux
/// fois.
pub fn merge_ways(ways: Vec<OsmWay>) -> Vec<OsmWay> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged = Vec::new();
    for way in ways {
        if seen.insert(way.id.clone()) {
            merged.push(way);
        }
    }
    merged
}
