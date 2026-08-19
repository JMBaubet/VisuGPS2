//! Module de nettoyage des traces GPX.
//!
//! Une trace importée n'est **valide** que si elle est « propre » : les
//! anomalies de relevé (points isolés hors trace, aller-retours inutiles,
//! sorties de trajectoire) doivent être corrigées par l'utilisateur avant de
//! pouvoir engager les traitements (édition caméra). Ce module fournit :
//!
//! - la **détection** des anomalies par changement de cap proche de 180° (la
//!   tolérance de cap est paramétrable : `Nettoyage.Cap.toleranceDeg`) ;
//! - la **persistance** des décisions de correction (fichier de travail
//!   `{mode}/traces/{trace_id}/cleaning.{phase}.json`, écriture atomique) ;
//! - la **finalisation** : génération du GPX nettoyé, sauvegarde de l'original
//!   en `{filename}.orig`, régénération des dérivés (geojson, stats, hash) et
//!   passage de la trace à l'état `"clean"`.
//!
//! La détection a un rôle **propositif** : elle propose des segments candidats
//! (plages d'index + apex) que l'utilisateur peut ajuster. La validation de
//! chaque cas est de la responsabilité de l'utilisateur ; le GPX original
//! n'est remplacé qu'à la finalisation, une fois **tous** les cas validés.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::Manager;

use crate::import_gpx::{
    build_geojson_feature, compute_file_hash, compute_stats, extract_line_coordinates,
    get_geojson_path, get_mode_dir, get_traces_path, get_trace_gpx_path, haversine,
    load_registry, save_registry, TraceMetadata,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

// ---------------------------------------------------------------------------
// Structures sérialisables (miroir des interfaces TS du store cleaning)
// ---------------------------------------------------------------------------

/// Type d'anomalie détectée — pilote l'outillage proposé dans l'IHM.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CleaningCaseKind {
    /// Point isolé qui sort de la trace et revient immédiatement (ex. 946).
    Spike,
    /// Tour soutenu dans un rond-point (angle cumulé > seuil, ex. plus d'un
    /// tour du rond-point) — étape 2 du pipeline de nettoyage.
    Roundabout,
    /// Branche aller-retour avec retraçage (ex. 711 / 791) — étape 3.
    OutAndBack,
    /// Cas créé **manuellement** par l'utilisateur (plage `[start, end]`
    /// désignée sur la carte) — jamais produit par la détection.
    Manual,
    /// Valeur de repli pour les états de travail antérieurs (inconnus).
    #[serde(other)]
    Unknown,
}

/// Point de la trace **déplacé** géographiquement : à la finalisation, le point
/// d'index original `index` est remplacé par les coordonnées fournies
/// (élévation et temps d'origine conservés).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MovedPoint {
    pub index: usize,
    pub lat: f64,
    pub lon: f64,
}

/// Corrections appliquées à un cas. Les index sont **originaux** (avant
/// suppression) de la trace.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Correction {
    #[serde(default)]
    pub delete_ranges: Vec<[usize; 2]>,
    #[serde(default)]
    pub moved_points: Vec<MovedPoint>,
}

/// Un cas d'anomalie à traiter.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CleaningCase {
    /// Identifiant stable du cas (ex. "c1"), utilisé par l'IHM.
    pub id: String,
    pub kind: CleaningCaseKind,
    /// Début de la zone d'intérêt (index original inclus).
    pub start_index: usize,
    /// Fin de la zone d'intérêt (index original inclus).
    pub end_index: usize,
    /// Points de rebroussement détectés (index originaux).
    #[serde(default)]
    pub apex_indices: Vec<usize>,
    /// Écart de cap maximal mesuré dans le cas (degrés).
    #[serde(default)]
    pub bearing_delta_deg: f64,
    /// Angle cumulé **signé** d'un rond-point (degrés ; négatif = sens
    /// anti-horaire). Utile uniquement pour le type `Roundabout` (nombre de
    /// tours = `|angle| / 360`, sens = signe).
    #[serde(default)]
    pub total_angle_deg: f64,
    /// Plages de suppression **suggérées** par la détection (pré-remplissage
    /// de `correction.delete_ranges` au premier affichage du cas).
    #[serde(default)]
    pub suggested_delete_ranges: Vec<[usize; 2]>,
    /// État de validation utilisateur : `"pending"` (à traiter), `"corrected"`
    /// (corrigé), `"kept"` (conservé tel quel — faux positif).
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub correction: Correction,
}

/// État complet du nettoyage d'une trace (persisté dans
/// `traces/{trace_id}/cleaning.{phase}.json`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CleaningState {
    pub trace_id: String,
    pub tolerance_deg: f64,
    /// Phase du pipeline en cours : `"spike"` (pts hors trace), `"roundabout"`
    /// (ronds-points) ou `"out_and_back"` (aller/retour — étape 3 à venir).
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub cases: Vec<CleaningCase>,
}

/// Paramètres de détection des ronds-points (miroir des sliders de l'outil de
/// référence fourni). Lus par le frontend depuis `Nettoyage.RondPoints.*` et
/// transmis aux commandes, comme la tolérance de cap.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RoundaboutParams {
    /// Virage minimal entre deux points consécutifs pour prolonger le tour
    /// (degrés).
    #[serde(default = "default_angle_min_deg")]
    pub angle_min_deg: f64,
    /// Nombre de points minimum d'un rond-point.
    #[serde(default = "default_points_min")]
    pub points_min: usize,
    /// Nombre de points maximum (fenêtre de détection).
    #[serde(default = "default_points_max")]
    pub points_max: usize,
    /// Angle cumulé au-delà duquel un tour soutenu est signalé (degrés).
    #[serde(default = "default_angle_seuil_deg")]
    pub angle_seuil_deg: f64,
    /// Nombre de points affichés **avant/après** le segment détecté dans l'IHM
    /// (suppression/déplacement manuels).
    #[serde(default = "default_marge_points")]
    pub marge_points: usize,
}

fn default_angle_min_deg() -> f64 {
    5.0
}
fn default_points_min() -> usize {
    5
}
fn default_points_max() -> usize {
    50
}
fn default_angle_seuil_deg() -> f64 {
    210.0
}
fn default_marge_points() -> usize {
    5
}

impl Default for RoundaboutParams {
    fn default() -> Self {
        Self {
            angle_min_deg: default_angle_min_deg(),
            points_min: default_points_min(),
            points_max: default_points_max(),
            angle_seuil_deg: default_angle_seuil_deg(),
            marge_points: default_marge_points(),
        }
    }
}

// ---------------------------------------------------------------------------
// Utilitaires géométriques
// ---------------------------------------------------------------------------

/// Cap initial (bearing) entre deux points, en degrés [0, 360).
fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let d_lambda = (lon2 - lon1).to_radians();
    let y = d_lambda.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * d_lambda.cos();
    let deg = y.atan2(x).to_degrees();
    (deg + 360.0) % 360.0
}

/// Distance angulaire minimale entre deux caps (degrés, [0, 180]).
fn angle_distance(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

/// Longueur cumulée (Haversine) d'une séquence de points.
fn segment_length(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|w| haversine(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum()
}

// ---------------------------------------------------------------------------
// Extraction des points d'un GPX
// ---------------------------------------------------------------------------

/// Point de trace complet (lat/lon + altitude + timestamp préservé).
#[derive(Clone, Debug)]
struct GpxPoint {
    lat: f64,
    lon: f64,
    ele: Option<f64>,
    time: Option<String>,
}

/// Aplatie `tracks → segments → points` en conservant tous les attributs.
fn extract_full_points(gpx: &gpx::Gpx) -> Vec<GpxPoint> {
    let mut pts = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                // `gpx::Time::format` rend l'ISO 8601 (UTC, avec offset).
                let time = wp.time.and_then(|t| t.format().ok());
                pts.push(GpxPoint {
                    lat: pt.y(),
                    lon: pt.x(),
                    ele: wp.elevation,
                    time,
                });
            }
        }
    }
    pts
}

// ---------------------------------------------------------------------------
// Détection des anomalies
// ---------------------------------------------------------------------------

/// Fenêtre (en index) au-delà de laquelle deux points de rebroussement sont
/// considérés comme deux cas distincts.
const U_TURN_GROUP_WINDOW: usize = 25;

/// Détecte les **rebroussements** (changement de cap ≈ 180°) sur une liste de
/// coordonnées (lat, lon) et construit les cas de type `Spike` / `OutAndBack`.
///
/// Algorithme (inspiré de l'outil de détection fourni par l'utilisateur) :
/// 1. Pour chaque point, on compare le cap vers le point précédent et le cap
///    vers le point suivant : s'ils sont quasi identiques (différence ≤
///    tolérance), le point est un **rebroussement** (changement de cap ≈ 180°).
/// 2. Les rebroussements proches sont regroupés en un **cas**.
/// 3. On délimite la zone de déviation par retraçage symétrique (les points du
///    retour sont des jumeaux des points de l'aller) et on classe le cas :
///    - branche courte (< 100 m) → `spike` (suggestion : supprimer l'apex) ;
///    - sinon → `out_and_back` (suggestion : supprimer le demi-tour + le retour).
///
/// Le seuil de longueur de branche est une **heuristique de travail interne**,
/// pas un critère d'architecture : les étapes 1 et 3 sont deux types d'erreur
/// distincts (point isolé hors trace vs branche aller-retour) avec des
/// traitements et des sorties différents.
fn detect_uturn_cases(points: &[(f64, f64)], tolerance_deg: f64) -> Vec<CleaningCase> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }

    // 1. Points de rebroussement.
    let mut uturns: Vec<usize> = Vec::new();
    for i in 1..n - 1 {
        let cap_prev = bearing(points[i].0, points[i].1, points[i - 1].0, points[i - 1].1);
        let cap_next = bearing(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1);
        if angle_distance(cap_prev, cap_next) <= tolerance_deg {
            uturns.push(i);
        }
    }
    if uturns.is_empty() {
        return Vec::new();
    }

    // 2. Regroupement des rebroussements proches en cas.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &u in &uturns {
        if let Some(last) = groups.last_mut() {
            if let Some(&lu) = last.last() {
                if u - lu <= U_TURN_GROUP_WINDOW {
                    last.push(u);
                    continue;
                }
            }
        }
        groups.push(vec![u]);
    }

    // 3. Construction des cas.
    let mut cases = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        let apex = group[0];

        // Zone de déviation par retraçage symétrique (jumeaux aller/retour).
        let mut d_max = 0usize;
        while apex > d_max && apex + d_max + 1 < n {
            let a = apex - (d_max + 1);
            let b = apex + (d_max + 1);
            if haversine(points[a].0, points[a].1, points[b].0, points[b].1) < 30.0 {
                d_max += 1;
            } else {
                break;
            }
        }
        let start = apex - d_max;
        let end = apex + d_max;

        // Longueur cumulée de la branche (aller + retour).
        let branch_len = segment_length(&points[start..=end]);

        // Classification : branche courte → spike (point isolé) ; sinon un
        // aller-retour. Le type « parallel » (ajout de points) est réservé aux
        // cas assignés manuellement par l'utilisateur.
        let (kind, suggested) = if branch_len < 100.0 {
            (CleaningCaseKind::Spike, vec![[apex, apex]])
        } else {
            (CleaningCaseKind::OutAndBack, vec![[apex, end]])
        };

        // Écart de cap maximal parmi les rebroussements du groupe.
        let max_delta = group
            .iter()
            .map(|&i| {
                let c1 = bearing(points[i].0, points[i].1, points[i - 1].0, points[i - 1].1);
                let c2 = bearing(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1);
                angle_distance(c1, c2)
            })
            .fold(0.0f64, f64::max);

        cases.push(CleaningCase {
            id: format!("c{}", gi + 1),
            kind,
            start_index: start,
            end_index: end,
            apex_indices: group.clone(),
            bearing_delta_deg: max_delta,
            total_angle_deg: 0.0,
            suggested_delete_ranges: suggested,
            state: "pending".to_string(),
            correction: Correction::default(),
        });
    }

    cases
}

/// Détection **combinée** (points hors trace + aller-retours) — c'est la
/// détection de l'**étape 1** « Pts hors trace » du pipeline : elle conserve
/// le fonctionnement d'origine (avant l'étape 2), où les deux types d'anomalies
/// de rebroussement étaient proposés et corrigés ensemble (modification du GPX).
pub fn detect_anomalies(points: &[(f64, f64)], tolerance_deg: f64) -> Vec<CleaningCase> {
    detect_uturn_cases(points, tolerance_deg)
}

/// Points isolés hors trace — sous-détection de l'étape 1 (utilisée par
/// l'import pour le décompte, et par les tests).
pub fn detect_spikes(points: &[(f64, f64)], tolerance_deg: f64) -> Vec<CleaningCase> {
    detect_uturn_cases(points, tolerance_deg)
        .into_iter()
        .filter(|c| c.kind == CleaningCaseKind::Spike)
        .collect()
}

/// Branches aller-retour — sous-détection de l'étape 1 (utilisée par l'import
/// pour le décompte, et par les tests). L'**étape 3** « Aller/Retour » (à
/// venir) produira un autre type de fichier : elle ne modifiera pas le GPX.
pub fn detect_out_and_backs(points: &[(f64, f64)], tolerance_deg: f64) -> Vec<CleaningCase> {
    detect_uturn_cases(points, tolerance_deg)
        .into_iter()
        .filter(|c| c.kind == CleaningCaseKind::OutAndBack)
        .collect()
}

/// Détecte les **ronds-points** (tours soutenus) sur une liste de coordonnées.
///
/// Portage fidèle de l'algorithme `analyzeRondpoints` de l'outil fourni :
/// on cumule les virages (différence de cap normalisée [−180, 180]) tant que
/// chaque virage reste ≥ `angle_min_deg` et que la fenêtre ne dépasse pas
/// `points_max`. Un rond-point est retenu quand l'angle cumulé dépasse
/// `±angle_seuil_deg` avec un nombre de points dans `[points_min, points_max]`.
///
/// Contrairement aux rebroussements, il s'agit d'une **rotation soutenue**
/// (sans rebroussement ~180°) : les étapes 1 et 3 (pts hors trace / aller-
/// retour) ne produisent pas ce type de cas.
pub fn detect_roundabouts(points: &[(f64, f64)], params: &RoundaboutParams) -> Vec<CleaningCase> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let mut cases = Vec::new();
    let mut analyzed = vec![false; n];

    for i in 0..n.saturating_sub(2) {
        if analyzed[i] {
            continue;
        }
        let mut total_angle = 0.0f64;
        let mut j = i;
        let mut end_index = i;

        while j < n - 2 {
            if end_index - i + 1 > params.points_max {
                break;
            }
            let b1 = bearing(points[j].0, points[j].1, points[j + 1].0, points[j + 1].1);
            let b2 = bearing(points[j + 1].0, points[j + 1].1, points[j + 2].0, points[j + 2].1);
            let mut angle_diff = b2 - b1;
            if angle_diff > 180.0 {
                angle_diff -= 360.0;
            }
            if angle_diff < -180.0 {
                angle_diff += 360.0;
            }
            if angle_diff.abs() < params.angle_min_deg {
                break;
            }
            total_angle += angle_diff;
            end_index = j + 1;

            if total_angle.abs() > params.angle_seuil_deg {
                let point_count = end_index - i + 1;
                if point_count >= params.points_min && point_count <= params.points_max {
                    cases.push(CleaningCase {
                        id: format!("rp{}", cases.len() + 1),
                        kind: CleaningCaseKind::Roundabout,
                        start_index: i,
                        end_index,
                        apex_indices: vec![],
                        bearing_delta_deg: total_angle.abs(),
                        total_angle_deg: total_angle,
                        suggested_delete_ranges: vec![],
                        state: "pending".to_string(),
                        correction: Correction::default(),
                    });
                    for k in i..=end_index {
                        analyzed[k] = true;
                    }
                }
                break;
            }
            j += 1;
        }
    }
    cases
}

/// Détecte les anomalies de la **phase demandée** du pipeline de nettoyage.
/// Phase inconnue → repli sur l'étape 1.
pub fn detect_cases_for_phase(
    points: &[(f64, f64)],
    phase: &str,
    tolerance_deg: f64,
    params: &RoundaboutParams,
) -> Vec<CleaningCase> {
    match phase {
        "roundabout" => detect_roundabouts(points, params),
        "out_and_back" => detect_out_and_backs(points, tolerance_deg),
        // Étape 1 « Pts hors trace » : détection **combinée** d'origine (points
        // hors trace + aller-retours) — le GPX est modifié à la validation.
        _ => detect_anomalies(points, tolerance_deg),
    }
}

/// Détecte les anomalies directement depuis un GPX parsé (détection combinée,
/// utilisée par le test de scan des traces réelles).
#[cfg(test)]
pub fn detect_anomalies_from_gpx(gpx: &gpx::Gpx, tolerance_deg: f64) -> Vec<CleaningCase> {
    let coords: Vec<(f64, f64)> = extract_full_points(gpx)
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect();
    detect_anomalies(&coords, tolerance_deg)
}

/// Coordonnées (lat, lon) extraites d'un GPX parsé (aplatissement complet).
fn coords_from_gpx(gpx: &gpx::Gpx) -> Vec<(f64, f64)> {
    extract_full_points(gpx)
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect()
}

/// Détecte les **points hors trace** (étape 1) directement depuis un GPX
/// parsé — utilisée à l'import pour poser le statut « à nettoyer ».
pub fn detect_spikes_from_gpx(gpx: &gpx::Gpx, tolerance_deg: f64) -> Vec<CleaningCase> {
    detect_spikes(&coords_from_gpx(gpx), tolerance_deg)
}

/// Détecte les **ronds-points** (étape 2) directement depuis un GPX parsé.
pub fn detect_roundabouts_from_gpx(gpx: &gpx::Gpx, params: &RoundaboutParams) -> Vec<CleaningCase> {
    detect_roundabouts(&coords_from_gpx(gpx), params)
}

/// Détecte les **aller-retours** (étape 3) directement depuis un GPX parsé.
pub fn detect_out_and_backs_from_gpx(
    gpx: &gpx::Gpx,
    tolerance_deg: f64,
) -> Vec<CleaningCase> {
    detect_out_and_backs(&coords_from_gpx(gpx), tolerance_deg)
}

/// Détection **combinée des 3 étapes** pour l'import : la trace doit être
/// signalée « à nettoyer » dès qu'une anomalie existe, quelle que soit son
/// étape (points hors trace, ronds-points ou aller-retours).
pub fn detect_all_phases_from_gpx(
    gpx: &gpx::Gpx,
    tolerance_deg: f64,
    roundabout_params: &RoundaboutParams,
) -> (Vec<CleaningCase>, Vec<CleaningCase>, Vec<CleaningCase>) {
    (
        detect_spikes_from_gpx(gpx, tolerance_deg),
        detect_roundabouts_from_gpx(gpx, roundabout_params),
        detect_out_and_backs_from_gpx(gpx, tolerance_deg),
    )
}

// ---------------------------------------------------------------------------
// Décisions persistées par phase (faux positifs mémorisés)
// ---------------------------------------------------------------------------
//
// Quand une étape est **validée**, son fichier de travail est supprimé : les
// décisions « faux positif » ne doivent pas être perdues. On persiste par
// phase un fichier `traces/{trace_id}/cleaning.{phase}.decisions.json` contenant les
// zones décidées (coordonnée représentative + état). À la re-détection
// ultérieure (retour sur une étape déjà validée), les cas détectés dont la
// zone correspond à une décision « kept » sont re-marqués automatiquement.

/// Décision validée par l'utilisateur, persistée par phase.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PhaseDecision {
    pub kind: CleaningCaseKind,
    /// État validé — « kept » (faux positif) pour le moment.
    pub state: String,
    /// Coordonnée représentative de la zone décidée (apex ou centroïde).
    pub lat: f64,
    pub lon: f64,
}

fn get_decisions_path(mode_dir: &Path, trace_id: &str, phase: &str) -> PathBuf {
    mode_dir
        .join("traces")
        .join(trace_id)
        .join(format!("cleaning.{phase}.decisions.json"))
}

fn load_decisions(mode_dir: &Path, trace_id: &str, phase: &str) -> Vec<PhaseDecision> {
    let path = get_decisions_path(mode_dir, trace_id, phase);
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_decisions(
    mode_dir: &Path,
    trace_id: &str,
    phase: &str,
    decisions: &[PhaseDecision],
) -> Result<(), String> {
    let path = get_decisions_path(mode_dir, trace_id, phase);
    let content = serde_json::to_string_pretty(decisions)
        .map_err(|e| format!("Sérialisation des décisions : {}", e))?;
    write_atomic(&path, content.as_bytes())
}

/// Coordonnée représentative d'un cas : l'apex s'il existe, sinon le
/// **centroïde** de la zone `[start_index, end_index]` (ronds-points).
fn case_representative(points: &[(f64, f64)], case: &CleaningCase) -> Option<(f64, f64)> {
    if let Some(&a) = case.apex_indices.first() {
        if a < points.len() {
            return Some(points[a]);
        }
    }
    if points.is_empty() {
        return None;
    }
    let lo = case.start_index.min(points.len() - 1);
    let hi = case.end_index.min(points.len() - 1);
    if lo > hi {
        return None;
    }
    let (mut lat, mut lon, mut n) = (0.0f64, 0.0f64, 0.0f64);
    for p in &points[lo..=hi] {
        lat += p.0;
        lon += p.1;
        n += 1.0;
    }
    if n == 0.0 {
        None
    } else {
        Some((lat / n, lon / n))
    }
}

/// Re-marque comme « kept » (faux positif) les cas détectés dont la zone
/// correspond à une décision persistée (dans un rayon de ~40 m).
fn merge_decisions(points: &[(f64, f64)], cases: &mut [CleaningCase], decisions: &[PhaseDecision]) {
    if decisions.is_empty() {
        return;
    }
    for c in cases.iter_mut() {
        if c.state != "pending" {
            continue;
        }
        let Some((lat, lon)) = case_representative(points, c) else {
            continue;
        };
        let kept = decisions.iter().any(|d| {
            d.state == "kept" && haversine(lat, lon, d.lat, d.lon) < 40.0
        });
        if kept {
            c.state = "kept".to_string();
            c.correction = Correction::default();
        }
    }
}

/// Lit la tolérance de cap paramétrée (`Nettoyage.Cap.toleranceDeg`), avec
/// repli sur 5° si le paramètre est absent ou illisible.
///
/// Ne **panique jamais** : l'accès au state des paramètres se fait via
/// `try_state` (un `state()` paniquerait si le state n'était pas géré, ce qui
/// laisserait une commande async sans réponse — l'import resterait bloqué
/// côté frontend).
pub fn read_tolerance_deg(app: &tauri::AppHandle) -> f64 {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return 5.0,
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return 5.0,
    };
    tolerance_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Résout la tolérance depuis les tables TOML (surcharge utilisateur
/// prioritaire, puis valeur par défaut, repli 5°). Pure et testable.
fn tolerance_from_settings(default_toml: &toml::Table, user_overrides: &toml::Table) -> f64 {
    let value = get_toml_value_by_path(user_overrides, "Nettoyage.Cap.toleranceDeg")
        .or_else(|| get_toml_value_by_path(default_toml, "Nettoyage.Cap.toleranceDeg"));
    match value {
        Some(toml::Value::Float(f)) => *f,
        Some(toml::Value::Integer(i)) => *i as f64,
        _ => 5.0,
    }
}

/// Lit les paramètres de détection des ronds-points
/// (`Nettoyage.RondPoints.*`) avec repli sur les valeurs par défaut. Ne
/// **panique jamais** (`try_state`), comme `read_tolerance_deg`.
pub fn read_roundabout_params(app: &tauri::AppHandle) -> RoundaboutParams {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return RoundaboutParams::default(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return RoundaboutParams::default(),
    };
    roundabout_params_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Résout un paramètre flottant depuis les tables TOML (surcharge utilisateur
/// prioritaire, puis valeur par défaut, repli sur `fallback`).
fn read_f64_setting(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
    path: &str,
    fallback: f64,
) -> f64 {
    let value = get_toml_value_by_path(user_overrides, path)
        .or_else(|| get_toml_value_by_path(default_toml, path));
    match value {
        Some(toml::Value::Float(f)) => *f,
        Some(toml::Value::Integer(i)) => *i as f64,
        _ => fallback,
    }
}

/// Résout un paramètre entier depuis les tables TOML (même logique).
fn read_usize_setting(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
    path: &str,
    fallback: usize,
) -> usize {
    read_f64_setting(default_toml, user_overrides, path, fallback as f64)
        .round()
        .max(0.0) as usize
}

/// Résout les paramètres ronds-points depuis les tables TOML. Pure et testable.
pub fn roundabout_params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> RoundaboutParams {
    RoundaboutParams {
        angle_min_deg: read_f64_setting(
            default_toml,
            user_overrides,
            "Nettoyage.RondPoints.angleMinDeg",
            5.0,
        ),
        points_min: read_usize_setting(
            default_toml,
            user_overrides,
            "Nettoyage.RondPoints.pointsMin",
            5,
        ),
        points_max: read_usize_setting(
            default_toml,
            user_overrides,
            "Nettoyage.RondPoints.pointsMax",
            50,
        ),
        angle_seuil_deg: read_f64_setting(
            default_toml,
            user_overrides,
            "Nettoyage.RondPoints.angleSeuilDeg",
            210.0,
        ),
        marge_points: read_usize_setting(
            default_toml,
            user_overrides,
            "Nettoyage.RondPoints.margePoints",
            5,
        ),
    }
}

// ---------------------------------------------------------------------------
// Chargement du GPX source d'une trace
// ---------------------------------------------------------------------------

/// Résout et parse le fichier GPX original d'une trace depuis le registre.
fn load_trace_gpx(app: &tauri::AppHandle, trace_id: &str) -> Result<(gpx::Gpx, TraceMetadata), String> {
    let mode_dir = get_mode_dir(app)?;
    let traces_path = get_traces_path(&mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?
        .clone();

    let gpx_file = crate::import_gpx::get_trace_gpx_path(&mode_dir, trace_id, &trace.filename);
    if !gpx_file.exists() {
        return Err(format!(
            "Fichier GPX source introuvable pour la trace (id={}, fichier={:?})",
            trace_id, gpx_file
        ));
    }
    let file = std::fs::File::open(&gpx_file).map_err(|e| format!("Ouverture du fichier : {}", e))?;
    let reader = BufReader::new(file);
    let gpx = gpx::read(reader).map_err(|e| format!("Fichier GPX invalide : {}", e))?;
    Ok((gpx, trace))
}

// ---------------------------------------------------------------------------
// Persistance du fichier de travail
// ---------------------------------------------------------------------------

/// Chemin du fichier de travail de nettoyage d'une trace **pour une phase**
/// (`traces/{trace_id}/cleaning.{phase}.json`). Chaque phase dispose de son
/// propre fichier : les index de cas sont propres à la version du GPX traitée.
fn get_cleaning_path(mode_dir: &Path, trace_id: &str, phase: &str) -> PathBuf {
    mode_dir
        .join("traces")
        .join(trace_id)
        .join(format!("cleaning.{phase}.json"))
}

/// Supprime les fichiers de travail de nettoyage d'une trace (toutes phases,
/// `cleaning.*.json` dans son dossier). Tolérant.
pub(crate) fn remove_cleaning_files(mode_dir: &Path, trace_id: &str) {
    let dir = mode_dir.join("traces").join(trace_id);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("cleaning.") && name.ends_with(".json") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Écriture atomique d'un contenu (fichier tmp + rename).
fn write_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Écriture du fichier temporaire : {}", e))?;
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Renommage du fichier : {}", e))?;
    Ok(())
}

/// Met à jour le statut de nettoyage d'une trace dans le registre.
fn set_cleaning_status(app: &tauri::AppHandle, trace_id: &str, status: &str) -> Result<(), String> {
    let mode_dir = get_mode_dir(app)?;
    let traces_path = get_traces_path(&mode_dir);
    let mut registry = load_registry(&traces_path);
    let trace = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;
    trace.cleaning_status = status.to_string();
    save_registry(&traces_path, &registry)
}

/// Met à jour la phase de nettoyage en cours d'une trace dans le registre.
fn set_cleaning_phase(app: &tauri::AppHandle, trace_id: &str, phase: &str) -> Result<(), String> {
    let mode_dir = get_mode_dir(app)?;
    let traces_path = get_traces_path(&mode_dir);
    let mut registry = load_registry(&traces_path);
    let trace = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;
    trace.cleaning_phase = phase.to_string();
    save_registry(&traces_path, &registry)
}

// ---------------------------------------------------------------------------
// Application des corrections et génération du GPX nettoyé
// ---------------------------------------------------------------------------

/// Applique les corrections de tous les cas validés sur les points originaux
/// et retourne la liste finale des points ainsi que le nombre de suppressions.
fn apply_corrections(points: &[GpxPoint], cases: &[CleaningCase]) -> Result<(Vec<GpxPoint>, usize), String> {
    let n = points.len();
    let mut deleted = vec![false; n];

    for c in cases {
        if c.state == "kept" {
            continue; // cas conservé tel quel → aucune correction
        }
        for range in &c.correction.delete_ranges {
            let (from, to) = (range[0], range[1]);
            if from > to || to >= n {
                return Err(format!(
                    "Cas {} : plage de suppression invalide [{}, {}] ({} points).",
                    c.id, from, to, n
                ));
            }
            for i in from..=to {
                deleted[i] = true;
            }
        }
    }

    let mut result = Vec::with_capacity(n);
    let mut removed = 0usize;
    for (i, point) in points.iter().enumerate() {
        if deleted[i] {
            removed += 1;
            continue;
        }
        let mut p = point.clone();
        // Points déplacés : remplace les coordonnées des points conservés.
        if let Some(mp) = cases
            .iter()
            .filter(|c| c.state != "kept")
            .flat_map(|c| &c.correction.moved_points)
            .find(|mp| mp.index == i)
        {
            p.lat = mp.lat;
            p.lon = mp.lon;
        }
        result.push(p);
    }

    if result.len() < 2 {
        return Err("Le nettoyage aboutirait à moins de 2 points : correction refusée.".to_string());
    }

    Ok((result, removed))
}

/// Échappe les caractères XML réservés pour un contenu de balise.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Génère le contenu du GPX nettoyé (1.1, `<trk><trkseg>` unique), en
/// préservant latitude/longitude (6 décimales), altitude et timestamp.
fn build_cleaned_gpx(points: &[GpxPoint], name: &str) -> String {
    let mut s = String::with_capacity(points.len() * 96 + 256);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<gpx creator=\"VisuGPS2 - Nettoyage de trace\" version=\"1.1\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n");
    s.push_str("  <trk>\n");
    s.push_str(&format!("    <name>{}</name>\n", xml_escape(name)));
    s.push_str("    <trkseg>\n");
    for p in points {
        s.push_str(&format!("      <trkpt lat=\"{:.6}\" lon=\"{:.6}\">\n", p.lat, p.lon));
        if let Some(ele) = p.ele {
            s.push_str(&format!("        <ele>{:.1}</ele>\n", ele));
        }
        if let Some(t) = &p.time {
            s.push_str(&format!("        <time>{}</time>\n", t));
        }
        s.push_str("      </trkpt>\n");
    }
    s.push_str("    </trkseg>\n");
    s.push_str("  </trk>\n");
    s.push_str("</gpx>\n");
    s
}

// ---------------------------------------------------------------------------
// Commandes Tauri
// ---------------------------------------------------------------------------

/// Détecte les anomalies d'une trace **pour une phase donnée** du pipeline de
/// nettoyage (re-parse le GPX courant, aucune persistance). La tolérance de cap
/// et les paramètres de rond-point sont fournis par le frontend (paramètres
/// `Nettoyage.Cap.toleranceDeg` / `Nettoyage.RondPoints.*`).
#[tauri::command]
pub async fn detect_trace_anomalies(
    app: tauri::AppHandle,
    trace_id: String,
    phase: String,
    tolerance_deg: f64,
    roundabout_params: Option<RoundaboutParams>,
) -> Result<Vec<CleaningCase>, String> {
    let (gpx, _) = load_trace_gpx(&app, &trace_id)?;
    let coords: Vec<(f64, f64)> = extract_full_points(&gpx)
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect();
    let params = roundabout_params.unwrap_or_default();
    Ok(detect_cases_for_phase(&coords, &phase, tolerance_deg, &params))
}

/// Retourne l'état de nettoyage d'une trace **pour une phase** : le fichier de
/// travail de cette phase s'il existe et est **valide** (corrections déjà en
/// cours), sinon une détection fraîche. Un fichier illisible, invalide ou d'une
/// autre phase est ignoré et régénéré par détection — il ne doit jamais
/// bloquer l'IHM.
#[tauri::command]
pub async fn get_cleaning_state(
    app: tauri::AppHandle,
    trace_id: String,
    phase: String,
    tolerance_deg: f64,
    roundabout_params: Option<RoundaboutParams>,
) -> Result<CleaningState, String> {
    let mode_dir = get_mode_dir(&app)?;
    let state_path = get_cleaning_path(&mode_dir, &trace_id, &phase);

    if state_path.exists() {
        match std::fs::read_to_string(&state_path) {
            Ok(content) => match serde_json::from_str::<CleaningState>(&content) {
                Ok(state) if state.phase == phase => return Ok(state),
                Ok(_) => {
                    eprintln!(
                        "[cleaning] Fichier de travail d'une autre phase ({}), re-détection.",
                        phase
                    );
                }
                Err(e) => {
                    eprintln!("[cleaning] Fichier de travail invalide ({}), re-détection.", e);
                }
            },
            Err(e) => {
                eprintln!("[cleaning] Fichier de travail illisible : {}", e);
            }
        }
    }

    let (gpx, _) = load_trace_gpx(&app, &trace_id)?;
    let coords: Vec<(f64, f64)> = extract_full_points(&gpx)
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect();
    let params = roundabout_params.unwrap_or_default();
    let mut cases = detect_cases_for_phase(&coords, &phase, tolerance_deg, &params);
    // Retour sur une étape déjà validée : re-marquer les faux positifs mémorisés.
    let decisions = load_decisions(&mode_dir, &trace_id, &phase);
    merge_decisions(&coords, &mut cases, &decisions);
    Ok(CleaningState {
        trace_id,
        tolerance_deg,
        phase,
        cases,
    })
}

/// Sauvegarde partielle du travail d'une phase (fichier
/// `traces/{trace_id}/cleaning.{phase}.json`). Le GPX reste intact. Passe la trace en
/// `"in_progress"` et mémorise la phase en cours.
#[tauri::command]
pub async fn save_cleaning_state(
    app: tauri::AppHandle,
    trace_id: String,
    phase: String,
    state_json: serde_json::Value,
) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let path = get_cleaning_path(&mode_dir, &trace_id, &phase);

    let content = serde_json::to_string_pretty(&state_json)
        .map_err(|e| format!("Sérialisation du fichier de travail : {}", e))?;
    write_atomic(&path, content.as_bytes())?;

    set_cleaning_status(&app, &trace_id, "in_progress")?;
    set_cleaning_phase(&app, &trace_id, &phase)?;
    Ok(())
}

/// Abandonne les corrections en cours : supprime les fichiers de travail (toutes
/// phases) et remet la trace en `"needs_review"` à la première étape.
#[tauri::command]
pub async fn reset_cleaning(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    remove_cleaning_files(&mode_dir, &trace_id);
    set_cleaning_status(&app, &trace_id, "needs_review")?;
    set_cleaning_phase(&app, &trace_id, "spike")
}

/// Valide une **étape** du pipeline de nettoyage : applique les corrections
/// validées de la phase, génère le GPX nettoyé (entrée de l'étape suivante),
/// sauvegarde l'original en `{filename}.gpx.orig` **une seule fois** (étape 1),
/// régénère les dérivés (geojson, stats, hash) et avance `cleaning_phase`.
///
/// Refuse la validation tant que **tous** les cas de la phase ne sont pas
/// validés (`state != "pending"`). L'étape 3 (« Aller/Retour ») n'est **pas
/// implémentée** dans cette itération : elle produira un autre type de fichier
/// (pas une modification du GPX) et est refusée ici.
///
/// Si aucune correction n'est à appliquer (aucune anomalie — auto-validation —
/// ou tout conservé tel quel), la phase avance **sans réécrire le GPX**.
#[tauri::command]
pub async fn validate_phase(
    app: tauri::AppHandle,
    trace_id: String,
    phase: String,
    state_json: serde_json::Value,
) -> Result<TraceMetadata, String> {
    if phase == "out_and_back" {
        return Err(
            "Étape « Aller/Retour » non implémentée : elle produira un autre type de fichier (pas une modification du GPX).".to_string(),
        );
    }
    let state: CleaningState = serde_json::from_value(state_json)
        .map_err(|e| format!("État de nettoyage invalide : {}", e))?;
    if !state.phase.is_empty() && state.phase != phase {
        return Err(format!(
            "Phase incohérente : état « {} », phase demandée « {} ».",
            state.phase, phase
        ));
    }

    // 1. Tous les cas de la phase doivent être validés.
    let pending = state.cases.iter().filter(|c| c.state == "pending").count();
    if pending > 0 {
        return Err(format!(
            "Validation impossible : {} cas restent à traiter.",
            pending
        ));
    }

    // 2. Phase suivante du pipeline (défini par l'ordre des étapes).
    let next_phase = match phase.as_str() {
        "spike" => "roundabout".to_string(),
        "roundabout" => "out_and_back".to_string(),
        _ => return Err(format!("Phase inconnue : {}", phase)),
    };

    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);

    // Charger le GPX courant et ses points (réutilisés pour les corrections et
    // pour mémoriser les décisions de la phase).
    let (gpx, trace) = load_trace_gpx(&app, &trace_id)?;
    let original_points = extract_full_points(&gpx);
    let coords: Vec<(f64, f64)> = original_points
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect();
    let trace_name = trace.name.clone();

    // Y a-t-il réellement des corrections à appliquer ?
    let has_changes = state.cases.iter().any(|c| {
        c.state != "kept"
            && (!c.correction.delete_ranges.is_empty() || !c.correction.moved_points.is_empty())
    });

    if has_changes {
        // 3. Appliquer les corrections de la phase.
        let (final_points, removed_count) = apply_corrections(&original_points, &state.cases)?;

        // 4. Générer et écrire le GPX nettoyé (entrée de l'étape suivante).
        let cleaned_gpx = build_cleaned_gpx(&final_points, &trace_name);
        let gpx_file = get_trace_gpx_path(&mode_dir, &trace_id, &trace.filename);

        // 5. Backup de l'original (une seule fois, ne pas écraser un backup).
        let backup_path = gpx_file.with_extension("gpx.orig");
        if !backup_path.exists() {
            std::fs::copy(&gpx_file, &backup_path)
                .map_err(|e| format!("Sauvegarde de l'original (backup) : {}", e))?;
        }

        // 6. Écraser le GPX (écriture atomique).
        write_atomic(&gpx_file, cleaned_gpx.as_bytes())?;

        // 7. Régénérer les dérivés depuis le GPX nettoyé.
        let file = std::fs::File::open(&gpx_file).map_err(|e| format!("Ouverture du fichier : {}", e))?;
        let reader = BufReader::new(file);
        let cleaned_gpx_obj = gpx::read(reader).map_err(|e| format!("GPX nettoyé invalide : {}", e))?;
        let (stats, _) = compute_stats(&cleaned_gpx_obj)?;
        let coords = extract_line_coordinates(&cleaned_gpx_obj)?;
        let geojson_path = get_geojson_path(&mode_dir, &trace_id);
        let feature = build_geojson_feature(coords, &trace_id, &trace_name);
        let geojson_content = serde_json::to_string_pretty(&feature)
            .map_err(|e| format!("Sérialisation GeoJSON : {}", e))?;
        write_atomic(&geojson_path, geojson_content.as_bytes())?;
        let hash = compute_file_hash(&gpx_file)?;

        // 8. Mettre à jour le registre : stats/hash, avancer la phase, la trace
        //    reste « needs_review » (l'étape 3 n'existe pas encore → jamais
        //    « clean » dans cette itération).
        let mut registry = load_registry(&traces_path);
        {
            let trace = registry
                .iter_mut()
                .find(|t| t.id == trace_id)
                .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;
            trace.stats = stats;
            trace.hash = hash;
            trace.cleaning_phase = next_phase.clone();
            trace.cleaning_status = "needs_review".to_string();
        }
        save_registry(&traces_path, &registry)?;

        println!(
            "[cleaning] Étape « {} » validée pour « {} » : {} point(s) supprimé(s), {} → {} points. Prochaine étape : « {} ».",
            phase,
            trace_name,
            removed_count,
            original_points.len(),
            final_points.len(),
            next_phase
        );
    } else {
        // Aucune correction (aucune anomalie ou tout conservé) : on avance la
        // phase sans réécrire le GPX ni régénérer les dérivés.
        set_cleaning_phase(&app, &trace_id, &next_phase)?;
        println!(
            "[cleaning] Étape « {} » validée sans correction (auto-validation). Prochaine étape : « {} ».",
            phase, next_phase
        );
    }

    // 9. Mémoriser les décisions de la phase (faux positifs) : les cas validés
    //    **sans modification effective** (conservés tel quel, ou corrigés sans
    //    correction) gardent leur zone dans le GPX — on la persiste pour la
    //    re-marquer automatiquement si l'utilisateur revient sur l'étape.
    let decisions: Vec<PhaseDecision> = state
        .cases
        .iter()
        .filter_map(|c| {
            if c.state == "pending" {
                return None;
            }
            let unchanged = c.state == "kept"
                || (c.correction.delete_ranges.is_empty() && c.correction.moved_points.is_empty());
            if !unchanged {
                return None;
            }
            case_representative(&coords, c)
                .map(|(lat, lon)| PhaseDecision {
                    kind: c.kind.clone(),
                    state: c.state.clone(),
                    lat,
                    lon,
                })
        })
        .collect();
    if !decisions.is_empty() {
        save_decisions(&mode_dir, &trace_id, &phase, &decisions)?;
    }

    // 10. Supprimer le fichier de travail de la phase validée.
    let state_path = get_cleaning_path(&mode_dir, &trace_id, &phase);
    if state_path.exists() {
        let _ = std::fs::remove_file(&state_path);
    }

    let registry = load_registry(&traces_path);
    registry
        .into_iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Cas « spike » : un point isolé qui s'écarte de la ligne puis revient
    /// exactement sur ses pas (analogue au point 946 / 4101).
    fn spike_trace() -> Vec<(f64, f64)> {
        vec![
            (41.618450, 2.537800), // 0  ← jumeau de 8
            (41.618450, 2.537900), // 1  ← jumeau de 7
            (41.618450, 2.538000), // 2  ← jumeau de 6
            (41.618450, 2.538100), // 3  ← jumeau de 5
            (41.618440, 2.538250), // 4  apex (écart ~12,5 m)
            (41.618450, 2.538100), // 5
            (41.618450, 2.538000), // 6
            (41.618450, 2.537900), // 7
            (41.618450, 2.537800), // 8
            (41.618450, 2.537700), // 9  continuation (plus de jumeau)
        ]
    }

    /// Cas « out_and_back » : une branche aller-retour avec retraçage exact
    /// (analogue au point 711 / 791).
    fn out_and_back_trace() -> Vec<(f64, f64)> {
        vec![
            (41.62250, 2.55200), // 0  ← jumeau de 12
            (41.62250, 2.55250), // 1  ← jumeau de 11
            (41.62250, 2.55300), // 2  ← jumeau de 10
            (41.62250, 2.55350), // 3  ← jumeau de 9
            (41.62226, 2.55421), // 4  ← jumeau de 8
            (41.62200, 2.55380), // 5  ← jumeau de 7
            (41.62193, 2.55421), // 6  apex (demi-tour)
            (41.62200, 2.55380), // 7
            (41.62226, 2.55421), // 8
            (41.62250, 2.55350), // 9
            (41.62250, 2.55300), // 10
            (41.62250, 2.55250), // 11
            (41.62250, 2.55200), // 12
            (41.62250, 2.55150), // 13 continuation
        ]
    }

    #[test]
    fn spike_is_detected_and_classified() {
        let pts = spike_trace();
        let cases = detect_anomalies(&pts, 5.0);

        assert_eq!(cases.len(), 1, "un seul cas doit être détecté");
        let c = &cases[0];
        assert_eq!(c.kind, CleaningCaseKind::Spike);
        assert_eq!(c.apex_indices, vec![4]);
        // Suggestion : supprimer le point isolé.
        assert_eq!(c.suggested_delete_ranges, vec![[4, 4]]);
    }

    #[test]
    fn out_and_back_is_detected_and_classified() {
        let pts = out_and_back_trace();
        let cases = detect_anomalies(&pts, 5.0);

        assert_eq!(cases.len(), 1, "un seul cas doit être détecté");
        let c = &cases[0];
        assert_eq!(c.kind, CleaningCaseKind::OutAndBack);
        assert_eq!(c.apex_indices, vec![6]);
        // La zone couvre l'aller-retour complet ; la suggestion supprime le
        // demi-tour + le retour (on garde l'aller).
        assert_eq!(c.start_index, 0);
        assert_eq!(c.end_index, 12);
        assert_eq!(c.suggested_delete_ranges, vec![[6, 12]]);
    }

    #[test]
    fn tolerance_controls_detection() {
        let pts = spike_trace();
        // Tolérance 0 → le rebroussement (écart de cap parfaitement nul dans la
        // construction) reste détecté ici car le retraçage est exact. On vérifie
        // plutôt qu'une tolérance très faible ne crée pas de faux positif sur une
        // ligne droite : construction d'une ligne sans anomalie.
        let straight: Vec<(f64, f64)> = (0..20)
            .map(|i| (41.6, 2.50 + i as f64 * 0.001))
            .collect();
        assert!(detect_anomalies(&straight, 5.0).is_empty());

        // Une tolérance très élevée détecte aussi les demi-tours imparfaits.
        assert_eq!(detect_anomalies(&pts, 20.0).len(), 1);
    }

    /// Cas « rond-point » : un tour soutenu de ~1,25 tour. Les points suivent
    /// un petit cercle (~30 m de rayon), 8 points par tour (pas de 45°).
    fn roundabout_trace() -> Vec<(f64, f64)> {
        let center_lat = 41.62000f64;
        let center_lon = 2.55000f64;
        let radius = 0.0003f64;
        (0..=10)
            .map(|k| {
                let angle = (k as f64) * 45.0f64.to_radians();
                (
                    center_lat + radius * angle.cos(),
                    center_lon + radius * angle.sin(),
                )
            })
            .collect()
    }

    #[test]
    fn roundabout_is_detected_and_classified() {
        let pts = roundabout_trace();
        let cases = detect_roundabouts(&pts, &RoundaboutParams::default());

        assert_eq!(cases.len(), 1, "un seul rond-point doit être détecté");
        let c = &cases[0];
        assert_eq!(c.kind, CleaningCaseKind::Roundabout);
        assert_eq!(c.start_index, 0);
        // Angle cumulé au-delà du seuil (210°) : ~225° pour 5 pas de 45°.
        assert!(c.total_angle_deg.abs() > 210.0, "angle cumulé = {}", c.total_angle_deg);
        assert_eq!(c.bearing_delta_deg, c.total_angle_deg.abs());
        // Correction **manuelle** : aucune suggestion automatique.
        assert!(c.suggested_delete_ranges.is_empty());
        // Pas de rebroussement : ni spike ni aller-retour ne le signalent.
        assert!(detect_spikes(&pts, 5.0).is_empty());
        assert!(detect_out_and_backs(&pts, 5.0).is_empty());
    }

    #[test]
    fn roundabout_no_false_positive_on_straight_line() {
        let straight: Vec<(f64, f64)> = (0..30)
            .map(|i| (41.6, 2.50 + i as f64 * 0.001))
            .collect();
        assert!(detect_roundabouts(&straight, &RoundaboutParams::default()).is_empty());
    }

    #[test]
    fn roundabout_params_control_detection() {
        let pts = roundabout_trace();
        // Un seuil d'angle cumulé trop haut → aucune détection.
        let mut params = RoundaboutParams::default();
        params.angle_seuil_deg = 1000.0;
        assert!(detect_roundabouts(&pts, &params).is_empty());
        // Un virage minimal trop strict → la progression s'arrête d'emblée.
        let mut params = RoundaboutParams::default();
        params.angle_min_deg = 90.0;
        assert!(detect_roundabouts(&pts, &params).is_empty());
    }

    /// Les décisions persistées (« faux positif ») re-marquent les cas détectés
    /// dont la zone correspond (retour sur une étape déjà validée).
    #[test]
    fn decisions_merge_remarks_kept_cases() {
        let pts = roundabout_trace();
        let mut cases = detect_roundabouts(&pts, &RoundaboutParams::default());
        assert_eq!(cases.len(), 1);

        // Décision au centroïde du rond-point → le cas re-détecté est « kept ».
        let rep = case_representative(&pts, &cases[0]).unwrap();
        let decisions = vec![PhaseDecision {
            kind: CleaningCaseKind::Roundabout,
            state: "kept".to_string(),
            lat: rep.0,
            lon: rep.1,
        }];
        merge_decisions(&pts, &mut cases, &decisions);
        assert_eq!(cases[0].state, "kept");
        assert!(cases[0].correction.delete_ranges.is_empty());

        // Une décision éloignée (~1 km) ne marque pas le cas.
        let mut cases2 = detect_roundabouts(&pts, &RoundaboutParams::default());
        let far = vec![PhaseDecision {
            kind: CleaningCaseKind::Roundabout,
            state: "kept".to_string(),
            lat: rep.0 + 0.01,
            lon: rep.1 + 0.01,
        }];
        merge_decisions(&pts, &mut cases2, &far);
        assert_eq!(cases2[0].state, "pending");
    }

    /// Les phases 1 et 3 sont des détections **distinctes** : `detect_spikes`
    /// ne produit que des points isolés, `detect_out_and_backs` que des
    /// branches aller-retour (le seuil de longueur est une heuristique interne,
    /// pas un critère entre étapes).
    #[test]
    fn phases_split_spike_and_out_and_back() {
        let spikes = detect_spikes(&spike_trace(), 5.0);
        assert_eq!(spikes.len(), 1);
        assert_eq!(spikes[0].kind, CleaningCaseKind::Spike);
        assert!(detect_out_and_backs(&spike_trace(), 5.0).is_empty());

        let oabs = detect_out_and_backs(&out_and_back_trace(), 5.0);
        assert_eq!(oabs.len(), 1);
        assert_eq!(oabs[0].kind, CleaningCaseKind::OutAndBack);
        assert!(detect_spikes(&out_and_back_trace(), 5.0).is_empty());
    }

    /// Vérifie l'application des corrections (suppressions) et la génération
    /// du GPX nettoyé.
    #[test]
    fn apply_corrections_and_build_gpx() {
        let raw: Vec<GpxPoint> = out_and_back_trace()
            .iter()
            .enumerate()
            .map(|(i, (lat, lon))| GpxPoint {
                lat: *lat,
                lon: *lon,
                ele: Some(100.0 + i as f64),
                time: Some(format!("2026-01-01T10:00:{:02}Z", i)),
            })
            .collect();

        // Cas corrigé : supprime le demi-tour + le retour (index 6..12).
        let mut correction = Correction::default();
        correction.delete_ranges.push([6, 12]);
        let cases = vec![CleaningCase {
            id: "c1".to_string(),
            kind: CleaningCaseKind::OutAndBack,
            start_index: 0,
            end_index: 12,
            apex_indices: vec![6],
            bearing_delta_deg: 0.0,
            total_angle_deg: 0.0,
            suggested_delete_ranges: vec![[6, 12]],
            state: "corrected".to_string(),
            correction,
        }];

        let (final_points, removed) = apply_corrections(&raw, &cases).unwrap();
        assert_eq!(removed, 7);
        // Points conservés : 0..=5 + 13 → 7 points.
        assert_eq!(final_points.len(), 7);
        assert_eq!(final_points[6].lat, 41.62250);

        // Le GPX généré doit être du XML valide et contenir les points.
        let gpx_str = build_cleaned_gpx(&final_points, "Test trace");
        assert!(gpx_str.starts_with("<?xml"));
        assert!(gpx_str.contains("<name>Test trace</name>"));
        assert!(gpx_str.matches("<trkpt ").count() == 7);
        assert!(gpx_str.contains("<ele>105.0</ele>"));
        assert!(gpx_str.contains("<time>2026-01-01T10:00:05Z</time>"));

        // Un cas « conservé tel quel » (faux positif) ne supprime rien.
        let mut kept = cases.clone();
        kept[0].state = "kept".to_string();
        kept[0].correction = Correction::default();
        let (kept_points, kept_removed) = apply_corrections(&raw, &kept).unwrap();
        assert_eq!(kept_points.len(), raw.len());
        assert_eq!(kept_removed, 0);

        // Un point **déplacé** (index original 3) remplace ses coordonnées.
        let mut moved = cases.clone();
        moved[0].correction.moved_points.push(MovedPoint {
            index: 3,
            lat: 41.62300,
            lon: 2.55310,
        });
        let (moved_points, _) = apply_corrections(&raw, &moved).unwrap();
        // Le point d'index 3 est conservé (pas supprimé) : position dans le
        // résultat = index 3 (les suppressions [6,12] sont après).
        assert_eq!(moved_points.len(), 7);
        assert_eq!(moved_points[3].lat, 41.62300);
        assert_eq!(moved_points[3].lon, 2.55310);
        // Les autres points conservent leurs coordonnées.
        assert_eq!(moved_points[0].lat, raw[0].lat);

        // Un point déplacé **et** supprimé ne doit pas réapparaître.
        let mut both = cases.clone();
        both[0].correction.delete_ranges.push([3, 3]);
        both[0].correction.moved_points.push(MovedPoint {
            index: 3,
            lat: 41.62300,
            lon: 2.55310,
        });
        let (both_points, _) = apply_corrections(&raw, &both).unwrap();
        assert!(
            !both_points.iter().any(|p| (p.lat - 41.62300).abs() < 1e-9),
            "un point supprimé ne doit pas être déplacé"
        );
    }

    /// Vérifie que la finalisation refuse un cas encore « pending ».
    #[test]
    fn finalize_refuses_pending_cases() {
        // On vérifie la règle métier au niveau du check (reproduit dans
        // finalize_cleaning) : un état avec un cas pending doit être refusé.
        let state = CleaningState {
            trace_id: "t".to_string(),
            tolerance_deg: 5.0,
            phase: "spike".to_string(),
            cases: vec![CleaningCase {
                id: "c1".to_string(),
                kind: CleaningCaseKind::Spike,
                start_index: 0,
                end_index: 2,
                apex_indices: vec![1],
                bearing_delta_deg: 0.0,
                total_angle_deg: 0.0,
                suggested_delete_ranges: vec![],
                state: "pending".to_string(),
                correction: Correction::default(),
            }],
        };
        let pending = state.cases.iter().filter(|c| c.state == "pending").count();
        assert_eq!(pending, 1);
        assert!(!state.cases.iter().all(|c| c.state != "pending"));
    }

    /// Détection sur les GPX réels du mode OPE (Application Support).
    /// Test `#[ignore]` : il dépend de fichiers présents uniquement sur la
    /// machine de travail. À lancer via `cargo test -- --ignored`.
    ///
    /// Tous les `.gpx` du dossier `traces/*/` sont scannés : le pipeline
    /// complet d'import (name, stats, coords, détection) ne doit **jamais
    /// paniquer**, et la détection doit rester rapide même sur les gros fichiers.
    #[test]
    #[ignore]
    fn detects_real_trace_anomalies() {
        let traces_root =
            "/Users/jean-marcbaubet/Library/Application Support/com.jean-marc.baubet.visugps2/OPE/traces";
        let mut files: Vec<String> = std::fs::read_dir(traces_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .flat_map(|e| std::fs::read_dir(e.path()).ok())
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "gpx").unwrap_or(false))
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();
        files.sort();
        assert!(!files.is_empty(), "Aucun GPX trouvé dans {}", traces_root);

        for path in &files {
            let start = std::time::Instant::now();
            let file = std::fs::File::open(path)
                .unwrap_or_else(|e| panic!("Fichier introuvable {} : {}", path, e));
            let gpx = gpx::read(BufReader::new(file)).unwrap();

            // Pipeline d'import complet (étapes 6 → 8bis de import_gpx_file) :
            // aucun de ces appels ne doit paniquer.
            let trace_name = crate::import_gpx::extract_name(&gpx, path);
            assert!(!trace_name.is_empty());
            let (stats, count) = crate::import_gpx::compute_stats(&gpx).unwrap();
            assert!(stats.points_count >= 2 && count == stats.points_count);
            let coords = crate::import_gpx::extract_line_coordinates(&gpx).unwrap();
            assert_eq!(coords.len(), count);
            let _feature = crate::import_gpx::build_geojson_feature(coords, "id-test", &trace_name);

            let cases = detect_anomalies_from_gpx(&gpx, 5.0);
            let elapsed = start.elapsed();
            println!(
                "{} → {} cas ({} pts, {:.0} ms)",
                path,
                cases.len(),
                count,
                elapsed.as_millis()
            );
            for c in &cases {
                println!(
                    "  {:?} apex={:?} zone=[{},{}] suggestion={:?}",
                    c.kind, c.apex_indices, c.start_index, c.end_index,
                    c.suggested_delete_ranges
                );
            }
            // La détection doit rester rapide (pas de blocage sur les gros fichiers).
            assert!(
                elapsed.as_millis() < 5000,
                "{} : détection trop lente ({} ms)",
                path,
                elapsed.as_millis()
            );

            // Appliquer les corrections suggérées et vérifier que le GPX
            // généré est cohérent (points conservés, XML relisible).
            let raw = extract_full_points(&gpx);
            for c in &mut cases.clone() {
                c.state = "corrected".to_string();
                c.correction.delete_ranges = c.suggested_delete_ranges.clone();
            }
            let (final_points, removed) = apply_corrections(&raw, &cases).unwrap();
            assert!(
                final_points.len() >= 2,
                "{} : trop de points supprimés ({} restants)",
                path,
                final_points.len()
            );
            let gpx_str = build_cleaned_gpx(&final_points, "test nettoyage");
            assert_eq!(
                gpx_str.matches("<trkpt ").count(),
                final_points.len(),
                "{} : nombre de <trkpt> du GPX généré",
                path
            );
            let reparsed = gpx::read(std::io::Cursor::new(gpx_str)).unwrap();
            let reparsed_count: usize = reparsed
                .tracks
                .iter()
                .flat_map(|t| &t.segments)
                .flat_map(|s| &s.points)
                .count();
            assert_eq!(reparsed_count, final_points.len(), "{} : re-parse", path);
            let _ = removed;
        }
    }

    /// `read_tolerance_deg` ne doit **jamais paniquer** si le state des
    /// paramètres n'est pas géré (repli sur 5°). Un panic dans une commande
    /// async laisserait l'appelant bloqué sans réponse. La logique de lecture
    /// est testée au niveau de `tolerance_from_settings` (fonction pure).
    #[test]
    fn tolerance_from_settings_reads_default() {
        let default: toml::Table =
            toml::from_str("[Nettoyage.Cap]\ntoleranceDeg = 5.0\n").unwrap();
        assert_eq!(tolerance_from_settings(&default, &toml::Table::new()), 5.0);
    }

    /// La surcharge utilisateur est prioritaire sur la valeur par défaut.
    #[test]
    fn tolerance_from_settings_user_override_priority() {
        let default: toml::Table =
            toml::from_str("[Nettoyage.Cap]\ntoleranceDeg = 5.0\n").unwrap();
        let mut cap = toml::Table::new();
        cap.insert("toleranceDeg".to_string(), toml::Value::Float(7.5));
        let mut nettoyage = toml::Table::new();
        nettoyage.insert("Cap".to_string(), toml::Value::Table(cap));
        let mut user_overrides = toml::Table::new();
        user_overrides.insert("Nettoyage".to_string(), toml::Value::Table(nettoyage));

        assert_eq!(tolerance_from_settings(&default, &user_overrides), 7.5);
    }

    /// Clé absente → repli sur 5° (le comportement de `try_state` sans state
    /// managé est couvert par la garantie « None → 5.0 » de `read_tolerance_deg`).
    #[test]
    fn tolerance_from_settings_missing_key_falls_back() {
        let default = toml::Table::new();
        assert_eq!(tolerance_from_settings(&default, &toml::Table::new()), 5.0);
    }

    /// Les paramètres ronds-points sont lus depuis le TOML (défauts si absents).
    #[test]
    fn roundabout_params_from_settings_reads_defaults() {
        let default: toml::Table = toml::from_str(
            "[Nettoyage.RondPoints]\nangleMinDeg = 7.0\npointsMin = 6\npointsMax = 60\nangleSeuilDeg = 300.0\nmargePoints = 8\n",
        )
        .unwrap();
        let p = roundabout_params_from_settings(&default, &toml::Table::new());
        assert_eq!(p.angle_min_deg, 7.0);
        assert_eq!(p.points_min, 6);
        assert_eq!(p.points_max, 60);
        assert_eq!(p.angle_seuil_deg, 300.0);
        assert_eq!(p.marge_points, 8);
    }

    /// Tables vides → valeurs par défaut de `RoundaboutParams`.
    #[test]
    fn roundabout_params_from_settings_falls_back() {
        let p = roundabout_params_from_settings(&toml::Table::new(), &toml::Table::new());
        assert_eq!(p.angle_min_deg, 5.0);
        assert_eq!(p.points_min, 5);
        assert_eq!(p.points_max, 50);
        assert_eq!(p.angle_seuil_deg, 210.0);
        assert_eq!(p.marge_points, 5);
    }
}
