//! Index Geofabrik et sélection des régions.
//!
//! `regions.json` est produit depuis `index-v1.json` et stocké **sur le support
//! OSM** (clé USB), jamais dans le dossier de la trace. Il ne contient que les
//! métadonnées utiles : identifiant, libellé, bbox aplatie, nom et URL du PBF.
//!
//! La sélection des régions est **exhaustive** : toute région dont la bbox
//! intersecte celle de la trace est requise, y compris les micro-états
//! entièrement contenus dans une région plus grande (Monaco dans PACA). On ne
//! suppose jamais qu'un PBF parent contient les données d'une sous-région.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::geometry;
use super::types::{
    RegionEntry, RegionsFile, TunnelRequirementRegion, TunnelRequirements,
    READINESS_DISABLED, READINESS_MISSING_PBF, READINESS_NEEDS_REGIONS,
    READINESS_NOT_CONFIGURED, READINESS_NO_REGION, READINESS_READY,
};

/// Nom du fichier d'index, sur le support OSM.
pub const REGIONS_FILE_NAME: &str = "regions.json";

/// URL officielle de l'index Geofabrik.
pub const INDEX_URL: &str = "https://download.geofabrik.de/index-v1.json";

/// Chemin de `regions.json` dans le dossier OSM.
pub fn regions_path(pbf_dir: &Path) -> PathBuf {
    pbf_dir.join(REGIONS_FILE_NAME)
}

/// Chemin d'un PBF dans le dossier OSM.
pub fn pbf_path(pbf_dir: &Path, filename: &str) -> PathBuf {
    pbf_dir.join(filename)
}

/// Lit `regions.json`. Retourne `None` si le fichier est absent, illisible ou
/// vide — l'appelant le régénère alors depuis Geofabrik.
pub fn load_regions(pbf_dir: &Path) -> Option<RegionsFile> {
    let content = std::fs::read_to_string(regions_path(pbf_dir)).ok()?;
    let file: RegionsFile = serde_json::from_str(&content).ok()?;
    if file.regions.is_empty() {
        return None;
    }
    Some(file)
}

/// Écrit `regions.json` de manière atomique.
pub fn save_regions(pbf_dir: &Path, regions: &RegionsFile) -> Result<(), String> {
    std::fs::create_dir_all(pbf_dir).map_err(|e| {
        format!(
            "Création du dossier OSM ({}): {}",
            pbf_dir.display(),
            e
        )
    })?;
    let json = serde_json::to_string_pretty(regions)
        .map_err(|e| format!("Sérialisation des régions OSM : {}", e))?;
    crate::gpx_audit::pipeline::write_atomic(
        &regions_path(pbf_dir),
        json.as_bytes(),
    )
}

/// Transforme `index-v1.json` en `regions.json` allégé.
///
/// Chaque géométrie (Polygon ou MultiPolygon) est aplatie en bbox
/// `[minLon, minLat, maxLon, maxLat]`. Les features sans géométrie, sans
/// identifiant ou sans URL de PBF sont ignorées.
///
/// Le code et le nom du pays sont résolus via la chaîne `parent` : les
/// sous-régions Geofabrik (`provence-alpes-cote-d-azur` → `france`) n'ont pas
/// de propriété ISO, contrairement aux entrées pays.
pub fn transform_index(content: &str, version: &str) -> Result<RegionsFile, String> {
    let index: Value = serde_json::from_str(content)
        .map_err(|e| format!("Index Geofabrik illisible : {}", e))?;
    let features = index
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| "Index Geofabrik invalide : tableau « features » absent.".to_string())?;

    // La carte des parents inclut toutes les features, même celles qui seront
    // écartées faute de géométrie ou d'URL : une région retenue peut avoir un
    // pays parent dont l'entrée est incomplète.
    let mut parents: HashMap<String, (String, String, Option<String>)> = HashMap::new();
    let mut raw_regions = Vec::new();

    for feature in features {
        let Some(properties) = feature.get("properties") else {
            continue;
        };
        let Some(id) = properties.get("id").and_then(Value::as_str) else {
            continue;
        };
        let parent = properties
            .get("parent")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = properties
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .to_string();
        let iso = iso_country_code(properties);
        parents.insert(id.to_string(), (parent.clone(), name.clone(), iso.clone()));

        let Some(geometry) = feature.get("geometry") else {
            continue;
        };
        if geometry.is_null() {
            continue;
        }
        let Some(coordinates) = geometry.get("coordinates") else {
            continue;
        };
        let points = flatten_coordinates(coordinates);
        if points.is_empty() {
            continue;
        }
        let Some(pbf_url) = properties
            .get("urls")
            .and_then(|urls| urls.get("pbf"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(pbf_filename) = pbf_url
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
        else {
            continue;
        };

        let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for (lon, lat) in &points {
            bbox[0] = bbox[0].min(*lon);
            bbox[1] = bbox[1].min(*lat);
            bbox[2] = bbox[2].max(*lon);
            bbox[3] = bbox[3].max(*lat);
        }

        raw_regions.push(RawRegion {
            id: id.to_string(),
            name,
            parent,
            iso,
            bbox,
            pbf_filename: pbf_filename.to_string(),
            pbf_url: pbf_url.to_string(),
        });
    }

    let regions = raw_regions
        .into_iter()
        .map(|raw| {
            let (country_code, country_name) =
                resolve_country(&parents, &raw.parent, raw.iso.as_deref(), &raw.name);
            RegionEntry {
                id: raw.id,
                name: raw.name,
                country: raw.parent.clone(),
                parent: raw.parent,
                country_code,
                country_name,
                bbox: raw.bbox,
                pbf_filename: raw.pbf_filename,
                pbf_url: raw.pbf_url,
                size_mb: None,
            }
        })
        .collect();

    Ok(RegionsFile {
        version: version.to_string(),
        regions,
    })
}

/// Entrée brute de l'index, avant résolution du pays.
struct RawRegion {
    id: String,
    name: String,
    parent: String,
    iso: Option<String>,
    bbox: [f64; 4],
    pbf_filename: String,
    pbf_url: String,
}

/// Résout le pays d'une région : sa propre propriété ISO, sinon la première
/// entrée de la chaîne `parent` qui en porte une.
fn resolve_country(
    parents: &HashMap<String, (String, String, Option<String>)>,
    parent: &str,
    own_iso: Option<&str>,
    own_name: &str,
) -> (Option<String>, Option<String>) {
    if let Some(code) = own_iso {
        return (Some(code.to_string()), Some(own_name.to_string()));
    }

    let mut current = parent.to_string();
    for _ in 0..16 {
        let Some((next_parent, name, iso)) = parents.get(&current) else {
            break;
        };
        if let Some(code) = iso {
            return (Some(code.clone()), Some(name.clone()));
        }
        if next_parent.is_empty() || next_parent == &current {
            break;
        }
        current = next_parent.clone();
    }
    (None, None)
}

/// Extrait `iso3166-1:alpha2` (tableau ou chaîne) et le normalise.
fn iso_country_code(properties: &Value) -> Option<String> {
    let value = properties.get("iso3166-1:alpha2")?;
    let code = match value {
        Value::Array(items) => items.first()?.as_str()?,
        Value::String(code) => code.as_str(),
        _ => return None,
    };
    let code = code.trim().to_ascii_uppercase();
    (code.len() == 2 && code.chars().all(|character| character.is_ascii_alphabetic()))
        .then_some(code)
}

/// Toutes les régions dont la bbox intersecte celle de la trace, y compris les
/// sous-régions entièrement contenues.
pub fn find_regions<'a>(regions: &'a RegionsFile, bbox: [f64; 4]) -> Vec<&'a RegionEntry> {
    regions
        .regions
        .iter()
        .filter(|region| geometry::bbox_intersects(bbox, region.bbox))
        .collect()
}

/**
 * Régions **les plus spécifiques** couvrant la trace.
 *
 * Pour chaque point de la trace, on retient la région dont la bbox est la plus
 * petite qui contient ce point. Une trace en PACA sélectionne PACA, pas `alps`
 * ni `france` ; une trace à Monaco sélectionne Monaco ; une trace
 * transfrontalière sélectionne les plus petites régions de chaque côté.
 *
 * Le pré-filtre par la bbox de la trace limite les candidates à quelques
 * entrées, si bien que le coût reste négligeable même sur une trace de 200 km :
 * O(points × candidates), avec un nombre de candidates de l'ordre de 2 à 10.
 */
pub fn select_regions<'a>(
    regions: &'a RegionsFile,
    points: &[[f64; 2]],
) -> Vec<&'a RegionEntry> {
    if points.is_empty() {
        return Vec::new();
    }

    let bbox = geometry::bbox(points);
    let mut candidates = find_regions(regions, bbox);
    if candidates.is_empty() {
        return candidates;
    }
    candidates.sort_by(|a, b| bbox_area(a.bbox).total_cmp(&bbox_area(b.bbox)));

    let mut selected_ids: HashSet<&str> = HashSet::new();
    let mut selected: Vec<&RegionEntry> = Vec::new();

    for point in points {
        for candidate in &candidates {
            if geometry::bbox_contains_point(candidate.bbox, *point) {
                if selected_ids.insert(candidate.id.as_str()) {
                    selected.push(candidate);
                }
                // La première région contenante est la plus petite : la liste
                // des candidates est triée par aire croissante.
                break;
            }
        }
    }

    selected.sort_by(|a, b| bbox_area(a.bbox).total_cmp(&bbox_area(b.bbox)));
    selected
}

fn bbox_area(bbox: [f64; 4]) -> f64 {
    (bbox[2] - bbox[0]).abs() * (bbox[3] - bbox[1]).abs()
}

/// Un PBF est disponible s'il existe et n'est pas vide.
pub fn pbf_present(pbf_dir: &Path, filename: &str) -> bool {
    std::fs::metadata(pbf_path(pbf_dir, filename))
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

/// Taille locale d'un PBF en Mo, arrondie au dixième.
pub fn pbf_size_mb(pbf_dir: &Path, filename: &str) -> Option<f64> {
    std::fs::metadata(pbf_path(pbf_dir, filename))
        .ok()
        .map(|metadata| ((metadata.len() as f64 / 1_048_576.0) * 10.0).round() / 10.0)
}

/// Compte les `.osm.pbf` présents dans le dossier OSM.
pub fn count_pbfs(pbf_dir: &Path) -> usize {
    std::fs::read_dir(pbf_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        == Some("pbf")
                })
                .count()
        })
        .unwrap_or(0)
}

/// Construit l'état des ressources OSM nécessaires à une trace.
pub fn build_requirements(
    active: bool,
    folder: Option<&Path>,
    regions: Option<&RegionsFile>,
    points: &[[f64; 2]],
) -> TunnelRequirements {
    let folder_string = folder.map(|path| path.to_string_lossy().to_string());

    if !active {
        return TunnelRequirements {
            readiness: READINESS_DISABLED.to_string(),
            folder: folder_string,
            regions: Vec::new(),
            missing_count: 0,
            message: Some("La détection des tunnels est désactivée.".to_string()),
        };
    }

    let Some(folder) = folder else {
        return TunnelRequirements {
            readiness: READINESS_NOT_CONFIGURED.to_string(),
            folder: None,
            regions: Vec::new(),
            missing_count: 0,
            message: Some("Aucun dossier OSM n'est configuré.".to_string()),
        };
    };
    if !folder.is_dir() || std::fs::read_dir(folder).is_err() {
        return TunnelRequirements {
            readiness: READINESS_NOT_CONFIGURED.to_string(),
            folder: folder_string,
            regions: Vec::new(),
            missing_count: 0,
            message: Some(format!(
                "Le dossier OSM est inaccessible : {}.",
                folder.display()
            )),
        };
    }

    let Some(regions) = regions else {
        return TunnelRequirements {
            readiness: READINESS_NEEDS_REGIONS.to_string(),
            folder: folder_string,
            regions: Vec::new(),
            missing_count: 0,
            message: Some(
                "La liste des régions OSM (regions.json) est absente.".to_string(),
            ),
        };
    };

    let required = select_regions(regions, points);
    if required.is_empty() {
        return TunnelRequirements {
            readiness: READINESS_NO_REGION.to_string(),
            folder: folder_string,
            regions: Vec::new(),
            missing_count: 0,
            message: Some(
                "Aucune région Geofabrik ne couvre la zone de cette trace.".to_string(),
            ),
        };
    }

    let mut entries = Vec::with_capacity(required.len());
    let mut missing_count = 0;
    for region in required {
        let present = pbf_present(folder, &region.pbf_filename);
        if !present {
            missing_count += 1;
        }
        entries.push(TunnelRequirementRegion {
            id: region.id.clone(),
            name: region.name.clone(),
            pbf_filename: region.pbf_filename.clone(),
            size_mb: if present {
                pbf_size_mb(folder, &region.pbf_filename).or(region.size_mb)
            } else {
                region.size_mb
            },
            present,
            too_large: false,
        });
    }

    let readiness = if missing_count > 0 {
        READINESS_MISSING_PBF
    } else {
        READINESS_READY
    };

    TunnelRequirements {
        readiness: readiness.to_string(),
        folder: folder_string,
        regions: entries,
        missing_count,
        message: (missing_count > 0).then(|| {
            format!(
                "{} fichier(s) PBF manquant(s) pour cette trace.",
                missing_count
            )
        }),
    }
}

fn flatten_coordinates(value: &Value) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    collect_coordinates(value, &mut points);
    points
}

fn collect_coordinates(value: &Value, points: &mut Vec<(f64, f64)>) {
    let Some(array) = value.as_array() else {
        return;
    };
    if array.len() == 2 {
        if let (Some(lon), Some(lat)) = (array[0].as_f64(), array[1].as_f64()) {
            points.push((lon, lat));
            return;
        }
    }
    for item in array {
        collect_coordinates(item, points);
    }
}
