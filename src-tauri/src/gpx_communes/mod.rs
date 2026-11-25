//! Module Communes — détection des communes traversées par une trace GPX.
//!
//! Une trace nettoyée (`audit_status = "clean"`) est croisée avec les limites
//! administratives communales d'OpenStreetMap (`admin_level = 8`) : le module
//! produit la liste ordonnée des passages — commune, kilomètres d'entrée et de
//! sortie, longueur — et la persiste dans un fichier **unique** par trace,
//! `traces/{trace_id}/communes.json` (voir `file`).
//!
//! L'état de la fonctionnalité vit dans le registre des traces
//! (`traces.json`, champ `communes_completed`) : il est reposé à `false` par
//! `audit_validate` — le GPX réécrit change la géométrie de la trace, donc
//! invalide la description des communes — et à `true` par la détection, une
//! fois le fichier écrit.
//!
//! Le moteur nominal est désormais le serveur **hermes** : `POST
//! /communes/detect` calcule les passages, et le module ne garde que la
//! conversion de la réponse, le contrôle de couverture et la persistance (voir
//! `hermes`). Le moteur **Overpass** historique reste en place, dormant, avec
//! son portage du prototype validé (`features/communes/`) :
//! `osm` (relations OSM → polygones nommés) → `geometry` (primitives pures) →
//! `pipeline` (l'algorithme) → `file` (persistance) → `commands` (la commande).
//! `geometry` sert aux deux chemins, ainsi qu'au calcul de surface des secteurs
//! hermes.

pub mod commands;
pub mod file;
pub mod geometry;
pub mod hermes;
pub mod osm;
pub mod pipeline;
pub mod types;

#[cfg(test)]
mod tests;
