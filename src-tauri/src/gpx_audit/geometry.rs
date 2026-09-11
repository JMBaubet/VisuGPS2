//! Projection équirectangulaire locale + construction de la géométrie
//! métrique d'une trace (spécification ANALYSE §2.2 / §2.4).

use super::types::AuditPoint;

/// Projection équirectangulaire locale centrée sur le premier point P₀.
///
/// `fwd`/`inv` sont cohérents (inv ∘ fwd = id). La compensation en distance
/// par `kx = cos(lat₀)·111320` rend les distances fiables ; les angles
/// restent corrects.
pub struct Projector {
    lat0: f64,
    lon0: f64,
    kx: f64, // mètres par degré de longitude (compensé en cos(lat₀))
    ky: f64, // mètres par degré de latitude
}

impl Projector {
    pub fn new(p0: &AuditPoint) -> Self {
        let kx = (p0.lat * std::f64::consts::PI / 180.0).cos() * 111320.0;
        let ky = 110540.0;
        Self {
            lat0: p0.lat,
            lon0: p0.lon,
            kx,
            ky,
        }
    }

    /// Projection avant : coordonnées géographiques → (x, y) métriques.
    pub fn fwd(&self, p: &AuditPoint) -> (f64, f64) {
        let x = (p.lon - self.lon0) * self.kx;
        let y = (p.lat - self.lat0) * self.ky;
        (x, y)
    }

    /// Projection inverse : (x, y) métriques → coordonnées géographiques.
    pub fn inv(&self, x: f64, y: f64) -> (f64, f64) {
        let lat = self.lat0 + y / self.ky;
        let lon = self.lon0 + x / self.kx;
        (lat, lon)
    }
}

/// Géométrie métrique d'une trace — entrée des détecteurs AR/RP (§2.4).
pub struct Geometry {
    pub px: Vec<f64>,  // x métrique de chaque point
    pub py: Vec<f64>,  // y métrique de chaque point
    pub cum: Vec<f64>, // distance cumulée depuis P₀ (m) ; cum[0] = 0
    pub total: f64,    // cum[m−1] — longueur totale (m)
    pub ids: Vec<u32>, // identifiant stable de chaque point
}

/// Construit la géométrie métrique d'une trace (spec ANALYSE §2.4).
pub fn build_geometry(points: &[AuditPoint]) -> Geometry {
    let m = points.len();
    if m == 0 {
        return Geometry {
            px: Vec::new(),
            py: Vec::new(),
            cum: Vec::new(),
            total: 0.0,
            ids: Vec::new(),
        };
    }

    let proj = Projector::new(&points[0]);
    let mut px = Vec::with_capacity(m);
    let mut py = Vec::with_capacity(m);
    let mut ids = Vec::with_capacity(m);
    for p in points {
        let (x, y) = proj.fwd(p);
        px.push(x);
        py.push(y);
        ids.push(p.id);
    }

    let mut cum = vec![0.0; m];
    for i in 1..m {
        cum[i] = cum[i - 1] + (px[i] - px[i - 1]).hypot(py[i] - py[i - 1]);
    }
    let total = cum[m - 1];

    Geometry {
        px,
        py,
        cum,
        total,
        ids,
    }
}
