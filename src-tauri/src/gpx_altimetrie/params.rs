//! Lecture des paramètres `Altimetrie.*`.
//!
//! Même découpage que `gpx_multiride::commands` : une fonction Tauri
//! (`read_altimetrie_params`) qui ne panique jamais, et une fonction pure
//! (`altimetrie_params_from_settings`) directement testable.

use std::sync::{Arc, RwLock};

use tauri::Manager;

use crate::settings::{get_toml_value_by_path, SettingsState};

use super::types::AltimetrieParams;

/// Lit les paramètres du pipeline (`Altimetrie.*`) avec repli sur les défauts.
///
/// Ne panique jamais (`try_state`) : l'import et la validation d'audit
/// s'appuient dessus, une défaillance du système de paramètres ne doit pas les
/// bloquer.
pub fn read_altimetrie_params(app: &tauri::AppHandle) -> AltimetrieParams {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return default_altimetrie_params(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return default_altimetrie_params(),
    };
    altimetrie_params_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Valeurs par défaut — miroir de la section `Altimetrie.*` de
/// `settings.default.toml` et des constantes du prototype validé.
pub fn default_altimetrie_params() -> AltimetrieParams {
    AltimetrieParams::default()
}

/// Résout les paramètres depuis les tables TOML : surcharge utilisateur
/// prioritaire, puis schéma par défaut, puis repli. Pure et testable.
pub fn altimetrie_params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> AltimetrieParams {
    let fallback = default_altimetrie_params();

    let number = |path: &str, default: f64| -> f64 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Float(v)) => *v,
            Some(toml::Value::Integer(v)) => *v as f64,
            _ => default,
        }
    };

    let boolean = |path: &str, default: bool| -> bool {
        get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path))
            .and_then(|value| value.as_bool())
            .unwrap_or(default)
    };

    AltimetrieParams {
        pas_echantillonnage: number(
            "Altimetrie.Preparation.pasEchantillonnage",
            fallback.pas_echantillonnage,
        ),
        correction_plateaux: number(
            "Altimetrie.Preparation.correctionPlateaux",
            fallback.correction_plateaux as f64,
        )
        .round() as i32,
        fenetre_anti_pic: number(
            "Altimetrie.Preparation.fenetreAntiPic",
            fallback.fenetre_anti_pic,
        ),
        seuil_anti_pic: number(
            "Altimetrie.Preparation.seuilAntiPic",
            fallback.seuil_anti_pic,
        ),
        fenetre_altitude: number(
            "Altimetrie.Lissage.fenetreAltitude",
            fallback.fenetre_altitude,
        ),
        fenetre_pente: number("Altimetrie.Lissage.fenetrePente", fallback.fenetre_pente),
        sigma_gaussien: number(
            "Altimetrie.Lissage.sigmaGaussien",
            fallback.sigma_gaussien,
        ),
        fenetre_mediane: number(
            "Altimetrie.Lissage.fenetreMediane",
            fallback.fenetre_mediane,
        ),
        lissage_renforce: boolean(
            "Altimetrie.Lissage.lissageRenforce",
            fallback.lissage_renforce,
        ),
        seuil_montee: number("Altimetrie.Detection.seuilMontee", fallback.seuil_montee),
        ecart_fusion: number("Altimetrie.Detection.ecartFusion", fallback.ecart_fusion),
        recherche_extrema: number(
            "Altimetrie.Detection.rechercheExtrema",
            fallback.recherche_extrema,
        ),
        fenetre_calcul: number(
            "Altimetrie.Segments.fenetreCalcul",
            fallback.fenetre_calcul,
        ),
        longueur_min: number("Altimetrie.Segments.longueurMin", fallback.longueur_min),
    }
}
