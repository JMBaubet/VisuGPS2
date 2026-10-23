//! Gestion des données OSM partagées (tunnels, communes).
//!
//! Ce module est la couche de gestion des fichiers `.osm.pbf` et de
//! `regions.json` : inventaire, dates, états de fraîcheur, espace disque. Il
//! réutilise l'acquisition Geofabrik de `gpx_tunnels::regions` et
//! `gpx_tunnels::download` ; l'extraction complète de ces briques reste un
//! refactor séparé.
//!
//! - `types` — contrats IPC ;
//! - `params` — lecture des réglages `Osm.*` ;
//! - `inventory` — état des fichiers et de l'espace disque.

pub mod inventory;
pub mod params;
pub mod types;

#[cfg(test)]
mod tests;
