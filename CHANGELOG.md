# Journal des modifications

> Toutes les modifications notables de VisuGPS2 sont consignées dans ce fichier.
> Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).
> Le projet suit un versionnement sémantique (`MAJEUR.MINEUR.CORRECTIF`).
>
> Version déclarée à ce jour : **0.0.1** (`package.json`, `src-tauri/Cargo.toml`,
> `src-tauri/tauri.conf.json`). Les entrées ci-dessous sont classées par date de
> livraison de phase, sans présumer du numéro de version à publier.

## [Non publié] — Module Audit GPX (remplace le module Nettoyage)

Refonte complète du contrôle de validité des traces GPX. L'ancien pipeline de
**nettoyage en 3 étapes** est retiré et remplacé par le module **Audit GPX**,
portage en Rust de l'application HTML de référence
(`docs/audit/reference/verifgpx-V3.0.html`), dont les spécifications normatives
sont `docs/audit/spec/ANALYSE.md` et `docs/audit/spec/CORRECTIONS.md`.

### Ajouté

**Module Rust `gpx_audit`** (`src-tauri/src/gpx_audit/`, 14 fichiers) :

- Détecteur **AR** — aller-retours ponctuels (rebonds, aiguilles de traceur) —
  avec paires miroir, budget de paires par sommet et garde anti-« virage en
  épingle » sur la longueur des branches (`ar.rs`).
- Détecteur **RP** — boucles de giratoire (270°, 360° et plus) — par cumul
  d'angle, quantification et gardes anti-aiguille (`rp.rs`), avec le calcul des
  ancres d'accès (`anchor.rs`).
- Consolidation préalable de la trace (`consolidation.rs`), projection
  équirectangulaire locale et géométrie métrique (`geometry.rs`), orchestration
  de la détection (`pipeline.rs`).
- Moteur de correction (`corrections.rs`) : resynchronisation des index par
  **identifiants stables** (invariant C4), suppression de plage, routage,
  garde d'imbrication, absorption des faux positifs, marquage et annulation.
- Aperçu prospectif de suppression (`preview.rs`) et éléments de rendu — ancres
  de routage, étiquettes (`overlay.rs`).
- Contrat algorithmique OpenRouteService, dont le test d'identité des deux
  tracés (`routing.rs`), et réécriture du GPX après audit (`export.rs`).
- **10 commandes Tauri** (`commands.rs`), portant le catalogue de l'application
  de 29 à **34 commandes**.

**Frontend** :

- Store Pinia `src/stores/audit.ts` (état **volatil**, sans persistance) et vue
  `src/views/Audit.vue` (route `/audit`, en **lazy loading**).
- Composants `src/components/Audit/` : `AuditToolbar`, `AuditProgressChip`,
  `AuditFindingsPanel`, `AuditSynthesis`, `AuditActionPanel`, `AuditMap` (3ᵉ
  instance Mapbox GL, avec `auditMapFeatures.ts` et `auditMapLayers.ts`), et les
  dialogues `ConfirmExitDialog` / `ConfirmApplyDialog`.
- Composable `src/composables/useAuditOrs.ts` : requêtes OpenRouteService
  (profils voiture et vélo, bascule automatique de clé sur quota épuisé).
- Namespace de paramètres `Audit.*` (5 catégories, 10 paramètres) exposé dans le
  drawer de la vue `/audit`.
- Nouveau type de paramètre **`"string"`** (texte libre, non chiffré, sans
  borne) et composant d'entrée `InputString.vue`, branché dans `ParameterCard.vue`.

### Modifié

- `TraceMetadata` : le champ `audit_status` (`"clean"` | `"needs_review"`)
  remplace `cleaning_status` et `cleaning_phase`. Statut posé à l'import par la
  détection AR + RP, et repassé à `"clean"` par l'audit.
- Détection à l'import : `gpx_audit::pipeline::detect_all` avec les paramètres
  `Audit.*` se substitue à `cleaning::detect_all_phases_from_gpx`, sous
  protection anti-panic (repli sur `"needs_review"`).
- `Circuit.vue` : badge « À auditer » et icône `mdi-map-marker-path` au lieu de
  « À nettoyer » / `mdi-broom` ; le bouton Éditer redirige vers
  `/audit?traceId=…`, la trace circulant désormais par la **query** de la route
  (plus de store intermédiaire).
- `EditionCamera.vue` : le garde-fou redirige vers `/audit?traceId=…`.
- `lib.rs` : `mod cleaning;` et ses 5 commandes retirés, `mod gpx_audit;` et ses
  10 commandes enregistrés.
- `migrate_mode_storage` : la migration de l'ancien agencement plat (`gpx/`,
  `geojson/`, `keyframes/`) est conservée, mais le dossier hérité `cleaning/`
  n'est plus migré — il est supprimé intégralement, ses fichiers de travail
  étant obsolètes.

### Supprimé

- Module Rust `src-tauri/src/cleaning.rs` (pipeline de nettoyage en 3 étapes).
- 5 commandes Tauri : `detect_trace_anomalies`, `get_cleaning_state`,
  `save_cleaning_state`, `reset_cleaning`, `validate_phase`.
- Store `src/stores/cleaning.ts`, vue `src/views/Cleaning.vue` et le dossier
  `src/components/Cleaning/` (5 composants).
- Route `/nettoyage`.
- Namespace de paramètres `Nettoyage.*` (6 paramètres), vue `_meta` associée et
  ses 2 groupes.

### Migration et compatibilité

Deux mécanismes automatiques, tous deux **silencieux** (aucun log utilisateur) :

- **D1 — registres pré-audit** : au chargement de `traces.json`,
  `load_registry` détecte la clé `"cleaning_status"` (format des versions
  antérieures) et retourne une liste **vide**, sans jamais réécrire le fichier.
  Les traces importées avant la migration disparaissent donc de l'interface,
  mais leurs dossiers et fichiers GPX restent **intacts sur disque** — aucune
  perte de données. Le registre est réécrit au nouveau format au prochain
  import.
- **D2c — artefacts hérités** : au premier accès à un mode, `get_mode_dir`
  supprime les fichiers de travail `cleaning.*.json` résiduels de tous les
  dossiers de traces. Opération idempotente.

> ⚠️ **Point d'attention** : D1 masque les traces antérieures, et le premier
> import suivant réécrit `traces.json` au nouveau format — les entrées de
> l'ancien format ne sont alors plus référencées. Sauvegardez `{app_data_dir}`
> avant la bascule si ces traces doivent être conservées.
>
> Les surcharges `Nettoyage.*` éventuelles dans `config.toml` /
> `config-dev.toml` deviennent orphelines et sans effet ; elles peuvent être
> supprimées manuellement.

### Tests

- `cargo test --lib` : **206 tests verts** (219 avant la migration : 16 tests
  retirés avec `cleaning.rs`, 3 ajoutés — 2 sur `load_registry` pour D1 et les
  registres illisibles, 1 sur la cohérence du namespace `Audit.*`).
- `cargo test --lib gpx_audit` : **200 tests verts** — `ar_test` (18),
  `rp_test` (61), `corrections_test` (54), `commands_test` (21),
  `overlay_test` (19), `preview_test` (14), `routing_test` (13).
- `cargo check --lib` sans avertissement, `npx vue-tsc --noEmit` et
  `npm run build` sans erreur.

### Documentation

- `docs/COMMANDS.md` : section Nettoyage remplacée par le catalogue des
  10 commandes Audit (signatures, types, comportement), `TraceMetadata` mis à
  jour, catalogue porté à 34 commandes.
- `docs/ARCHITECTURE.md` : section « Audit GPX (`/audit`) » en remplacement de
  « Nettoyage de trace GPX (`/nettoyage`) » (état d'audit, volatilité des
  findings, identifiants stables, 14 fichiers du module, migration D1/D2c,
  paramètres).
- `docs/DATA_STORAGE.md` : arborescence disque, bloc `traces.json`, note de
  migration et purge D2c, volatilité des findings, surcharges orphelines.
- `docs/CONTEXT.md`, `docs/SPEC_IMPORT_GPX.md` (version 1.3) et
  `docs/EXTENDING.md` : bascule des exemples et des descriptions vers le module
  Audit.
- `docs/audit/integration/04_COMMANDS_TAURI.md` : avenant du 2026-09-12
  entérinant les 3 commandes supplémentaires (`audit_map_overlay`,
  `audit_delete_preview`, `audit_routes_identical`).
- `docs/audit/` : dossier du module (spécifications normatives, artefact de
  référence gelé, fichiers de test de régression, plan d'intégration en
  11 livrables).

---

## [0.0.1] — Base initiale

État du projet avant la refonte du contrôle de validité des traces :

- Application desktop multi-fenêtres (Tauri v2 + Vue 3 + Vuetify 3 + Pinia),
  détection multi-écrans et placement automatique des fenêtres, synchronisation
  du thème entre fenêtres.
- Gestion des modes d'exécution isolés (`OPE`, `EVAL_*`).
- Système de paramètres TOML avec chiffrement AES-256-GCM des secrets et drawer
  de réglages entièrement piloté par la table `_meta`.
- Import/suppression/mise à jour de traces GPX (registre `traces.json`, un
  dossier par trace), favoris et affichage persistés.
- Carte Mapbox GL : clustering des points de départ, traces favorites et
  affichées en dégradé, focus carte, synchronisation carte ↔ liste triée par
  distance.
- Vue d'édition caméra : génération de keyframes (algorithmes `simple` et
  `frustum`), lecture par boucle `requestAnimationFrame`, graphe SVG
  d'avancement, HUD télémétrie et distance, édition fine des keyframes
  (`CameraEditor`), verrous de segments en mode validation.
- Nettoyage de trace GPX en 3 étapes séquentielles (points hors trace →
  ronds-points → aller/retour), avec widget de progression, carte dédiée et
  fichiers de travail par phase — **retiré** par l'entrée ci-dessus.

<!--
Gabarit pour les prochaines entrées :

## [X.Y.Z] — AAAA-MM-JJ

### Ajouté
### Modifié
### Déprécié
### Supprimé
### Corrigé
### Sécurité
-->
