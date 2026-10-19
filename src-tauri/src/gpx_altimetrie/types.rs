//! Types du module Altimétrie.
//!
//! Ils sont sérialisés en **camelCase** : le fichier `altimetrie.json` est un
//! contrat de sortie consommé par la vue, et les commandes Tauri suivent la
//! convention IPC du reste de l'application.
//!
//! Le fichier est à la fois **état restaurable** et sortie d'analyse :
//! - `profile` porte le profil affiché (altitude lissée ancrée) et les
//!   coordonnées interpolées, pour dessiner le graphe et recentrer la carte ;
//! - `segments` porte la segmentation par catégorie de pente ;
//! - `climbs` porte les montées, leurs métriques et l'état d'édition
//!   (fusion/séparation). Les groupes séparés sont stockés membre par membre
//!   avec un `groupId` commun, afin qu'une réouverture de la vue puisse les
//!   reconstituer — contrairement à l'export consolidé du prototype.

use serde::{Deserialize, Serialize};

/// Version du format du fichier. Une version inconnue est ignorée par
/// `file::load_file`, qui laisse l'appelant relancer la détection.
pub const FILE_VERSION: u32 = 1;

// ─── Paramètres ───────────────────────────────────────────────────────

/// Paramètres du pipeline altimétrique. Leurs chemins de configuration sont
/// `Altimetrie.*` dans `settings.default.toml` ; les défauts ci-dessous sont le
/// miroir exact du prototype validé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieParams {
    /// Pas de rééchantillonnage (m).
    pub pas_echantillonnage: f64,
    /// Longueur minimale d'une série plate corrigée (points) ; < 2 désactive.
    pub correction_plateaux: i32,
    /// Fenêtre de l'anti-pic Hampel (m).
    pub fenetre_anti_pic: f64,
    /// Seuil de l'anti-pic Hampel (m) ; 0 désactive.
    pub seuil_anti_pic: f64,
    /// Fenêtre du lissage d'altitude (m).
    pub fenetre_altitude: f64,
    /// Fenêtre du lissage de pente (m).
    pub fenetre_pente: f64,
    /// Sigma du filtre gaussien (m) ; 0 désactive.
    pub sigma_gaussien: f64,
    /// Fenêtre du filtre médian (m) ; 0 désactive.
    pub fenetre_mediane: f64,
    /// Active le second passage Savitzky-Golay du lissage ancré.
    pub lissage_renforce: bool,
    /// Seuil d'hystérésis de détection des montées (%).
    pub seuil_montee: f64,
    /// Écart maximal entre deux montées avant fusion automatique (m) ;
    /// 0 désactive la fusion.
    pub ecart_fusion: f64,
    /// Rayon de recherche des extrema, en pourcentage de la longueur candidate.
    pub recherche_extrema: f64,
    /// Fenêtre de calcul de la pente pour la catégorisation (m).
    pub fenetre_calcul: f64,
    /// Longueur minimale d'un segment de catégorie (m) ; 0 désactive la fusion
    /// des micro-segments.
    pub longueur_min: f64,
}

impl Default for AltimetrieParams {
    fn default() -> Self {
        Self {
            pas_echantillonnage: 50.0,
            correction_plateaux: 6,
            fenetre_anti_pic: 200.0,
            seuil_anti_pic: 15.0,
            fenetre_altitude: 500.0,
            fenetre_pente: 150.0,
            sigma_gaussien: 75.0,
            fenetre_mediane: 0.0,
            lissage_renforce: true,
            seuil_montee: 0.5,
            ecart_fusion: 250.0,
            recherche_extrema: 60.0,
            fenetre_calcul: 100.0,
            longueur_min: 200.0,
        }
    }
}

// ─── Fichier altimetrie.json ──────────────────────────────────────────

/// Racine du fichier `traces/{trace_id}/altimetrie.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieFile {
    pub version: u32,
    pub trace_id: String,
    /// Nom du fichier GPX d'origine.
    pub source: String,
    /// Date de la dernière écriture, ISO 8601.
    pub updated_at: String,
    pub params: AltimetrieParams,
    pub trace: AltimetrieTrace,
    /// Profil affiché, échantillonné au pas `params.pas_echantillonnage`.
    pub profile: Vec<AltimetrieProfilePoint>,
    /// Segments de pente couvrant la trace échantillonnée.
    pub segments: Vec<AltimetrieSegment>,
    /// Montées, triées par `startKm` croissant.
    pub climbs: Vec<AltimetrieClimb>,
    pub legend: AltimetrieLegend,
}

/// Contexte de la trace dans le fichier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieTrace {
    /// Longueur réelle de la trace (m, distance 2D cumulée des points bruts).
    pub total_distance_m: f64,
    /// Longueur couverte par la grille rééchantillonnée (m).
    pub total_distance_sampled_m: f64,
    /// Distance résiduelle non couverte par la grille (m).
    pub trailing_gap_m: f64,
    /// Pas de rééchantillonnage effectif (m).
    pub step_m: f64,
    /// Nombre de points du profil.
    pub total_points: usize,
}

/// Point du profil affiché. `d` est la distance cumulée sur la grille (m),
/// `ele` l'altitude lissée ancrée (m), `lat`/`lon` la position interpolée sur
/// la trace brute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieProfilePoint {
    pub d: f64,
    pub ele: f64,
    pub lat: f64,
    pub lon: f64,
}

/// Segment de catégorie de pente. Les bornes sont exprimées en kilomètres et en
/// fraction de la longueur réelle (0–1), comme dans l'export du prototype.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieSegment {
    pub index: usize,
    pub start_km: f64,
    pub end_km: f64,
    pub start_progress: f64,
    pub end_progress: f64,
    pub category_id: String,
    pub avg_slope_pct: f64,
    pub d_plus_m: f64,
}

/// Montée détectée ou créée manuellement.
///
/// - montée simple : `isMerged=false`, `isSeparated=false`, `groupId=None`,
///   `mergedParts=None` ;
/// - montée fusionnée non séparée : `isMerged=true`, `mergedParts` renseigné ;
/// - membre d'un groupe séparé : `isSeparated=true`, `groupId` partagé,
///   `isMerged=false`, `mergedParts=None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieClimb {
    /// Identifiant stable pour la sélection dans l'IHM.
    pub id: u32,
    /// Rang après tri par `startKm` (0-based).
    pub index: usize,
    pub start_km: f64,
    pub end_km: f64,
    pub start_progress: f64,
    pub end_progress: f64,
    pub d_plus_m: f64,
    pub avg_slope_pct: f64,
    pub alt_start_m: f64,
    pub alt_end_m: f64,
    /// Nombre de fusions automatiques ayant contribué (0 pour une montée
    /// simple ou manuelle).
    pub merged_count: u32,
    /// Les bornes ont été repliées sur un sous-segment valide.
    pub fallback: bool,
    pub is_merged: bool,
    pub is_separated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_parts: Option<Vec<AltimetrieMergedPart>>,
}

/// Sous-montée d'une montée fusionnée non séparée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieMergedPart {
    pub part_index: usize,
    pub start_km: f64,
    pub end_km: f64,
    pub start_progress: f64,
    pub end_progress: f64,
    pub d_plus_m: f64,
    pub avg_slope_pct: f64,
    pub alt_start_m: f64,
    pub alt_end_m: f64,
}

/// Légende des catégories de pente. Les bornes hautes sont exclusives ; `None`
/// représente l'infini (négatif pour `minPct`, positif pour `maxPct`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieLegend {
    pub note: String,
    pub fenetre_calcul: f64,
    pub longueur_min: f64,
    pub categories: Vec<AltimetrieCategoryDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieCategoryDef {
    pub id: String,
    pub label: String,
    pub min_pct: Option<f64>,
    pub max_pct: Option<f64>,
}

// ─── Entrée d'édition ─────────────────────────────────────────────────

/// Forme éditable d'une montée, envoyée par le frontend à
/// `altimetrie_preview` et `altimetrie_save`. Le backend recalcule toutes les
/// métriques et les bornes résolues ; il ne fait jamais confiance aux valeurs
/// dérivées.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieClimbEdit {
    pub id: u32,
    pub start_km: f64,
    pub end_km: f64,
    #[serde(default)]
    pub is_merged: bool,
    #[serde(default)]
    pub is_separated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_parts: Option<Vec<AltimetrieMergedPartEdit>>,
    #[serde(default)]
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AltimetrieMergedPartEdit {
    pub start_km: f64,
    pub end_km: f64,
}
