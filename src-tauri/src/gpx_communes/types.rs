//! Types du module Communes.

/// Un passage dans une commune : portion **continue** de trace située dans une
/// même commune, entre un point d'entrée et un point de sortie.
///
/// C'est le contrat du fichier `traces/{trace_id}/communes.json` **et** celui de
/// la commande de détection : les clés restent en `snake_case` des deux côtés,
/// comme `TraceMetadata` (nomenclature de la spécification, miroir de
/// l'interface TS). Aucune géométrie n'y figure : les coordonnées d'entrée et de
/// sortie sont reconstituables depuis la trace et les kilomètres.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct CommunePassage {
    /// Nom de la commune (tag `name` de la relation OSM).
    pub commune: String,
    /// Numéro de ce passage dans la commune (1-based, ordre chronologique).
    pub passage: u32,
    /// Nombre total de passages dans cette commune (≥ 1).
    pub nb_passages_total: u32,
    /// Kilomètre d'entrée dans la commune, depuis le départ de la trace.
    pub km_entree: f64,
    /// Kilomètre de sortie de la commune.
    pub km_sortie: f64,
    /// Longueur du passage (`km_sortie - km_entree`), en kilomètres.
    pub longueur_km: f64,
}

/// Réglages de la détection (`Communes.Detection.*` de `settings.default.toml`).
///
/// Les valeurs reprennent celles du prototype validé (`features/communes/`) :
/// chaque seuil y est justifié, et la `documentation` du paramètre correspondant
/// décrit son effet ainsi que les faux positifs qu'il expose.
#[derive(Debug, Clone, PartialEq)]
pub struct CommuneParams {
    /// Niveau administratif OSM visé (`"8"` = communes en France, Espagne,
    /// Italie, Suisse).
    pub admin_level: String,
    /// Élargissement de la bbox de recherche Overpass (°, 0.01 ≈ 1 km).
    pub bbox_buffer_deg: f64,
    /// Tolérance de simplification de la trace (°, 0.00005 ≈ 5 m).
    pub simplify_tolerance: f64,
    /// Nombre cible d'échantillons le long de la trace.
    pub target_samples: usize,
    /// Pas minimal entre deux échantillons (km, 0.1 = 100 m).
    pub min_sample_step_km: f64,
    /// Facteur d'exclusion des polygones aberrants : surface > ratio × médiane.
    pub max_area_ratio: f64,
    /// Longueur minimale d'un passage retenu (km, 0.015 = 15 m).
    pub min_segment_km: f64,
    /// Longueur en deçà de laquelle un passage est absorbé dans son voisin le
    /// plus long (km, 0.5 = 500 m).
    pub min_keep_km: f64,
}

impl Default for CommuneParams {
    /// Valeurs par défaut, miroir de `settings.default.toml`.
    fn default() -> Self {
        Self {
            admin_level: "8".to_string(),
            bbox_buffer_deg: 0.01,
            simplify_tolerance: 0.00005,
            target_samples: 2000,
            min_sample_step_km: 0.1,
            max_area_ratio: 10.0,
            min_segment_km: 0.015,
            min_keep_km: 0.5,
        }
    }
}

/// Nombre maximal d'échantillons consécutifs sans commune pouvant être comblés.
///
/// Constante interne (non exposée en paramètre) : le comblement ne s'applique
/// qu'à un trou d'**un** échantillon flanqué de la même commune des deux côtés —
/// un artefact de précision de frontière, jamais une vraie transition.
pub const MAX_GAP_FILL_SAMPLES: usize = 1;
