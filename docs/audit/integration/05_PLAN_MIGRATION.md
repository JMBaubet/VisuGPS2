# Livrable 5 — Plan de migration vers le module Audit

**Rôle** : décrire toutes les modifications à apporter à VisuGPS2 pour
remplacer `cleaning.rs` / `Cleaning.vue` par le nouveau module Audit.

**Décisions appliquées** :
- Décision 2 : remplacement total de `Cleaning.vue`
- Décision 4 : `audit_status` remplace `cleaning_status`, gate dur,
  réécriture GPX, suppression des artefacts `cleaning.*`
- D1 : ignorer les anciens registres sans écraser
- D2c : supprimer les `cleaning.*.json` au premier accès
- D3b : silencieux (pas de log utilisateur)
- C.2 : reset du store à la sortie de la vue

**Durée estimée** : 1 à 2 jours (répartie sur les phases 1-4).

---

## 1. Vue d'ensemble des modifications

| Catégorie | Créé | Modifié | Supprimé |
|---|---|---|---|
| Rust backend | ~25 fichiers | 3 fichiers | 1 fichier |
| Frontend Vue | ~10 fichiers | 4 fichiers | 8 fichiers |
| Configuration | 0 | 1 fichier | 0 |
| **Total** | **~35** | **8** | **~9** |

---

## 2. Modifications Rust — `TraceMetadata`

### 2.1 État actuel

**Fichier** : `src-tauri/src/import_gpx.rs`

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceMetadata {
    pub id: String,
    pub name: String,
    pub source: String,
    pub source_url: Option<String>,
    pub activity_type: Option<String>,
    pub filename: String,
    pub import_date: String,
    pub stats: TraceStats,
    pub hash: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub is_displayed: bool,
    #[serde(default = "default_cleaning_status")]
    pub cleaning_status: String,   // ← À SUPPRIMER
    #[serde(default)]
    pub cleaning_phase: String,    // ← À SUPPRIMER
}
```

### 2.2 État cible

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceMetadata {
    pub id: String,
    pub name: String,
    pub source: String,
    pub source_url: Option<String>,
    pub activity_type: Option<String>,
    pub filename: String,
    pub import_date: String,
    pub stats: TraceStats,
    pub hash: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub is_displayed: bool,
    #[serde(default = "default_audit_status")]
    pub audit_status: String,      // NOUVEAU
}

fn default_audit_status() -> String {
    "needs_review".to_string()
}
```

### 2.3 Règles

- Le champ `audit_status` peut valoir : `"clean"`, `"needs_review"`,
  `"in_progress"`.
- Valeur par défaut si absent : `"needs_review"`.
- Les champs `cleaning_status` et `cleaning_phase` sont **supprimés**
  (les anciens registres qui les contiennent sont ignorés, cf. §3).

### 2.4 Impact TypeScript

**Fichier** : `src/stores/traces.ts`

```typescript
export interface TraceMetadata {
  // ... champs existants ...
  favorite: boolean
  is_displayed: boolean
  audit_status: string    // NOUVEAU (au lieu de cleaning_status)
}
```

Toutes les références à `cleaning_status` ou `cleaning_phase` dans le
frontend doivent être mises à jour vers `audit_status`.

---

## 3. Détection de registre obsolète (D1)

### 3.1 Principe

Au chargement de `traces.json`, si le fichier contient la clé
`"cleaning_status"`, on considère qu'il vient d'une version antérieure
et on retourne une liste vide. Le fichier n'est **pas** écrasé.

### 3.2 Implémentation

**Fichier** : `src-tauri/src/import_gpx.rs` — fonction `load_registry`
(ou `get_traces`).

```rust
pub fn load_registry(mode_dir: &Path) -> Result<Vec<TraceMetadata>, String> {
    let traces_path = mode_dir.join("traces.json");
    if !traces_path.exists() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(&traces_path)
        .map_err(|e| format!("Lecture traces.json : {}", e))?;

    // D1 : détection de registre obsolète (format pré-audit)
    if raw.contains("\"cleaning_status\"") {
        // Silencieux (D3b) — pas de log
        return Ok(Vec::new());
    }

    let traces: Vec<TraceMetadata> = serde_json::from_str(&raw)
        .map_err(|e| format!("Parse traces.json : {}", e))?;
    Ok(traces)
}
```

### 3.3 Effet utilisateur

- Les traces importées avant la migration **disparaissent de l'UI**.
- Les fichiers GPX restent sur disque (`{mode}/traces/{trace_id}/...`).
- L'utilisateur peut toujours les récupérer manuellement s'il le
  souhaite (aucune perte de données).
- Le `traces.json` est **laissé intact** jusqu'à ce qu'un nouvel import
  le réécrive (au prochain import, le nouveau format remplace l'ancien).

### 3.4 Non-régression

Aucune action automatique destructrice. L'utilisateur garde le contrôle
total de ses données.

---

## 4. Suppression des `cleaning.*.json` orphelins (D2c)

### 4.1 Principe

Au premier accès à un mode (via `get_mode_dir`), scanner le dossier
`{mode}/traces/*/` et supprimer tous les fichiers
`cleaning.{phase}.json` et `cleaning.{phase}.decisions.json`.

### 4.2 Implémentation

**Fichier** : `src-tauri/src/gpx_audit/migration.rs`

```rust
// src-tauri/src/gpx_audit/migration.rs

use std::path::Path;
use std::fs;

/// Supprime les fichiers cleaning.*.json orphelins dans tous les dossiers
/// de traces du mode. Silencieux (D3b) — pas de log.
pub fn cleanup_obsolete_cleaning_files(mode_dir: &Path) {
    let traces_dir = mode_dir.join("traces");
    if !traces_dir.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(&traces_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let trace_dir = entry.path();
        if !trace_dir.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&trace_dir) else {
            continue;
        };
        for f in files.flatten() {
            let path = f.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("cleaning.") && name.ends_with(".json") {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}
```

### 4.3 Point d'appel

Dans `import_gpx.rs`, fonction `get_mode_dir` (ou équivalent), après la
migration `migrate_mode_storage` existante :

```rust
pub fn get_mode_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // ... code existant ...

    // D2c : nettoyage des artefacts obsolètes (silencieux)
    crate::gpx_audit::migration::cleanup_obsolete_cleaning_files(&mode_dir);

    Ok(mode_dir)
}
```

### 4.4 Idempotence

La fonction est idempotente : si les fichiers ont déjà été supprimés, le
scan ne fait rien. Aucun drapeau à maintenir.

---

## 5. Suppression des artefacts

### 5.1 Backend Rust

| Fichier | Action | Détail |
|---|---|---|
| `src-tauri/src/cleaning.rs` | **SUPPRIMÉ** | Remplacé par `src-tauri/src/gpx_audit/*.rs` |
| `src-tauri/src/lib.rs` | **MODIFIÉ** | Retirer `mod cleaning;` + les 5 commandes associées ; ajouter `mod gpx_audit;` + les 7 commandes audit |
| `src-tauri/src/import_gpx.rs` | **MODIFIÉ** | Remplacer l'appel à `cleaning::detect_anomalies_from_gpx` par un appel à `gpx_audit::commands::audit_run_detection` (ou une fonction interne équivalente) |

### 5.2 Commandes Tauri à retirer

```rust
// Dans lib.rs, retirer :
cleaning::detect_trace_anomalies,
cleaning::get_cleaning_state,
cleaning::save_cleaning_state,
cleaning::reset_cleaning,
cleaning::validate_phase,
```

### 5.3 Frontend Vue

| Fichier | Action |
|---|---|
| `src/stores/cleaning.ts` | **SUPPRIMÉ** — remplacé par `src/stores/audit.ts` |
| `src/views/Cleaning.vue` | **SUPPRIMÉ** — remplacé par `src/views/Audit.vue` |
| `src/components/Cleaning/` | **DOSSIER SUPPRIMÉ** (5 composants) |
| `src/router/index.ts` | **MODIFIÉ** — retirer la route `nettoyage`, ajouter la route `audit` |

Composants supprimés :
- `CleaningToolbar.vue`
- `CleaningPhaseStepper.vue`
- `CleaningMap.vue`
- `CleaningCasesPanel.vue`
- `CleaningPointTable.vue`

### 5.4 Configuration

| Fichier | Action |
|---|---|
| `src-tauri/settings.default.toml` | **MODIFIÉ** — retirer `[Nettoyage.*]`, ajouter `[Audit.*]` (voir Livrable 6) |
| `docs/COMMANDS.md` | **MODIFIÉ** — retirer les 5 commandes cleaning, ajouter les 7 commandes audit |
| `docs/ARCHITECTURE.md` | **MODIFIÉ** — retirer les mentions du module cleaning |
| `docs/DATA_STORAGE.md` | **MODIFIÉ** — retirer les mentions `cleaning.{phase}.json` |

---

## 6. Nouveaux fichiers à créer

### 6.1 Backend Rust

```
src-tauri/src/gpx_audit/
├── mod.rs                  (squelette + réexports)
├── types.rs                (Livrable 1)
├── ar.rs                   (Livrable 3)
├── rp.rs                   (Livrable 8)
├── anchor.rs               (Livrable 8)
├── geometry.rs             (projector + build_geometry)
├── consolidation.rs        (consolidate_points)
├── corrections.rs          (Livrable 9)
├── migration.rs            (D1 + D2c — cf. §4)
├── export.rs               (réécriture GPX)
├── commands.rs             (Livrable 4)
└── tests/
    ├── mod.rs
    ├── ar_test.rs          (Livrable 3)
    ├── rp_test.rs          (Livrable 8)
    ├── corrections_test.rs (Livrable 9)
    └── commands_test.rs    (Livrable 4)
```

### 6.2 Frontend Vue

```
src/
├── views/
│   └── Audit.vue                       (nouvelle vue)
│
├── stores/
│   └── audit.ts                        (Livrable 2)
│
└── components/Audit/
    ├── AuditToolbar.vue                (Livrable 10)
    ├── AuditMap.vue                    (3ᵉ instance Mapbox GL)
    ├── AuditFindingsPanel.vue
    ├── AuditActionPanel.vue
    ├── AuditSynthesis.vue
    ├── AuditProgressChip.vue
    └── dialogs/
        ├── ConfirmExitDialog.vue
        └── ConfirmApplyDialog.vue
```

---

## 7. Modifications ciblées

### 7.1 `src/components/Accueil/Circuit.vue`

**Avant** :

```typescript
function editerCircuit() {
  editionStore.selectTrace(trace.id)
  if (trace.cleaning_status !== 'clean') {
    cleaningStore.selectTrace(trace.id)
    router.push({ name: 'nettoyage' })
  } else {
    router.push({ name: 'editionCamera' })
  }
}
```

**Après** :

```typescript
function editerCircuit() {
  editionStore.selectTrace(trace.id)
  if (trace.audit_status !== 'clean') {
    router.push({
      name: 'audit',
      query: { traceId: trace.id },
    })
  } else {
    router.push({ name: 'editionCamera' })
  }
}
```

**Note** : `cleaningStore` n'est plus importé. La sélection de trace
repose désormais sur `query.traceId` (pas de store intermédiaire).

### 7.2 `src/views/EditionCamera.vue`

**Avant** :

```typescript
onMounted(() => {
  // ... code existant ...

  const trace = tracesStore.traces.find(t => t.id === selectedTraceId)
  if (trace && trace.cleaning_status !== 'clean') {
    router.replace({ name: 'nettoyage' })
    return
  }
  // ... suite ...
})
```

**Après** :

```typescript
onMounted(() => {
  // ... code existant ...

  const trace = tracesStore.traces.find(t => t.id === selectedTraceId)
  if (trace && trace.audit_status !== 'clean') {
    router.replace({
      name: 'audit',
      query: { traceId: trace.id },
    })
    return
  }
  // ... suite ...
})
```

### 7.3 `src/router/index.ts`

**Avant** :

```typescript
const routes = [
  { path: '/', name: 'accueil', component: Accueil },
  { path: '/visualisation', name: 'visualisation', component: Visualisation },
  { path: '/edition-camera', name: 'editionCamera', component: EditionCamera },
  { path: '/nettoyage', name: 'nettoyage', component: Cleaning },
  { path: '/screen-bis', name: 'screenBis', component: ScreenBis },
]
```

**Après** :

```typescript
const routes = [
  { path: '/', name: 'accueil', component: Accueil },
  { path: '/visualisation', name: 'visualisation', component: Visualisation },
  { path: '/edition-camera', name: 'editionCamera', component: EditionCamera },
  { path: '/audit', name: 'audit', component: () => import('../views/Audit.vue') },
  { path: '/screen-bis', name: 'screenBis', component: ScreenBis },
]
```

**Note** : `Audit.vue` en lazy loading (contrairement aux autres vues
chargées statiquement) pour ne pas alourdir le bundle principal avec
Mapbox GL lors du premier démarrage.

### 7.4 `src/stores/traces.ts`

**Avant** :

```typescript
export interface TraceMetadata {
  // ... champs existants ...
  cleaning_status: string
  cleaning_phase: string
}
```

**Après** :

```typescript
export interface TraceMetadata {
  // ... champs existants ...
  audit_status: string   // au lieu de cleaning_status + cleaning_phase
}
```

### 7.5 `src/components/Accueil/Circuit.vue` — Badge de nettoyage

**Avant** :

```vue
<v-chip
  v-if="trace.cleaning_status !== 'clean'"
  color="orange"
  size="small"
>
  À nettoyer
</v-chip>
```

**Après** :

```vue
<v-chip
  v-if="trace.audit_status !== 'clean'"
  color="orange"
  size="small"
>
  À auditer
</v-chip>
```

Et l'icône du bouton Éditer :

**Avant** : `mdi-broom` (si `cleaning_status !== 'clean'`)  
**Après** : `mdi-map-marker-path` (si `audit_status !== 'clean'`)

---

## 8. Ordre d'exécution strict

### Étape 1 — Backend squelette (Phase 1)

1. Créer `src-tauri/src/gpx_audit/` avec `mod.rs` et `types.rs`
2. Ajouter `mod gpx_audit;` dans `lib.rs`, **garder** `mod cleaning;`
3. Vérifier : `cargo check` passe

### Étape 2 — Modification `TraceMetadata`

4. Ajouter `audit_status`, retirer `cleaning_status` / `cleaning_phase`
5. Adapter `load_registry` (D1) et `get_mode_dir` (D2c)
6. Adapter le frontend `src/stores/traces.ts`
7. Vérifier : `cargo check` + `npx vue-tsc --noEmit` passent

### Étape 3 — Phase 1 du portage AR

8. Implémenter `ar.rs` (Livrable 3)
9. Créer `ar_test.rs`, valider les 18 tests
10. Critère : scénario §17.1 passe

### Étape 4 — Phase 2 du portage RP

11. Implémenter `rp.rs` + `anchor.rs` (Livrable 8)
12. Créer `rp_test.rs`, valider les 34 tests
13. Critère : scénarios §17.2 passent

### Étape 5 — Corrections et export

14. Implémenter `corrections.rs`, `migration.rs`, `export.rs` (Livrable 9)
15. Créer `corrections_test.rs`, valider les 12 scénarios D1..D12

### Étape 6 — Commandes Tauri

16. Implémenter `commands.rs` (Livrable 4)
17. Enregistrer les 7 commandes dans `lib.rs`
18. **Retirer** les 5 commandes cleaning
19. **Retirer** `mod cleaning;`

### Étape 7 — Suppression de `cleaning.rs`

20. Supprimer `src-tauri/src/cleaning.rs`
21. Vérifier : `cargo check` passe sans warning

### Étape 8 — Frontend store

22. Créer `src/stores/audit.ts` (Livrable 2)
23. Supprimer `src/stores/cleaning.ts`
24. Vérifier : `npx vue-tsc --noEmit` passe

### Étape 9 — Frontend vue et composants

25. Créer `src/views/Audit.vue` (Livrable 10)
26. Créer les composants `src/components/Audit/*` (Livrable 10)
27. Supprimer `src/views/Cleaning.vue`
28. Supprimer `src/components/Cleaning/`

### Étape 10 — Router et redirections

29. Modifier `src/router/index.ts` (route `audit` au lieu de `nettoyage`)
30. Modifier `Circuit.vue` et `EditionCamera.vue` (redirection)
31. Vérifier : navigation fluide entre Accueil / Audit / EditionCamera

### Étape 11 — Paramètres

32. Modifier `src-tauri/settings.default.toml` (Livrable 6)
33. Ajouter le type `"string"` au système de paramètres
34. Vérifier : drawer Paramètres affiche `Audit.*`

### Étape 12 — Tests de bout en bout

35. Import trace → audit → correction → apply → rechargement
36. Vérifier la réécriture GPX + backup `.orig`
37. Vérifier `audit_status = "clean"` dans `traces.json`
38. Vérifier le comportement de redirection depuis Accueil

### Étape 13 — Documentation

39. Mettre à jour `docs/COMMANDS.md`
40. Mettre à jour `docs/ARCHITECTURE.md`
41. Mettre à jour `docs/DATA_STORAGE.md`
42. Mettre à jour `docs/CONTEXT.md` (nouvelle fonctionnalité)

---

## 9. Points de vigilance

| # | Piège | Mitigation |
|---|---|---|
| 1 | Import de `cleaningStore` oublié dans un composant après suppression | Rechercher globalement `cleaningStore` et `cleaning_status` avant de supprimer |
| 2 | `audit_status` manquant dans les anciens `traces.json` → panic serde | Utiliser `#[serde(default = "default_audit_status")]` |
| 3 | D1 : détection `"cleaning_status"` trop stricte (match partiel) | Utiliser la chaîne exacte `"cleaning_status"` (avec guillemets) |
| 4 | D2c : suppression `cleaning.*` pourrait toucher un fichier utilisateur légitime | Le pattern est très spécifique (`cleaning.{phase}.json`) et ne collisionne pas avec d'autres formats |
| 5 | `nextPointId` : doit être réinitialisé à la première analyse | Fait dans `runAudit` (Livrable 2) |
| 6 | Backup `.orig` : ne jamais écraser un `.orig` existant | Test explicite dans `audit_validate` |
| 7 | Route `/audit` en lazy loading : Mapbox GL ne doit pas être chargé au démarrage | `component: () => import('../views/Audit.vue')` |
| 8 | `Circuit.vue` : l'icône `mdi-broom` doit disparaître au profit d'une icône cohérente | Remplacer par `mdi-map-marker-path` |
| 9 | Suppression du dossier `src/components/Cleaning/` : vérifier qu'aucun autre composant ne l'importe | `grep -r "components/Cleaning" src/` avant suppression |
| 10 | `docs/COMMANDS.md` doit être mis à jour APRÈS la suppression effective des commandes | Sinon risque de désynchronisation doc ↔ code |

---

## 10. Critères de validation globaux

À la fin de la migration complète :

- [ ] `cargo check` passe sans warning ni erreur
- [ ] `cargo test --lib` : tous les tests verts
- [ ] `npx vue-tsc --noEmit` passe sans erreur
- [ ] `npm run build` réussit
- [ ] Aucune référence à `cleaning` dans le code source (`grep -ri cleaning src/ src-tauri/src/`)
- [ ] La route `/nettoyage` renvoie une 404
- [ ] La route `/audit` fonctionne
- [ ] Import → audit → édition caméra → retour fonctionne
- [ ] La documentation (`docs/`) est à jour
- [ ] Le `CHANGELOG.md` (si existant) mentionne la migration

---

## 11. Rollback (en cas de problème)

En cas d'échec critique, retour à l'état pré-migration :

```bash
# Sur la branche feature/audit-integration
git checkout main
```

**Aucune donnée utilisateur n'est perdue** :
- Les fichiers `traces.json` obsolètes sont **laissés intacts** (D1)
- Les fichiers `cleaning.*.json` sont supprimés (D2c) — c'est la seule
  perte irréversible, mais ils ne sont plus utilisés par aucune version
- Les fichiers GPX, `.orig`, `.geojson`, `keyframes_*` sont préservés

**Précaution recommandée** : avant de lancer la migration sur une
installation de production, faire une **sauvegarde complète** de
`{app_data_dir}`. Cette sauvegarde permet un rollback total en cas de
problème.