//! Module Audit GPX — détection d'anomalies de trace (AR et RP).
//!
//! Remplace l'ancien module de nettoyage. Portage fidèle de l'application HTML
//! de référence (`docs/audit/reference/verifgpx-V3.0.html`).

pub mod anchor;
pub mod ar;
pub mod commands;
pub mod consolidation;
pub mod corrections;
pub mod export;
pub mod geometry;
// D1 (registre obsolète) et D2c (fichiers de travail hérités) : câblés depuis
// `import_gpx.rs` (`load_registry` et `get_mode_dir`).
pub mod migration;
pub mod overlay;
pub mod pipeline;
pub mod preview;
pub mod rp;
pub mod routing;
pub mod types;

#[cfg(test)]
mod tests;
