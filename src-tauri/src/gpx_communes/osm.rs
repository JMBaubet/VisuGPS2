//! Lecture de la réponse Overpass : relations de limite administrative →
//! communes (nom + multi-polygone).
//!
//! La requête du front demande `out geom` : chaque relation arrive avec la
//! **géométrie complète** de ses ways membres, et non de simples identifiants.
//! Les anneaux sont donc assemblés ici, en chaînant les ways membres par leurs
//! extrémités — travail que fait `osmtogeojson` côté navigateur dans le
//! prototype.

use std::collections::HashMap;

use serde::Deserialize;

use super::geometry::{point_in_ring, same_point};

/// Élément OSM. Seuls les champs lus sont déclarés : serde ignore le reste de la
/// réponse (identifiants, tags inutiles, métadonnées Overpass).
#[derive(Deserialize, Debug)]
struct OsmElement {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    members: Vec<OsmMember>,
}

#[derive(Deserialize, Debug)]
struct OsmMember {
    /// Rôle du way dans la relation : `outer`, `inner`, ou vide — traité comme
    /// extérieur, comme `osmtogeojson`.
    #[serde(default)]
    role: String,
    /// Géométrie du way, inline grâce à `out geom`.
    #[serde(default)]
    geometry: Vec<OsmCoord>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct OsmCoord {
    lat: f64,
    lon: f64,
}

/// Un anneau extérieur et ses trous, en coordonnées `[lon, lat]`.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmPolygon {
    /// Anneau extérieur, fermé (premier sommet == dernier).
    pub outer: Vec<[f64; 2]>,
    /// Anneaux intérieurs (enclaves d'autres communes).
    pub inners: Vec<Vec<[f64; 2]>>,
}

/// Une commune : son nom et l'ensemble de ses polygones (une relation
/// multipolygone peut en compter plusieurs).
#[derive(Debug, Clone, PartialEq)]
pub struct OsmCommune {
    pub name: String,
    pub polygons: Vec<OsmPolygon>,
}

/// Diagnostics de lecture, remontés en log par la commande.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct OsmStats {
    /// Éléments OSM reçus.
    pub elements: usize,
    /// Relations retenues (limite administrative du niveau visé, nommée).
    pub relations: usize,
    /// Anneaux assemblés.
    pub rings: usize,
    /// Anneaux **non refermables** : refermés de force (liaison droite).
    pub forced_closures: usize,
    /// Anneaux écartés faute d'extérieur identifié (trou orphelin).
    pub orphan_rings: usize,
}

/// Convertit la réponse Overpass en communes exploitables.
///
/// `admin_level` est le niveau attendu (`"8"` pour les communes des pays
/// couverts). Sont écartées les relations d'un autre niveau, sans nom, ou sans
/// géométrie exploitable ; `stats` en rend compte.
pub fn parse_communes(
    json: &str,
    admin_level: &str,
) -> Result<(Vec<OsmCommune>, OsmStats), String> {
    let elements: Vec<OsmElement> = serde_json::from_str(json)
        .map_err(|e| format!("Réponse Overpass illisible : {}", e))?;

    let mut stats = OsmStats {
        elements: elements.len(),
        ..Default::default()
    };
    let mut communes = Vec::new();

    for element in &elements {
        if element.kind != "relation" {
            continue;
        }
        let tags = &element.tags;
        if tags.get("boundary").map(String::as_str) != Some("administrative") {
            continue;
        }
        if tags.get("admin_level").map(String::as_str) != Some(admin_level) {
            continue;
        }
        let Some(name) = tags.get("name").filter(|n| !n.trim().is_empty()) else {
            continue;
        };

        // Ways membres : extérieurs et trous, dans l'ordre de la relation.
        let mut outer_ways: Vec<Vec<[f64; 2]>> = Vec::new();
        let mut inner_ways: Vec<Vec<[f64; 2]>> = Vec::new();
        for member in &element.members {
            if member.geometry.len() < 2 {
                continue;
            }
            let way: Vec<[f64; 2]> = member.geometry.iter().map(|c| [c.lon, c.lat]).collect();
            if member.role == "inner" {
                inner_ways.push(way);
            } else {
                outer_ways.push(way);
            }
        }

        let (outer_rings, closures) = assemble_rings(outer_ways);
        stats.forced_closures += closures;
        if outer_rings.is_empty() {
            stats.orphan_rings += 1;
            continue;
        }

        let (inner_rings, inner_closures) = assemble_rings(inner_ways);
        stats.forced_closures += inner_closures;

        // Chaque trou est rattaché à l'anneau extérieur qui le contient.
        let mut polygons: Vec<OsmPolygon> = outer_rings
            .into_iter()
            .map(|outer| OsmPolygon {
                outer,
                inners: Vec::new(),
            })
            .collect();
        for hole in inner_rings {
            let host = hole
                .first()
                .and_then(|p| polygons.iter_mut().find(|poly| point_in_ring(&poly.outer, *p)));
            match host {
                Some(poly) => poly.inners.push(hole),
                // Trou sans extérieur : il n'appartient à aucune commune de la
                // réponse (relation incomplète), sa surface serait comptée à tort.
                None => stats.orphan_rings += 1,
            }
        }

        stats.relations += 1;
        stats.rings += polygons.len();
        communes.push(OsmCommune {
            name: name.clone(),
            polygons,
        });
    }

    Ok((communes, stats))
}

/// Chaîne des ways en anneaux fermés.
///
/// Les ways d'une relation sont donnés dans un ordre et une orientation
/// quelconques : la chaîne est donc prolongée par ses **deux** extrémités (et
/// chaque way peut être parcouru dans les deux sens), sans quoi un anneau dont
/// l'assemblage démarre au milieu resterait ouvert.
///
/// Retourne les anneaux assemblés et le nombre de chaînes **non refermables**
/// (données OSM incomplètes). Celles-ci sont refermées de force : la liaison
/// manquante d'une frontière est courte et rectiligne, et une frontière
/// approchée vaut mieux qu'une commune absente du résultat.
fn assemble_rings(mut ways: Vec<Vec<[f64; 2]>>) -> (Vec<Vec<[f64; 2]>>, usize) {
    let mut rings = Vec::new();
    let mut forced_closures = 0;

    while let Some(way) = ways.pop() {
        let mut ring = way;
        loop {
            let Some(head) = ring.first().copied() else { break };
            let Some(tail) = ring.last().copied() else { break };
            if ring.len() >= 4 && same_point(head, tail) {
                break;
            }

            // 1. Prolongement de la queue : un way qui en part…
            if let Some(i) = ways.iter().position(|w| same_point(w[0], tail)) {
                let mut next = ways.swap_remove(i);
                next.remove(0);
                ring.extend(next);
                continue;
            }
            // …ou qui y arrive (way parcouru en sens inverse).
            if let Some(i) = ways.iter().position(|w| same_point(w[w.len() - 1], tail)) {
                let mut next = ways.swap_remove(i);
                next.reverse();
                next.remove(0);
                ring.extend(next);
                continue;
            }
            // 2. Prolongement de la tête : un way qui y aboutit…
            if let Some(i) = ways.iter().position(|w| same_point(w[w.len() - 1], head)) {
                let mut previous = ways.swap_remove(i);
                previous.pop();
                previous.extend(ring);
                ring = previous;
                continue;
            }
            // …ou qui en part (way parcouru en sens inverse).
            if let Some(i) = ways.iter().position(|w| same_point(w[0], head)) {
                let mut previous = ways.swap_remove(i);
                previous.reverse();
                previous.pop();
                previous.extend(ring);
                ring = previous;
                continue;
            }
            break;
        }

        if ring.len() < 4 {
            continue;
        }
        if !same_point(ring[0], ring[ring.len() - 1]) {
            forced_closures += 1;
            let head = ring[0];
            ring.push(head);
        }
        rings.push(ring);
    }

    (rings, forced_closures)
}
