//! Consolidation de la trace (spec ANALYSE §2.3) : suppression des points
//! redondants à moins de σ du dernier point conservé.

use super::geometry::Projector;
use super::types::AuditPoint;

/// Règle glissante cumulative : un point est supprimé si sa distance au
/// **dernier point conservé** est strictement inférieure à `threshold`.
/// Le premier point est toujours conservé. Retourne les points conservés
/// et le nombre de points supprimés. `threshold = 0` désactive la consolidation.
pub fn consolidate_points(raw: &[AuditPoint], threshold: f64) -> (Vec<AuditPoint>, usize) {
    let mut kept: Vec<AuditPoint> = Vec::new();
    if raw.is_empty() {
        return (kept, 0);
    }

    let proj = Projector::new(&raw[0]);
    // fwd(raw[0]) vaut (0, 0) : référence initiale du dernier point conservé.
    let (mut ax, mut ay) = proj.fwd(&raw[0]);
    let mut removed = 0usize;

    for (i, p) in raw.iter().enumerate() {
        let (qx, qy) = proj.fwd(p);
        if i > 0 && (qx - ax).hypot(qy - ay) < threshold {
            removed += 1;
            continue;
        }
        kept.push(p.clone());
        ax = qx;
        ay = qy;
    }

    (kept, removed)
}
