//! Module Altimétrie.
//!
//! Il produit et maintient `traces/{trace_id}/altimetrie.json` : profil lissé,
//! segments de pente et montées. Le module suit le découpage des modules
//! `gpx_audit` et `gpx_multiride` :
//! - `types` — contrats IPC et fichier ;
//! - `params` — lecture des réglages `Altimetrie.*` ;
//! - `pipeline` — algorithmes purs, sans dépendance Tauri ;
//! - `file` — lecture/écriture atomique et tolérante ;
//! - `commands` — adaptateurs Tauri et génération automatique.

pub mod file;
pub mod params;
pub mod types;

#[cfg(test)]
mod tests;
