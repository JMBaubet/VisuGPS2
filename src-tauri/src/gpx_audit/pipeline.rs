//! Orchestration de la détection (Livrable 4 §2.1).
//!
//! Ce module rassemble les étapes qui, dans le HTML de référence, sont
//! enchaînées par `runAudit()` : chargement du GPX, consolidation, construction
//! de la géométrie métrique, détection AR, détection RP, fusion et
//! renumérotation des findings.
//!
//! Il est **volontairement séparé de `commands.rs`** : il ne dépend ni de Tauri
//! ni d'un `AppHandle`, ce qui le rend testable directement (Livrable 4 §6).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use regex::Regex;

use super::ar::detect_ar;
use super::consolidation::consolidate_points;
use super::geometry::build_geometry;
use super::rp::detect_rp;
use super::types::{AuditParams, AuditPoint, Finding, FindingKind};

/// Nombre minimal de points après consolidation (Livrable 4 §2.1, gestion
/// d'erreurs) : en deçà, la trace est dégénérée et aucune anomalie n'a de sens.
pub const MIN_POINTS: usize = 5;

/// Résultat de l'orchestration : la trace de travail consolidée, sa longueur et
/// les findings fusionnés/renumérotés.
pub struct DetectionOutcome {
    /// Trace consolidée, identifiants stables 1..N.
    pub points: Vec<AuditPoint>,
    /// Longueur totale de la trace consolidée (m).
    pub total_distance_m: f64,
    /// Findings AR puis RP, triés par `parts[0].s` croissant, renumérotés.
    pub findings: Vec<Finding>,
}

/// Contexte source du GPX, préservé par `export::rewrite_gpx` (IHM §20.1).
#[derive(Debug, Clone, Default)]
pub struct SourceGpxContext {
    /// Attributs de la balise `<gpx>` du source.
    pub attrs: Option<String>,
    /// Élément `<metadata>` du source, sérialisé, **balise ouvrante incluse**.
    pub meta_xml: Option<String>,
    /// `<name>` de la première trace du source.
    pub trk_name: Option<String>,
}

// ─── Chargement ───────────────────────────────────────────────────────

/// Extrait les points bruts d'un document GPX déjà parsé (toutes traces,
/// tous segments, dans l'ordre du fichier).
pub fn raw_points_from_gpx(gpx: &gpx::Gpx) -> Vec<AuditPoint> {
    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let p = wp.point();
                points.push(AuditPoint {
                    // Identifiant provisoire : réaffecté après consolidation.
                    id: points.len() as u32,
                    lat: p.y(),
                    lon: p.x(),
                    ele: wp.elevation,
                });
            }
        }
    }
    points
}

/// Lit un fichier GPX et retourne ses points bruts.
pub fn load_gpx_points(gpx_path: &Path) -> Result<Vec<AuditPoint>, String> {
    if !gpx_path.exists() {
        return Err(format!("Fichier GPX introuvable : {}.", gpx_path.display()));
    }
    let file = File::open(gpx_path)
        .map_err(|e| format!("Ouverture du GPX ({}): {}", gpx_path.display(), e))?;
    let gpx = gpx::read(BufReader::new(file))
        .map_err(|e| format!("GPX illisible ({}): {}", gpx_path.display(), e))?;
    Ok(raw_points_from_gpx(&gpx))
}

// ─── Détection ────────────────────────────────────────────────────────

/// Exécute la chaîne complète de détection sur des points bruts.
///
/// Étapes (Livrable 4 §2.1) : consolidation → identifiants stables 1..N →
/// géométrie métrique → `detect_ar` → `detect_rp` → fusion → tri par
/// `parts[0].s` → renumérotation par famille.
///
/// Note : le rééchantillonnage RP (`resample_geo`, étape 6 du livrable) est
/// déjà interne à `detect_rp` (portage de `detectRP`, scénario §17.2) — il
/// n'est pas rejoué ici.
pub fn detect_all(
    raw: Vec<AuditPoint>,
    params: &AuditParams,
) -> Result<DetectionOutcome, String> {
    // Consolidation : supprime les points redondants à moins de σ.
    let (kept, _removed) = consolidate_points(&raw, params.consol_m);

    // Identifiants stables séquentiels 1..N sur la trace consolidée.
    let mut points = kept;
    for (i, p) in points.iter_mut().enumerate() {
        p.id = (i + 1) as u32;
    }

    if points.len() < MIN_POINTS {
        return Err("Trace trop courte après consolidation.".to_string());
    }

    let geo = build_geometry(&points);

    let mut findings = detect_ar(&points, &geo.px, &geo.py, &geo.ids, params);
    findings.extend(detect_rp(
        &points, &geo.px, &geo.py, &geo.cum, &geo.ids, geo.total, params,
    ));

    // Tri stable par début de zone : l'ordre relatif au sein d'une même
    // famille (donc la numérotation produite par chaque détecteur) est
    // préservé.
    findings.sort_by_key(|f| f.parts.first().map(|p| p.s).unwrap_or(0));

    renumber_findings(&mut findings);

    Ok(DetectionOutcome {
        points,
        total_distance_m: geo.total,
        findings,
    })
}

/// Renumérote les findings par famille (Livrable 4 §2.1, étape 10).
///
/// Chaque famille (« Aller-retour », « Boucle giratoire », « Tour de
/// rond-point ») possède son propre compteur ; le libellé nu reçoit le suffixe
/// `" : n"` et l'identifiant le préfixe `ar-` / `rp-`. Les détecteurs
/// numérotent déjà par famille : la passe est donc idempotente, mais elle
/// garantit le contrat quel que soit l'ordre de fusion.
pub fn renumber_findings(findings: &mut [Finding]) {
    let mut ar_n = 0usize;
    let mut rp_n = 0usize;
    for f in findings.iter_mut() {
        let bare = bare_label(&f.label).to_string();
        match f.kind {
            FindingKind::Ar => {
                ar_n += 1;
                f.id = format!("ar-{}", ar_n);
                f.label = format!("{} : {}", bare, ar_n);
            }
            FindingKind::Rp => {
                rp_n += 1;
                f.id = format!("rp-{}", rp_n);
                f.label = format!("{} : {}", bare, rp_n);
            }
        }
    }
}

/// Retire le suffixe de numérotation `" : n"` d'un libellé (le libellé nu sert
/// de clé de comptage par famille).
fn bare_label(label: &str) -> &str {
    match label.rsplit_once(" : ") {
        Some((bare, suffix)) if suffix.chars().all(|c| c.is_ascii_digit()) => bare,
        _ => label,
    }
}

// ─── Contexte source (réécriture GPX) ─────────────────────────────────

const RE_GPX_OPEN: &str = r"(?is)<gpx\b([^>]*)>";
const RE_METADATA: &str = r"(?is)<metadata\b.*?</metadata\s*>";

/// Relit l'entête du GPX **courant** pour la réémettre à l'identique lors de la
/// réécriture (IHM §20.1). Le nom de trace est lu via le parseur GPX (plus
/// robuste que le motif), les attributs et le `<metadata>` par motif.
pub fn read_source_context(gpx_path: &Path) -> Result<SourceGpxContext, String> {
    let raw = std::fs::read_to_string(gpx_path)
        .map_err(|e| format!("Lecture du GPX ({}): {}", gpx_path.display(), e))?;

    let attrs = re(RE_GPX_OPEN)?
        .captures(&raw)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty());

    let meta_xml = re(RE_METADATA)?
        .find(&raw)
        .map(|m| m.as_str().to_string());

    let trk_name = {
        let file = File::open(gpx_path)
            .map_err(|e| format!("Ouverture du GPX ({}): {}", gpx_path.display(), e))?;
        let gpx = gpx::read(BufReader::new(file))
            .map_err(|e| format!("GPX illisible ({}): {}", gpx_path.display(), e))?;
        gpx.tracks.first().and_then(|t| t.name.clone())
    };

    Ok(SourceGpxContext {
        attrs,
        meta_xml,
        trk_name,
    })
}

/// Compile un motif regex littéral. Un motif invalide serait une erreur de
/// programmation : elle est remontée en `Err` plutôt que de paniquer.
fn re(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("Motif interne invalide : {}", e))
}

/// Écriture atomique (fichier temporaire puis `rename`) — même garantie que
/// `import_gpx::save_registry` : un échec laisse le fichier précédent intact.
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    let mut tmp = path.to_path_buf();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Chemin invalide : {}.", path.display()))?;
    tmp.set_file_name(format!("{}.tmp", name));
    std::fs::write(&tmp, content)
        .map_err(|e| format!("Écriture du fichier temporaire ({}): {}", tmp.display(), e))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Renommage du fichier ({}): {}", path.display(), e));
    }
    Ok(())
}
