//! Module Audit GPX — détection d'anomalies de trace (AR et RP).
//!
//! Remplace l'ancien module de nettoyage (`cleaning.rs`). Portage fidèle
//! de l'application HTML de référence (`docs/audit/reference/verifgpx-V3.0.html`).

pub mod anchor;
pub mod ar;
pub mod consolidation;
pub mod geometry;
pub mod rp;
pub mod types;

#[cfg(test)]
mod tests;
