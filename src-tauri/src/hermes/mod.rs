//! Client du serveur `hermes`.
//!
//! Le module couvre la **connexion** à `hermes-api` (port 9000) : état du
//! serveur, datasets, régions, polygones custom, instances ORS, artefacts
//! auxiliaires et réveil Wake-on-LAN. Les détections communes/tunnels sont
//! ajoutées aux sous-étapes suivantes.
//!
//! Découpage :
//! - `config` — lecture des paramètres `Hermes.*` et construction du client ;
//! - `types` — contrats IPC et codes d'erreur ;
//! - `client` — requêtes HTTP, reprises et mapping d'erreurs ;
//! - `wake` — paquet magique WoL et envoi UDP ;
//! - `commands` — les commandes Tauri.
//!
//! Le réseau vit côté Rust : `hermes-api` n'expose pas de middleware CORS et le
//! WoL exige un socket UDP. Aucun jeton administratif n'est stocké ni transmis.

pub mod client;
pub mod commands;
pub mod config;
pub mod types;
pub mod wake;

#[cfg(test)]
mod tests;
