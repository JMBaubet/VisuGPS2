//! Module Audit GPX — détection d'anomalies de trace (AR et RP).
//!
//! Remplace l'ancien module de nettoyage (`cleaning.rs`). Portage fidèle
//! de l'application HTML de référence (`docs/audit/reference/verifgpx-V3.0.html`).

pub mod anchor;
pub mod ar;
pub mod commands;
pub mod consolidation;
pub mod corrections;
pub mod export;
pub mod geometry;
// D1 / D2c : câblage reporté à la Phase 5 (migration complète) — les fonctions
// sont prêtes mais aucun point d'appel ne les utilise encore.
#[allow(dead_code)]
pub mod migration;
pub mod overlay;
pub mod pipeline;
pub mod preview;
pub mod rp;
pub mod routing;
pub mod types;

#[cfg(test)]
mod tests;
