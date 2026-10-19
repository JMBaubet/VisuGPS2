//! Tests de lecture des paramètres `Altimetrie.*`.

use crate::gpx_altimetrie::params::{
    altimetrie_params_from_settings, default_altimetrie_params,
};
use crate::gpx_altimetrie::types::AltimetrieParams;

#[test]
fn defaults_match_the_prototype() {
    let params = default_altimetrie_params();
    assert_eq!(params, AltimetrieParams::default());
    assert_eq!(params.pas_echantillonnage, 50.0);
    assert_eq!(params.correction_plateaux, 6);
    assert_eq!(params.fenetre_altitude, 500.0);
    assert_eq!(params.seuil_montee, 0.5);
    assert_eq!(params.ecart_fusion, 250.0);
    assert_eq!(params.longueur_min, 200.0);
    assert!(params.lissage_renforce);
}

#[test]
fn user_overrides_take_precedence() {
    let default_toml: toml::Table = toml::from_str(
        r#"
        [Altimetrie.Preparation]
        pasEchantillonnage = 50
        correctionPlateaux = 6
        fenetreAntiPic = 200
        seuilAntiPic = 15.0

        [Altimetrie.Lissage]
        fenetreAltitude = 500
        fenetrePente = 150
        sigmaGaussien = 75.0
        fenetreMediane = 0
        lissageRenforce = true

        [Altimetrie.Detection]
        seuilMontee = 0.5
        ecartFusion = 250
        rechercheExtrema = 60

        [Altimetrie.Segments]
        fenetreCalcul = 100
        longueurMin = 200
        "#,
    )
    .expect("TOML de défauts");

    let overrides: toml::Table = toml::from_str(
        r#"
        [Altimetrie.Preparation]
        pasEchantillonnage = 25
        correctionPlateaux = 0
        seuilAntiPic = 0.0

        [Altimetrie.Lissage]
        fenetreAltitude = 750
        lissageRenforce = false

        [Altimetrie.Detection]
        seuilMontee = 1.2
        ecartFusion = 0

        [Altimetrie.Segments]
        longueurMin = 0
        "#,
    )
    .expect("TOML de surcharges");

    let params = altimetrie_params_from_settings(&default_toml, &overrides);

    assert_eq!(params.pas_echantillonnage, 25.0);
    assert_eq!(params.correction_plateaux, 0);
    assert_eq!(params.seuil_anti_pic, 0.0);
    assert_eq!(params.fenetre_altitude, 750.0);
    assert!(!params.lissage_renforce);
    assert_eq!(params.seuil_montee, 1.2);
    assert_eq!(params.ecart_fusion, 0.0);
    assert_eq!(params.longueur_min, 0.0);
    // Valeurs non surchargées : le schéma par défaut fait foi.
    assert_eq!(params.fenetre_anti_pic, 200.0);
    assert_eq!(params.fenetre_pente, 150.0);
    assert_eq!(params.sigma_gaussien, 75.0);
    assert_eq!(params.fenetre_calcul, 100.0);
}

#[test]
fn missing_paths_fall_back_to_defaults() {
    let default_toml: toml::Table = toml::from_str(
        r#"
        [Altimetrie.Preparation]
        pasEchantillonnage = 40
        "#,
    )
    .expect("TOML partiel");
    let overrides = toml::Table::new();

    let params = altimetrie_params_from_settings(&default_toml, &overrides);

    assert_eq!(params.pas_echantillonnage, 40.0);
    assert_eq!(params.correction_plateaux, 6);
    assert_eq!(params.fenetre_altitude, 500.0);
    assert!(params.lissage_renforce);
}
