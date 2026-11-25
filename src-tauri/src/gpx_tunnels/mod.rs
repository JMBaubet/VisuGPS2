//! Module Tunnels.
//!
//! Il produit et maintient `traces/{trace_id}/tunnels.json` : les tunnels OSM
//! empruntés par une trace propre, puis les éditions de l'opérateur (création,
//! suppression, fusion/séparation). Le module suit le découpage des modules
//! `gpx_altimetrie` et `gpx_communes` :
//! - `types` — contrats IPC et fichier ;
//! - `params` — lecture des réglages `Tunnels.*` ;
//! - `geometry` — projections et calculs purs, sans dépendance Tauri ;
//! - `file` — lecture/écriture atomique et tolérante ;
//! - `hermes` — conversion et commit de la détection par le serveur (moteur
//!   nominal) ;
//! - `pbf`, `regions`, `download`, `pipeline`, `commands` — acquisition OSM et
//!   matching **locaux** (moteur dormant depuis l'intégration de hermes).

pub mod commands;
pub mod download;
pub mod file;
pub mod geometry;
pub mod hermes;
pub mod params;
pub mod pbf;
pub mod pipeline;
pub mod regions;
pub mod types;

#[cfg(test)]
mod tests;
