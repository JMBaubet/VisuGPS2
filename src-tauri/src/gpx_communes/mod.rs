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
//! Le réseau reste côté front (WebView) : la commande reçoit la réponse Overpass
//! brute (`osm`), et le backend se charge du reste. Le portage du prototype
//! validé (`features/communes/`) se répartit ainsi :
//! `osm` (relations OSM → polygones nommés) → `geometry` (primitives pures) →
//! `pipeline` (l'algorithme) → `file` (persistance) → `commands` (la commande).

pub mod commands;
pub mod file;
pub mod geometry;
pub mod osm;
pub mod pipeline;
pub mod types;

#[cfg(test)]
mod tests;
