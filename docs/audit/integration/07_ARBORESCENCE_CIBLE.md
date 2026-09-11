# Livrable 7 — Arborescence cible du projet

**Rôle** : décrire l'arborescence complète de VisuGPS2 après intégration
du module Audit GPX. Sert de référence pour vérifier qu'aucun fichier
n'a été oublié lors de la migration.

**Légende** :
- `[N]` = nouveau (à créer)
- `[M]` = modifié (à mettre à jour)
- `[-]` = supprimé (à retirer)
- `[.]` = inchangé (aucune action)

---

## 1. Arborescence racine

```
VisuGPS2/
├── docs/
│   ├── ARCHITECTURE.md                    [M]
│   ├── CLAUDE-CODE-GUIDE.md               [.]
│   ├── COMMANDS.md                        [M]
│   ├── CONTEXT.md                         [M]
│   ├── CONVENTIONS.md                     [.]
│   ├── DATA_STORAGE.md                    [M]
│   ├── EXTENDING.md                       [.]
│   ├── README.md                          [M]
│   ├── ROADMAP_EDITION_CAMERA.md          [.]
│   ├── SPEC_AFFICHAGE_TRACES.md           [.]
│   ├── SPEC_IMPORT_GPX.md                 [M]
│   │
│   └── audit/                             [N] NOUVEAU DOSSIER
│       ├── README.md                      [N] Index du module
│       ├── spec/
│       │   ├── ANALYSE.md                 [N] (fourni par vous)
│       │   ├── CORRECTIONS.md             [N] (fourni par vous)
│       │   └── IHM_V2.md                  [N] (fourni par vous)
│       ├── reference/
│       │   ├── verifgpx-V3.0.html         [N] (fourni par vous)
│       │   └── test_files/
│       │       ├── scenario_17_1_ar_27pts.gpx  [N] (généré)
│       │       ├── AR_Detecté_aussi_en_RP.gpx  [N] (fourni par vous)
│       │       ├── rondpoints_g1.gpx           [N] (fourni par vous)
│       │       ├── rondpoints_g2.gpx           [N] (fourni par vous)
│       │       ├── rondpoints_g3.gpx           [N] (fourni par vous)
│       │       ├── RP_Santa_Susanna.gpx        [N] (fourni par vous)
│       │       ├── AR_Santa_Susanna.gpx        [N] (fourni par vous)
│       │       └── RP_erreur_Magny.gpx         [N] (fourni par vous)
│       └── integration/
│           ├── 01_TYPES_RS.md             [N] Livrable 1
│           ├── 02_AUDIT_TS.md             [N] Livrable 2
│           ├── 03_PHASE_AR.md             [N] Livrable 3
│           ├── 04_COMMANDS_TAURI.md       [N] Livrable 4
│           ├── 05_PLAN_MIGRATION.md       [N] Livrable 5
│           ├── 06_SETTINGS_AUDIT.md       [N] Livrable 6
│           ├── 07_ARBORESCENCE_CIBLE.md   [N] Livrable 7 (ce document)
│           ├── 08_PHASE_RP.md             [N] Livrable 8
│           ├── 09_CORRECTIONS.md          [N] Livrable 9
│           └── 10_COMPOSANTS_VUE.md       [N] Livrable 10
│
├── src/                                   (voir §2)
├── src-tauri/                             (voir §3)
├── package.json                           [.]
├── vite.config.ts                         [.]
├── tsconfig.json                          [.]
├── .env.example                           [.]
├── README.md                              [.]
├── QUICKSTART.md                          [.]
├── CHANGELOG.md                           [M] (si existant)
└── setup.sh / setup.ps1                   [.]
```

---

## 2. Frontend Vue — `src/`

```
src/
├── main.ts                                [.]
├── App.vue                                [.]
│
├── router/
│   └── index.ts                           [M] route audit / retrait nettoyage
│
├── stores/
│   ├── index.ts                           [.]
│   ├── app.ts                             [.]
│   ├── settings.ts                        [.]
│   ├── traces.ts                          [M] audit_status au lieu de cleaning_status
│   ├── keyframes.ts                       [.]
│   ├── edition.ts                         [.]
│   ├── ui.ts                              [.]
│   │
│   ├── cleaning.ts                        [-] SUPPRIMÉ
│   └── audit.ts                           [N] Livrable 2
│
├── views/
│   ├── Accueil.vue                        [.]
│   ├── EditionCamera.vue                  [M] redirection vers /audit
│   ├── ScreenBis.vue                      [.]
│   ├── Visualisation.vue                  [.]
│   │
│   ├── Cleaning.vue                       [-] SUPPRIMÉ
│   └── Audit.vue                          [N] Livrable 10
│
├── components/
│   ├── Accueil/
│   │   ├── Circuit.vue                    [M] editerCircuit() -> route audit
│   │   │                                      badge "À auditer"
│   │   │                                      icône mdi-map-marker-path
│   │   ├── CircuitsDrawer.vue             [.]
│   │   ├── Map.vue                        [.]
│   │   ├── AppBar.vue                     [.]
│   │   ├── ModeExecutionCard.vue          [.]
│   │   ├── SettingsDrawer.vue             [.]
│   │   └── SettingsCategory.vue           [.]
│   │
│   ├── Edition/                           [.] inchangé intégralement
│   │   ├── EditionMap.vue
│   │   ├── EditionToolbar.vue
│   │   ├── ViewportFrame.vue
│   │   ├── PlaybackControls.vue
│   │   ├── ProgressGraph.vue
│   │   ├── TelemetryHud.vue
│   │   ├── HeadingChangesPanel.vue
│   │   ├── DistanceHud.vue
│   │   └── CameraEditor.vue
│   │
│   ├── parameters/
│   │   ├── ParameterCard.vue              [M] branche "string" ajoutée
│   │   ├── InputBool.vue                  [.]
│   │   ├── InputInt.vue                   [.]
│   │   ├── InputFloat.vue                 [.]
│   │   ├── InputSecret.vue                [.]
│   │   ├── InputList.vue                  [.]
│   │   ├── InputRgba.vue                  [.]
│   │   ├── InputMaterial*.vue             [.]
│   │   ├── InputMonitor.vue               [.]
│   │   └── InputString.vue                [N] nouveau composant
│   │
│   ├── Cleaning/                          [-] DOSSIER SUPPRIMÉ
│   │   ├── CleaningToolbar.vue            [-]
│   │   ├── CleaningPhaseStepper.vue       [-]
│   │   ├── CleaningMap.vue                [-]
│   │   ├── CleaningCasesPanel.vue         [-]
│   │   └── CleaningPointTable.vue         [-]
│   │
│   └── Audit/                             [N] NOUVEAU DOSSIER
│       ├── AuditToolbar.vue               [N] Livrable 10
│       ├── AuditMap.vue                   [N] Livrable 10
│       ├── AuditFindingsPanel.vue         [N] Livrable 10
│       ├── AuditActionPanel.vue           [N] Livrable 10
│       ├── AuditSynthesis.vue             [N] Livrable 10
│       ├── AuditProgressChip.vue          [N] Livrable 10
│       └── dialogs/
│           ├── ConfirmExitDialog.vue      [N] Livrable 10
│           └── ConfirmApplyDialog.vue     [N] Livrable 10
│
├── algorithms/                            [.] inchangé intégralement
│   ├── keyframeGenerator.ts
│   ├── frustum.ts
│   └── headingChanges.ts
│
├── composables/
│   └── useSettingsTree.ts                 [.]
│
├── utils/
│   ├── format.ts                          [.]
│   ├── geo.ts                             [.]
│   └── materialColors.ts                  [.]
│
├── plugins/
│   └── vuetify.ts                         [.]
│
└── assets/                                [.]
    └── styles/
```

---

## 3. Backend Rust — `src-tauri/`

```
src-tauri/
├── Cargo.toml                             [.] (aucune nouvelle dépendance)
├── tauri.conf.json                        [.]
│
├── capabilities/
│   └── default.json                       [.]
│
├── icons/                                 [.]
│
├── settings.default.toml                  [M] Livrable 6
│                                              - retrait [Nettoyage.*]
│                                              - ajout [Audit.*]
│                                              - ajout [_meta.views.audit]
│
└── src/
    ├── main.rs                            [.] auto-généré
    ├── lib.rs                             [M] mod gpx_audit + 7 commandes
    │                                          retrait mod cleaning
    │                                          retrait 5 commandes cleaning
    ├── display.rs                         [.]
    ├── gestionMode.rs                     [.]
    ├── settings.rs                        [M] type "string" ajouté
    ├── import_gpx.rs                      [M] appelle gpx_audit::commands
    │                                          D1 : détection registre obsolète
    │                                          D2c : appel cleanup_obsolete
    │
    ├── cleaning.rs                        [-] SUPPRIMÉ
    │
    └── gpx_audit/                         [N] NOUVEAU MODULE
        ├── mod.rs                         [N] déclarations + réexports
        ├── types.rs                       [N] Livrable 1
        ├── ar.rs                          [N] Livrable 3
        ├── rp.rs                          [N] Livrable 8
        ├── anchor.rs                      [N] Livrable 8
        ├── geometry.rs                    [N] projector + build_geometry
        ├── consolidation.rs               [N] consolidate_points
        ├── corrections.rs                 [N] Livrable 9
        ├── migration.rs                   [N] Livrable 9 (D1 + D2c)
        ├── export.rs                      [N] réécriture GPX
        ├── commands.rs                    [N] Livrable 4 (7 commandes)
        └── tests/
            ├── mod.rs                     [N]
            ├── ar_test.rs                 [N] Livrable 3 (18 tests)
            ├── rp_test.rs                 [N] Livrable 8 (34 tests)
            ├── corrections_test.rs        [N] Livrable 9 (12 tests)
            └── commands_test.rs           [N] Livrable 4 (8 tests)
```

---

## 4. Stockage disque — `{app_data_dir}/`

```
{app_data_dir}/
├── .env                                   [.] inchangé
├── ModeExe.toml                           [.] inchangé
├── visugps2_master_key                    [.] inchangé (prod)
│
└── {active_mode}/                         (ex. OPE)
    ├── config.toml                        [M] surcharges Audit.* au lieu de Nettoyage.*
    │                                          (les surcharges Nettoyage.* résiduelles
    │                                           sont orphelines mais inoffensives)
    ├── config-dev.toml                    [M] idem
    │
    ├── traces.json                        [M] audit_status au lieu de cleaning_*
    │                                          (les anciens registres sont ignorés, D1)
    │
    └── traces/
        └── {trace_id}/
            ├── {filename}.gpx             [.] réécrit à audit_validate
            ├── {filename}.gpx.orig        [.] backup (posé à audit_validate)
            ├── trace.geojson              [.] régénéré à audit_validate
            ├── keyframes_169.json         [.]
            ├── keyframes_43.json          [.]
            │
            ├── cleaning.{phase}.json            [-] SUPPRIMÉ (D2c)
            └── cleaning.{phase}.decisions.json  [-] SUPPRIMÉ (D2c)
```

**Note** : aucune nouvelle structure de stockage n'est introduite par
le module Audit (les findings sont **volatils**, décision 6). Le
dossier `{trace_id}/` reste identique en structure ; seuls les fichiers
`cleaning.*` disparaissent.

---

## 5. Résumé chiffré

| Catégorie | Créés | Modifiés | Supprimés |
|---|---|---|---|
| **Documentation** | 11 (docs/audit/*) | 5 | 0 |
| **Frontend Vue — Vue** | 1 (`Audit.vue`) | 1 (`EditionCamera.vue`) | 1 (`Cleaning.vue`) |
| **Frontend Vue — Store** | 1 (`audit.ts`) | 1 (`traces.ts`) | 1 (`cleaning.ts`) |
| **Frontend Vue — Composants** | 9 (`Audit/*`) | 2 (`Circuit.vue`, `ParameterCard.vue`) | 5 (`Cleaning/*`) |
| **Frontend Vue — Autre** | 1 (`InputString.vue`) | 1 (`router/index.ts`) | 0 |
| **Rust — Modules** | 1 (`gpx_audit/`) | 3 (`lib.rs`, `settings.rs`, `import_gpx.rs`) | 1 (`cleaning.rs`) |
| **Rust — Fichiers internes** | 10 (`gpx_audit/*.rs`) | 0 | 0 |
| **Rust — Tests** | 5 (`tests/*.rs`) | 0 | 0 |
| **Configuration** | 0 | 1 (`settings.default.toml`) | 0 |
| **Stockage** | 0 | 2 (registre, configs) | 2 (`cleaning.*.json`) |
| **TOTAL** | **~39** | **~17** | **~11** |

---

## 6. Ordre de création recommandé

Cet ordre minimise les erreurs de compilation intermédiaires.

### Phase 1 — Squelette backend

1. `src-tauri/src/gpx_audit/mod.rs`
2. `src-tauri/src/gpx_audit/types.rs`
3. `src-tauri/src/gpx_audit/geometry.rs`
4. `src-tauri/src/gpx_audit/consolidation.rs`
5. `src-tauri/src/gpx_audit/ar.rs`
6. `src-tauri/src/gpx_audit/tests/mod.rs`
7. `src-tauri/src/gpx_audit/tests/ar_test.rs`
8. `src-tauri/src/lib.rs` (ajouter `mod gpx_audit;`)

### Phase 2 — Détecteur RP

9. `src-tauri/src/gpx_audit/rp.rs`
10. `src-tauri/src/gpx_audit/anchor.rs`
11. `src-tauri/src/gpx_audit/tests/rp_test.rs`

### Phase 3 — Corrections

12. `src-tauri/src/gpx_audit/corrections.rs`
13. `src-tauri/src/gpx_audit/migration.rs`
14. `src-tauri/src/gpx_audit/export.rs`
15. `src-tauri/src/gpx_audit/tests/corrections_test.rs`

### Phase 4 — Commandes et intégration backend

16. `src-tauri/src/gpx_audit/commands.rs`
17. `src-tauri/src/gpx_audit/tests/commands_test.rs`
18. `src-tauri/src/import_gpx.rs` (appel audit, D1, D2c)
19. `src-tauri/src/settings.rs` (type "string")
20. `src-tauri/src/lib.rs` (retrait cleaning, ajout commandes)
21. **Suppression** `src-tauri/src/cleaning.rs`

### Phase 5 — Frontend

22. `src/stores/audit.ts`
23. `src/stores/traces.ts` (audit_status)
24. `src/components/parameters/InputString.vue`
25. `src/components/parameters/ParameterCard.vue`
26. `src/components/Audit/AuditToolbar.vue`
27. `src/components/Audit/AuditProgressChip.vue`
28. `src/components/Audit/AuditFindingsPanel.vue`
29. `src/components/Audit/AuditSynthesis.vue`
30. `src/components/Audit/AuditActionPanel.vue`
31. `src/components/Audit/dialogs/ConfirmExitDialog.vue`
32. `src/components/Audit/dialogs/ConfirmApplyDialog.vue`
33. `src/components/Audit/AuditMap.vue`
34. `src/views/Audit.vue`
35. `src/router/index.ts`

### Phase 6 — Suppression et redirections

36. `src/components/Accueil/Circuit.vue` (redirection audit)
37. `src/views/EditionCamera.vue` (garde-fou audit)
38. **Suppression** `src/stores/cleaning.ts`
39. **Suppression** `src/views/Cleaning.vue`
40. **Suppression** `src/components/Cleaning/`

### Phase 7 — Configuration

41. `src-tauri/settings.default.toml`

### Phase 8 — Documentation

42. `docs/COMMANDS.md`
43. `docs/ARCHITECTURE.md`
44. `docs/DATA_STORAGE.md`
45. `docs/CONTEXT.md`
46. `docs/SPEC_IMPORT_GPX.md`
47. `CHANGELOG.md` (si existant)

### Phase 9 — Tests de bout en bout

48. Import trace → audit → correction → apply → rechargement
49. Navigation Accueil ↔ Audit ↔ EditionCamera
50. Vérification `audit_status` dans `traces.json`

---

## 7. Points de vigilance

| # | Piège | Mitigation |
|---|---|---|
| 1 | `src/components/Cleaning/` oublié après suppression de la vue | `grep -r "components/Cleaning" src/` avant suppression |
| 2 | `cleaningStore` encore importé dans un composant | `grep -r "cleaningStore\|stores/cleaning" src/` |
| 3 | `cleaning_status` encore utilisé dans un template | `grep -r "cleaning_status\|cleaning_phase" src/` |
| 4 | `docs/audit/` : ne pas mettre les livrables dans `docs/` racine (pollution) | Suivre la structure `docs/audit/integration/` |
| 5 | Route `/audit` non lazy : Mapbox GL chargé au démarrage | `component: () => import('../views/Audit.vue')` |
| 6 | `AuditMap.vue` : créer une 3ᵉ instance Mapbox sans détruire les précédentes | Chaque vue monte/démonte son instance (`onUnmounted` → `map.remove()`) |
| 7 | `settings.default.toml` : oubli de la table `[_meta]` pour la vue `audit` | Vérifier les 5 groupes + 1 vue |
| 8 | Documentation obsolète : `docs/COMMANDS.md` liste encore les commandes cleaning | Mettre à jour APRÈS la suppression effective |

---

## 8. Critères de validation globaux

Après migration complète :

- [ ] `cargo check` passe sans warning ni erreur
- [ ] `cargo test --lib` : tous les tests verts (72 tests attendus)
- [ ] `npx vue-tsc --noEmit` passe sans erreur
- [ ] `npm run build` réussit
- [ ] Aucune référence à `cleaning` dans le code source :
  ```bash
  grep -ri "cleaning" src/ src-tauri/src/ | grep -v "test_files"
  # -> ne doit rien retourner (sauf fichiers de test/documentation)
  ```
- [ ] La route `/nettoyage` renvoie une page blanche ou 404
- [ ] La route `/audit` fonctionne (drawer + carte + panneau)
- [ ] Le workflow complet Accueil → Audit → Édition caméra fonctionne
- [ ] Les 5 catégories de paramètres `Audit.*` apparaissent dans le drawer
- [ ] La documentation `docs/` est à jour

---

## 9. Vue d'ensemble finale (schéma)

```
                         ┌──────────────────────────┐
                         │       Accueil            │
                         │   (Circuit.vue Éditer)   │
                         └────────────┬─────────────┘
                                      │
                         audit_status === "clean" ?
                                      │
                    ┌─────────────────┴─────────────────┐
                    │                                   │
                   NON                                 OUI
                    │                                   │
                    ▼                                   ▼
        ┌─────────────────────┐              ┌─────────────────────┐
        │   /audit            │              │  /edition-camera    │
        │   (Audit.vue)       │              │  (EditionCamera.vue)│
        │                     │              └─────────────────────┘
        │  ┌───────────────┐  │
        │  │ AuditToolbar  │  │
        │  ├───────────────┤  │
        │  │ AuditFindings │  │
        │  │    Panel      │  │
        │  ├───────────────┤  │
        │  │  AuditMap     │  │ ← 3ᵉ instance Mapbox GL
        │  ├───────────────┤  │
        │  │ AuditAction   │  │
        │  │    Panel      │  │
        │  └───────────────┘  │
        └──────────┬──────────┘
                   │
                   │ Appliquer (pending === 0)
                   ▼
        ┌─────────────────────┐
        │ audit_validate      │  ← Commandes Tauri
        │  - backup .orig     │
        │  - réécriture GPX   │
        │  - audit_status=clean
        └──────────┬──────────┘
                   │
                   ▼
             Retour Accueil
             (store.reset())
```