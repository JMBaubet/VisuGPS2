//! Réécriture du GPX après audit (CORRECTIONS §5.4, IHM §20).
//!
//! Le fichier exporté est **la trace de travail** (`working`) telle quelle
//! (invariant C10) : les points des anomalies marquées faux positif comme ceux
//! restés à traiter y figurent inchangés. L'entête du source n'est **pas**
//! reconstruite : les attributs de la balise `<gpx>`, le `<metadata>` et le
//! `<name>` de la trace sont réémis tels quels, seuls le `<desc>` et le bloc
//! d'audit `<extensions><audit>` étant (re)posés (§20.1).
//!
//! Deux garanties de sécurité : le backup `.orig` est posé **une seule fois** et
//! n'est jamais écrasé ; l'écriture est **atomique** (fichier temporaire puis
//! `rename`), de sorte qu'un échec laisse le GPX d'origine intact.

use std::fs;
use std::path::Path;

use regex::Regex;

use super::types::AuditPoint;

/// Namespace du bloc d'audit (IHM §20.2).
/// Reproduit **littéralement** la constante du JS (`GPX_AUDIT_NS`), accent
/// compris, bien que l'URI ne soit pas un nom de domaine valide.
const GPX_AUDIT_NS: &str = "http://VérificationGPX.example/gpx/audit/1";

/// Nom d'application de repli (`GPX_APP_DEFAULT`, IHM §20.4).
const GPX_APP_DEFAULT: &str = "VérificationGPX";

/// Attributs de repli de la balise `<gpx>` — cas pathologique d'une source sans
/// aucun attribut (§20.3).
const GPX_HEADER_ATTRS_TAIL: &str = "xmlns=\"http://www.topografix.com/GPX/1/1\"";

// Motifs de retrait/insertion dans le `<metadata>` du source (§20.1),
// transposés des expressions régulières du JS (`i` → `(?i)`, `[\s\S]` → `(?s).`).
const RE_DESC: &str = r"(?is)<desc\b.*?</desc\s*>";
const RE_EXTENSIONS: &str = r"(?is)<extensions\b.*?</extensions\s*>";
const RE_METADATA_OPEN: &str = r"(?i)^(<metadata\b[^>]*>)";
const RE_METADATA_CLOSE: &str = r"(?i)</metadata\s*>";

/// Compteurs portés par le bloc d'audit (IHM §20.2).
///
/// `total` n'est pas stocké : il est **dérivé** de la somme des trois compteurs
/// (`routes + deletions + falsePositives`), ce qui garantit le déterminisme du
/// bloc (scénario D12).
#[derive(Debug, Clone, Copy, Default)]
pub struct FindingsSummary {
    /// Findings `correction = "route-car"` ou `"route-bike"`.
    pub routes: usize,
    /// Findings `correction = "delete"`.
    pub deletions: usize,
    /// Findings `status = "fp"`.
    pub false_positives: usize,
}

/// Réécrit le GPX après audit (IHM §20.3).
///
/// - pose le backup `{nom}.gpx.orig` s'il est absent (jamais écrasé) ;
/// - réécrit le GPX avec les points corrigés (`points` = trace de travail) ;
/// - préserve l'entête source : `source_gpx_attrs` (attributs de `<gpx>`),
///   `source_meta_xml` (élément `<metadata>` sérialisé, **débutant par la balise
///   ouvrante**), `source_trk_name` (`<name>` de la trace) ;
/// - insère le bloc d'audit horodaté dans `<metadata><extensions>` ;
/// - écrit de façon atomique (`.tmp` puis `rename`).
///
/// `app_name` est le « Nom de l'application » (IHM §20.4) : valeur vidée ou
/// blanche → repli sur `GPX_APP_DEFAULT`.
pub fn rewrite_gpx(
    original_path: &Path,
    points: &[AuditPoint],
    source_meta_xml: Option<&str>,
    source_gpx_attrs: Option<&str>,
    source_trk_name: Option<&str>,
    app_name: &str,
    findings_summary: FindingsSummary,
) -> Result<(), String> {
    if !original_path.exists() {
        return Err(format!(
            "Fichier GPX introuvable : {}.",
            original_path.display()
        ));
    }

    // Backup : le seul filet de sécurité en cas de problème — jamais écrasé.
    let orig_path = original_path.with_extension("gpx.orig");
    if !orig_path.exists() {
        fs::copy(original_path, &orig_path)
            .map_err(|e| format!("Backup du GPX ({}): {}", orig_path.display(), e))?;
    }

    // Repli du nom de trace : nom de fichier sans extension (IHM §20.3).
    let fallback_name = original_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("trace")
        .to_string();
    let trk_name = source_trk_name.unwrap_or(&fallback_name).to_string();

    // Nom d'application : valeur blanche → défaut (IHM §20.4).
    let trimmed = app_name.trim();
    let tool = if trimmed.is_empty() {
        GPX_APP_DEFAULT
    } else {
        trimmed
    };

    let xml = build_gpx_xml(
        points,
        &trk_name,
        source_gpx_attrs,
        source_meta_xml,
        tool,
        findings_summary,
    )?;

    // Écriture atomique : un échec du `rename` laisse l'original intact.
    let tmp_path = original_path.with_extension("gpx.tmp");
    fs::write(&tmp_path, xml.as_bytes())
        .map_err(|e| format!("Écriture du GPX temporaire ({}): {}", tmp_path.display(), e))?;
    if let Err(e) = fs::rename(&tmp_path, original_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Remplacement du GPX ({}): {}", original_path.display(), e));
    }

    println!(
        "[GPX] GPX réécrit ({} points) — {} routage(s), {} suppression(s), {} faux positif(s)",
        points.len(),
        findings_summary.routes,
        findings_summary.deletions,
        findings_summary.false_positives
    );

    Ok(())
}

// ─── Sérialisation (privée) ───────────────────────────────────────────

/// Sérialise le document GPX complet (IHM §20.3).
fn build_gpx_xml(
    points: &[AuditPoint],
    trk_name: &str,
    source_gpx_attrs: Option<&str>,
    source_meta_xml: Option<&str>,
    tool: &str,
    summary: FindingsSummary,
) -> Result<String, String> {
    // Attributs du source réémis tels quels ; cas pathologique (aucun attribut) :
    // entête de secours (§20.3).
    let attrs = match source_gpx_attrs {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => format!(
            "version=\"1.1\" creator=\"{}\" {}",
            xml_escape(tool),
            GPX_HEADER_ATTRS_TAIL
        ),
    };

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str(&format!("<gpx {}>\n", attrs));
    x.push_str(&build_metadata_xml(trk_name, tool, source_meta_xml, summary)?);
    x.push_str(&format!(
        "  <trk><name>{}</name><trkseg>\n",
        xml_escape(trk_name)
    ));

    for p in points {
        x.push_str(&format!("    <trkpt lat=\"{:.7}\" lon=\"{:.7}\">", p.lat, p.lon));
        // `<ele>` seulement si l'élévation est finie (le JS teste `isFinite`).
        if let Some(ele) = p.ele {
            if ele.is_finite() {
                x.push_str(&format!("<ele>{:.2}</ele>", ele));
            }
        }
        x.push_str("</trkpt>\n");
    }

    x.push_str("  </trkseg></trk>\n</gpx>\n");
    Ok(x)
}

/// Sérialise le `<metadata>` : celui du source (avec `<desc>` remplacé et bloc
/// d'audit inséré en dernier enfant), ou un bloc minimal valide (§20.1, §20.2).
fn build_metadata_xml(
    trk_name: &str,
    tool: &str,
    source_meta_xml: Option<&str>,
    summary: FindingsSummary,
) -> Result<String, String> {
    let audit = build_audit_xml(tool, summary);

    if let Some(meta) = source_meta_xml {
        // Retrait du `<desc>` et des `<extensions>` du source, réinsérés ensuite.
        let stripped = re(RE_DESC)?.replace_all(meta, "").to_string();
        let stripped = re(RE_EXTENSIONS)?.replace_all(&stripped, "").to_string();

        let with_desc = re(RE_METADATA_OPEN)?
            .replace(
                &stripped,
                format!(
                    "${{1}}\n    <desc>Trace auditée et corrigée avec {}</desc>",
                    xml_escape(tool)
                ),
            )
            .to_string();
        // Le bloc d'audit est inséré en **dernière** position d'enfant.
        let with_audit = re(RE_METADATA_CLOSE)?
            .replace(&with_desc, format!("{}</metadata>", audit))
            .to_string();
        return Ok(format!("{}\n", with_audit));
    }

    Ok(format!(
        "  <metadata>\n    <name>{}</name>\n    <desc>Trace auditée et corrigée avec {}</desc>\n    <time>{}</time>\n{}  </metadata>\n",
        xml_escape(trk_name),
        xml_escape(tool),
        iso_now(),
        audit
    ))
}

/// Sérialise le bloc d'audit `<extensions><audit>` (IHM §20.2).
fn build_audit_xml(tool: &str, summary: FindingsSummary) -> String {
    let total = summary.routes + summary.deletions + summary.false_positives;
    format!(
        "  <extensions>\n    <audit xmlns=\"{}\">\n      <modified>{}</modified>\n      <tool>{}</tool>\n      <corrections total=\"{}\" routes=\"{}\" deletions=\"{}\" falsePositives=\"{}\"/>\n    </audit>\n  </extensions>\n",
        GPX_AUDIT_NS,
        iso_now(),
        xml_escape(tool),
        total,
        summary.routes,
        summary.deletions,
        summary.false_positives
    )
}

/// Échappement XML minimal (`&`, `<`, `>`, `"`) — l'ordre du JS est conservé
/// (`&` d'abord, sans quoi les entités seraient doublement échappées).
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Horodatage ISO 8601 UTC, à la seconde (le JS retire les millisecondes).
fn iso_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Compile un motif regex littéral. Un motif invalide serait une erreur de
/// programmation : elle est remontée en `Err` plutôt que de paniquer.
fn re(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("Motif interne invalide : {}", e))
}
