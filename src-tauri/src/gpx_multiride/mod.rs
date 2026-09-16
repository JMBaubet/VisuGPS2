//! Module Multiride — détection des portions de trace parcourues plusieurs fois.
//!
//! Une trace GPX valide (auditée) peut contenir des portions empruntées
//! plusieurs fois : aller-retour sur un même tronçon, reconnaissance repassant
//! sur une section, boucle locale reprenant un chemin. Le module les détecte,
//! les qualifie par un sens (référence / aller / retour) et les persiste dans
//! un fichier de description (`traces/{trace_id}/multiride.json`).
//!
//! Ce fichier a trois usages :
//! - **état de travail** de la vue `/multiride` (ajustements compris :
//!   fusion de segments, marquage faux positif) ;
//! - **contrat de sortie** pour la Visualisation, à la nomenclature de la
//!   spécification ;
//! - **verrou** : tant que les passages détectés ne sont pas validés, l'édition
//!   caméra reste inaccessible (`multiride_status = "pending"`), au même titre
//!   qu'une trace non auditée.
//!
//! Portage du module « Multi-Sens » de la spécification, **dont l'archive de
//! conception a été retirée** (comme celle du module Audit) : cette
//! implémentation fait foi, et la `documentation` des paramètres
//! `Multiride.Detection.*` de `settings.default.toml` décrit l'effet de chaque
//! seuil ainsi que les faux positifs et faux négatifs qu'il expose.
//!
//! Pipeline : `detection` (lecture du GPX) → `projection` (géométrie métrique,
//! dédoublonnage) → `resample` (pas quasi constant) → `runs` (correspondances et
//! chaînage) → `segments` (assemblage) → `direction` (sens de chaque passage).

pub mod adjustments;
pub mod commands;
pub mod detection;
pub mod direction;
pub mod file;
pub mod projection;
pub mod resample;
pub mod runs;
pub mod segments;
pub mod types;

#[cfg(test)]
mod tests;
