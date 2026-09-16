//! Projection équirectangulaire locale et géométrie métrique d'une trace.
//!
//! Première étape du pipeline de la spécification : les coordonnées
//! géographiques sont converties en mètres dans un repère local centré sur le
//! premier point, et la trace est débarrassée de ses **doublons** (points
//! séparés de moins de 5 cm du dernier point conservé).
//!
//! La géométrie conserve la **provenance** de chaque point (`raw_index`) : c'est
//! elle qui permet de rendre les index du GPX d'origine au bout de la chaîne
//! (cf. `resample::raw_point_number`), seuls exploitables par un consommateur du
//! fichier de description.

use super::types::MultiridePoint;

/// Seuil de dédoublonnage de la spécification : (0,05 m)².
pub const DUP_THRESHOLD2: f64 = 0.0025;

/// Projection équirectangulaire locale centrée sur le premier point P₀.
///
/// `fwd` et `inv` sont cohérents (`inv ∘ fwd = id`). La projection est
/// **affine et séparable** — `x` ne dépend que de la longitude, `y` que de la
/// latitude — ce dont le rééchantillonnage tire parti : interpoler en métrique
/// puis projeter en retour donne exactement le résultat d'une interpolation en
/// degrés.
#[derive(Debug)]
pub struct Projector {
    lat0: f64,
    lon0: f64,
    /// Mètres par degré de longitude, compensés en `cos(lat₀)`.
    kx: f64,
    /// Mètres par degré de latitude.
    ky: f64,
}

impl Projector {
    pub fn new(p0: &MultiridePoint) -> Self {
        let kx = (p0.lat * std::f64::consts::PI / 180.0).cos() * 111320.0;
        Self {
            lat0: p0.lat,
            lon0: p0.lon,
            kx,
            ky: 110540.0,
        }
    }

    /// Projection avant : coordonnées géographiques → (x, y) métriques.
    pub fn fwd(&self, p: &MultiridePoint) -> (f64, f64) {
        (
            (p.lon - self.lon0) * self.kx,
            (p.lat - self.lat0) * self.ky,
        )
    }

    /// Projection inverse : (x, y) métriques → coordonnées géographiques.
    pub fn inv(&self, x: f64, y: f64) -> (f64, f64) {
        (self.lat0 + y / self.ky, self.lon0 + x / self.kx)
    }
}

/// Géométrie métrique d'une trace, après dédoublonnage.
#[derive(Debug)]
pub struct MultirideGeom {
    /// Projection du repère local.
    pub proj: Projector,
    /// Coordonnées métriques des points conservés.
    pub px: Vec<f64>,
    pub py: Vec<f64>,
    /// Distance cumulée depuis P₀ (m) ; `cum[0] = 0`.
    pub cum: Vec<f64>,
    /// Longueur totale de la trace nettoyée (m).
    pub total: f64,
    /// Provenance de chaque point conservé : index (0-based) dans les points
    /// **bruts** fournis. Un point écarté au dédoublonnage ne figure donc pas
    /// ici, et le point conservé porte l'index de la **première** occurrence.
    ///
    /// Lue par `resample::raw_point_number`, que l'assemblage des segments
    /// appellera pour convertir ses bornes en numéros de points GPX — d'où le
    /// marqueur temporaire, à retirer quand le pipeline la lira.
    #[allow(dead_code)]
    pub raw_index: Vec<usize>,
}

/// Construit la géométrie métrique d'une trace.
///
/// Retourne une erreur si la trace est dégénérée : moins de deux points fournis,
/// ou moins de deux points distincts après dédoublonnage (une trace dont tous
/// les points sont confondus n'a ni longueur ni géométrie).
pub fn build_geometry(points: &[MultiridePoint]) -> Result<MultirideGeom, String> {
    if points.len() < 2 {
        return Err("Trace dégénérée : moins de 2 points.".to_string());
    }

    let proj = Projector::new(&points[0]);
    let mut px = Vec::with_capacity(points.len());
    let mut py = Vec::with_capacity(points.len());
    let mut raw_index = Vec::with_capacity(points.len());

    for (index, p) in points.iter().enumerate() {
        let (x, y) = proj.fwd(p);
        // Dédoublonnage : un point à moins de 5 cm du dernier point conservé est
        // ignoré (documentation technique §3.2).
        if let (Some(&last_x), Some(&last_y)) = (px.last(), py.last()) {
            let dx = x - last_x;
            let dy = y - last_y;
            if dx * dx + dy * dy < DUP_THRESHOLD2 {
                continue;
            }
        }
        px.push(x);
        py.push(y);
        raw_index.push(index);
    }

    if px.len() < 2 {
        return Err("Trace dégénérée : moins de 2 points distincts.".to_string());
    }

    let mut cum = vec![0.0; px.len()];
    for i in 1..px.len() {
        cum[i] = cum[i - 1] + (px[i] - px[i - 1]).hypot(py[i] - py[i - 1]);
    }
    let total = cum[px.len() - 1];

    Ok(MultirideGeom {
        proj,
        px,
        py,
        cum,
        total,
        raw_index,
    })
}
