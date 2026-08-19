# 🧩 Spécification unifiée – Module d'import de fichiers GPX (Backend + Frontend)

> **Document de référence adapté à l'architecture réelle de VisuGPS2.**
> Ce fichier consolide et corrige les deux documents d'origine :
> - `010 Importation des fichiers gpx Back.md`
> - `010 Importation des fichiers gpx Front..md`
>
> Il tient compte des **modes d'exécution**, des **stores Pinia existants**, de l'**API Tauri 2.x** réellement utilisée, et des **conventions du projet**.

---

## 0. Récapitulatif des incohérences corrigées

Les documents d'origine reposaient sur une vision générique de Tauri. Voici les écarts majeurs avec l'application réelle, tous corrigés dans la présente spécification :

| # | Document d'origine | Application réelle | Correction apportée |
|---|--------------------|--------------------|---------------------|
| 1 | Stockage dans `{app_data_dir}/gpx/` et `{app_data_dir}/traces.json` | Les données vivent **dans le dossier du mode actif**, un dossier par trace : `{app_data_dir}/{active_mode}/traces/{trace_id}/` + `traces.json` (cf. `settings.rs`, `gestionMode.rs`, `import_gpx.rs`) | Stockage dans `{app_data_dir}/{active_mode}/traces/{trace_id}/` et `{app_data_dir}/{active_mode}/traces.json` |
| 2 | `tauri::api::dialog::FileDialogBuilder` et `tauri::api::path` (API Tauri **1.x**) | Tauri **2.x** : plugins (`tauri-plugin-dialog`) et `app.path().app_data_dir()` | Utilisation du plugin `tauri-plugin-dialog` + `app.path()` |
| 3 | Enregistrement de la commande dans `main.rs` | `main.rs` ne fait qu'appeler `tauri_app_lib::run()` ; les commandes sont enregistrées dans **`lib.rs`** (`invoke_handler`) | Enregistrement dans `lib.rs` |
| 4 | 3 options pour gérer les traces (store / composable / état local) + `readTextFile` direct côté frontend | Le projet utilise systématiquement le **pattern Setup Store Pinia** (`app.ts`, `settings.ts`) et passe par des **commandes Tauri** (le backend seul connaît le mode actif) | Création d'un `useTracesStore` + commande `get_traces` côté backend |
| 5 | Aucune mention des **capabilities** Tauri 2 | `capabilities/default.json` régit les permissions ; `fs` et `dialog` ne sont pas activés par défaut | Ajout des permissions `dialog:default` (et **pas** de `fs`, pour ne pas exposer le système de fichiers) |
| 6 | Fichier Rust `importGpx.rs` (camelCase) | Convention Rust snake_case ; `gestionMode.rs` est l'unique exception (`#[allow(non_snake_case)]`) | Fichier `import_gpx.rs` |
| 7 | Snackbar local dans `CircuitsDrawer.vue` | Pattern store Pinia partout ; notifications réutilisables | Store de notifications (ou composable) mutualisé |
| 8 | Bouton `mdi-image-plus-outline` décrit avec `@click="importerGpx"` | Le bouton existe dans `CircuitsDrawer.vue` mais **sans `@click`** | Câblage du `@click` sur le bouton existant |
| 9 | `Circuit.vue` non évoqué | `Circuit.vue` affiche des données **en dur** (`Distance : 123.5 km | Dénivelé : 2094 m`) | `Circuit.vue` consomme les stats issues du store de traces |
| 10 | Liste de dépendances Cargo générique | `serde`, `serde_json`, `chrono` déjà présents ; `sha2`, `uuid`, `hex`, `log` déjà transitifs dans `Cargo.lock` | Dépendances **réellement à ajouter** : `gpx`, `geo`, `tauri-plugin-dialog` |

---

## 1. 📌 Contexte du projet

**Stack réelle** (vérifiée dans `package.json`, `Cargo.toml`, `tauri.conf.json`) :

- **Backend** : Tauri **2.x** + Rust (édition **2024**), modules `display.rs`, `gestionMode.rs`, `settings.rs`, orchestration dans `lib.rs`.
- **Frontend** : Vue **3.5** + Vuetify **3.12** + TypeScript **5.6** + Pinia **3** + Vue Router **4.6** + Mapbox GL **3.24**.
- **Identifier Tauri** : `com.jean-marc.baubet.visugps2`.
- **Sortie build** : `dist/` → embarqué par Tauri.

**Objectif fonctionnel** : importer un fichier `.gpx` (Garmin Connect, Strava, OpenRunner, RideWithGPS…), en extraire les métadonnées macroscopiques, le stocker localement, et le référencer dans un registre afin d'alimenter la liste des circuits du `CircuitsDrawer`.

**Point d'entrée UI** : le bouton `mdi-image-plus-outline` existant dans `CircuitsDrawer.vue` (actuellement sans `@click`).

---

## 2. 🎯 Architecture cible (intégration dans l'app existante)

```
┌─────────────────────────────────────────────────────────────┐
│  CircuitsDrawer.vue                                         │
│    └─ bouton mdi-image-plus-outline  ── @click="importerGpx"│
│         │                                                    │
│         ▼                                                    │
│  useTracesStore.importerGpx()   (src/stores/traces.ts)      │
│    │     │                                                   │
│    │     └── invoke('import_gpx_file')  ─────────┐           │
│    │                                              ▼          │
│    │     ┌────────────────────────────────────────────┐     │
│    │     │ Backend Rust : import_gpx.rs               │     │
│    │     │   - dialog (plugin) → sélection fichier    │     │
│    │     │   - parsing gpx + stats                     │     │
│    │     │   - SHA256 (anti-doublon)                   │     │
│    │     │   - stockage dans {app_data}/{mode}/traces/{trace_id}/    │     │
│    │     │   - mise à jour de {app_data}/{mode}/       │     │
│    │     │     traces.json (écriture atomique)         │     │
│    │     └────────────────────────────────────────────┘     │
│    ▼                                                          │
│  useTracesStore.loadTraces()  → invoke('get_traces')         │
│    │                                                          │
│    ▼                                                          │
│  CircuitsDrawer.vue  →  <Circuit v-for="..." :trace="t" />   │
│    └─ Circuit.vue lit les stats (distance, dénivelé…)         │
└─────────────────────────────────────────────────────────────┘
```

**Règle d'or** : le frontend ne lit/écrit **jamais** directement le système de fichiers. Tout passe par des commandes Tauri, car **seul le backend connaît le mode d'exécution actif** et donc le bon dossier de stockage.

---

## 3. 🗂️ Gestion des chemins et modes d'exécution

### 3.1 Rappel du fonctionnement des modes (existant)

L'application gère plusieurs **modes d'exécution** (voir `gestionMode.rs`, `settings.rs`, `ModeExecutionCard.vue`) :

- Le mode actif est lu depuis `{app_data_dir}/.env` :
  - clé `APP_ENV_DEV` en développement (`cfg!(debug_assertions)`)
  - clé `APP_ENV_PROD` en production (build release)
- Les modes sont décrits dans `{app_data_dir}/ModeExe.toml` ; chaque mode possède un **dossier physique** `{app_data_dir}/{nom_mode}/` (ex : `OPE/`, `EVAL_test/`).
- `gestionMode::read_active_mode(app_data_dir, is_dev)` retourne le mode actif.
- Les **settings** appliquent déjà ce pattern : `{app_data_dir}/{active_mode}/config-dev.toml` (dev) ou `config.toml` (prod).

### 3.2 Application au module GPX

Pour rester cohérent avec le reste de l'application, **toutes les données GPX doivent vivre dans le dossier du mode actif** :

```text
{app_data_dir}/
└── {active_mode}/            ← ex: OPE, EVAL_essai
    ├── gpx/                  ← fichiers .gpx copiés (nom unique si doublon)
    └── traces.json           ← registre principal (Vec<TraceMetadata>)
```

**Résolution du chemin côté backend** (pseudo-code cohérent avec `settings.rs`) :

```rust
let is_dev = cfg!(debug_assertions);
let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
let active_mode = crate::gestionMode::read_active_mode(&app_data_dir, is_dev);
let mode_dir = app_data_dir.join(&active_mode);
let gpx_dir = mode_dir.join("gpx");
let traces_path = mode_dir.join("traces.json");
std::fs::create_dir_all(&gpx_dir).map_err(|e| e.to_string())?;
```

> ℹ️ **Conséquence** : changer de mode d'exécution isole automatiquement les bibliothèques de traces. C'est le comportement attendu et cohérent avec l'isolation des `config.toml`.

---

## 4. 🦀 Backend Rust – `src-tauri/src/import_gpx.rs`

### 4.1 Commandes Tauri à exposer

Quatre commandes (le frontend ne touche jamais au disque) :

```rust
#[tauri::command]
async fn import_gpx_file(app: tauri::AppHandle) -> Result<TraceMetadata, String>

#[tauri::command]
async fn get_traces(app: tauri::AppHandle) -> Result<Vec<TraceMetadata>, String>

#[tauri::command]
async fn delete_trace(app: tauri::AppHandle, trace_id: String) -> Result<(), String>

#[tauri::command]
async fn update_trace(
    app: tauri::AppHandle,
    trace_id: String,
    favorite: Option<bool>,
    is_displayed: Option<bool>,
) -> Result<(), String>
```

- `import_gpx_file` : ouvre le sélecteur natif (**fichier unique**, filtre `*.gpx`), parse, calcule, stocke, met à jour le registre, retourne la métadonnée.
- `get_traces` : lit `{app_data_dir}/{active_mode}/traces.json` et renvoie `Vec<TraceMetadata>` (vecteur vide si absent/illisible).
- `delete_trace` : supprime le fichier GPX (tolérant si absent) puis l'entrée du registre (recherche par `id` UUID), sauvegarde atomique.
- `update_trace` : mise à jour partielle (PATCH) — seuls les champs `Some(...)` sont modifiés.

### 4.2 Sélecteur de fichier (Tauri 2.x)

**Ne pas utiliser** `tauri::api::dialog::FileDialogBuilder` (API 1.x). Utiliser le plugin `tauri-plugin-dialog` :

```rust
use tauri_plugin_dialog::DialogExt;

let path = app.dialog()
    .file()
    .add_filter("Fichier GPX", &["gpx"])
    .blocking_pick_file();

let Some(file_path) = path else {
    return Err("Aucun fichier sélectionné.".to_string()); // annulation
};
let file_path = file_path.into_path().map_err(|e| e.to_string())?;
```

### 4.3 Parsing et statistiques

Parser avec le crate [`gpx`](https://crates.io/crates/gpx) (via [`geo`](https://crates.io/crates/geo) pour Haversine).

**Métadonnées générales**

| Champ | Source / Méthode |
|-------|------------------|
| Nom de la trace | `<name>` du GPX, sinon nom du fichier sans extension |
| Source (éditeur) | Détection heuristique (§4.4) |
| URL source | Selon l'éditeur détecté |
| Type d'activité | `<type>` si présent (`running`, `cycling`, `hiking`…) |

**Statistiques calculées**

| Champ | Méthode |
|-------|---------|
| Point de départ | Premier `<trkpt>` du premier `<trkseg>` (lat, lon, ele) |
| Point d'arrivée | Dernier `<trkpt>` du dernier `<trkseg>` |
| Distance totale (m) | Haversine + Pythagore 3D entre points successifs |
| Dénivelé + cumulé (m) | Somme des Δalt > 0 |
| Dénivelé − cumulé (m) | Somme des Δalt < 0 (valeur absolue) |
| Altitude min / max | min / max de tous les `<ele>` |
| Nombre de points | Total des `<trkpt>` tous segments confondus |
| Durée estimée (s) | Si `<time>` présents : dernier − premier ; sinon `null` |

**Informations techniques**

| Champ | Méthode |
|-------|---------|
| Hash SHA256 | Sur le contenu binaire (anti-doublon), préfixé `sha256:` |
| Date d'import | ISO 8601 UTC (`chrono::Utc::now()`) |
| Nom stocké | Nom effectif après copie dans `gpx/` (désambiguïsation si conflit) |

**Formule de distance 3D**

```text
distance_2d = haversine(lat1, lon1, lat2, lon2)   // mètres
delta_alt   = alt2 - alt1                          // 0 si une altitude manque
distance_3d = sqrt(distance_2d² + delta_alt²)
```

Haversine (rayon terrestre moyen R = 6 371 000 m) :

```text
a = sin²(Δlat/2) + cos(lat1)·cos(lat2)·sin²(Δlon/2)
c = 2·atan2(√a, √(1−a))
distance = R·c
```

### 4.4 Détection de l'éditeur et URL source

Par priorité décroissante :

1. **`<link href="…">`** (domaine) :
   - `strava.com` → `"Strava"`
   - `connect.garmin.com` → `"Garmin Connect"`
   - `openrunner.com` → `"OpenRunner"`
   - `ridewithgps.com` → `"RideWithGPS"`
   - autre → domaine extrait
   - URL complète conservée comme `source_url`.
2. **`<author><name>`** (fallback) : mots-clés connus → éditeur correspondant.
3. **Signature XML** (fallback) : namespace `gpxtpx:` → probablement Garmin ; extensions propriétaires Strava, etc.
4. **Fallback final** : éditeur = `"Inconnu"`, `source_url` = `None`.

### 4.5 Gestion du registre `traces.json`

1. Vérifier l'existence de `traces.json` dans le dossier du mode actif.
2. Si oui → lire et désérialiser en `Vec<TraceMetadata>` ; sinon → vecteur vide.
3. Pour le fichier importé :
   - **Doublon SHA256** → ne ni copier ni enregistrer, retourner `Err("Cette trace a déjà été importée.")`.
   - Sinon → copier dans `gpx/` avec nom unique si conflit (`trace.gpx` → `trace_1.gpx`).
   - Ajouter l'entrée au registre.
   - **Écriture atomique** : écrire dans un `.tmp` puis `rename`.

### 4.5-bis Détection automatique des anomalies (statut de nettoyage)

Une trace n'est **valide** que si elle est « propre ». Les fichiers GPX édités (OpenRunner, etc.) contiennent souvent des anomalies de relevé : **points isolés hors trace** (ex. point 946) ou **aller-retours inutiles** (ex. points 711/791) — détectables par un **changement de cap proche de 180°** au point de demi-tour.

À l'import, avant l'enregistrement au registre, le backend lance une **détection automatique** via `cleaning::detect_anomalies_from_gpx`, avec la tolérance de cap paramétrable **`Nettoyage.Cap.toleranceDeg`** (défaut 5°, plage 1–20°, pas 0,5°, unité `°`). Le résultat est porté par le champ **`cleaning_status`** de `TraceMetadata` :

- aucune anomalie → `"clean"` ;
- anomalies détectées → `"needs_review"`.

Une trace non « clean » ne peut **pas** entrer en édition caméra : le bouton Éditer de l'accueil redirige vers la vue `/nettoyage`, et `EditionCamera.vue` redirige également vers `/nettoyage` (garde-fou). Le workflow de nettoyage (3 étapes séquentielles, validation manuelle des cas, sauvegardes partielles par phase dans `traces/{trace_id}/cleaning.{phase}.json`, validation d'étape avec GPX réécrit + backup `.orig`) est décrit dans [ARCHITECTURE.md](./ARCHITECTURE.md), section « Nettoyage de trace GPX ».

### 4.6 Structures de données Rust

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Point3D {
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TraceStats {
    pub start_point: Point3D,
    pub end_point: Point3D,
    pub distance_m: f64,
    pub positive_elevation_m: f64,
    pub negative_elevation_m: f64,
    pub alt_min_m: Option<f64>,
    pub alt_max_m: Option<f64>,
    pub points_count: usize,
    pub duration_s: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TraceMetadata {
    pub id: String,                       // UUID v4
    pub name: String,
    pub source: String,
    pub source_url: Option<String>,
    pub activity_type: Option<String>,
    pub filename: String,                 // nom dans gpx/
    pub import_date: String,              // ISO 8601 UTC
    pub stats: TraceStats,
    pub hash: String,                     // "sha256:…"
    #[serde(default)]
    pub favorite: bool,                   // marquer comme favori (persisté)
    #[serde(default)]
    pub is_displayed: bool,               // afficher sur la carte (persisté)
    #[serde(default)]
    pub cleaning_status: String,          // "clean" (défaut) | "needs_review" | "in_progress"
}
```

> ℹ️ Les noms de champs en `snake_case` sérialisés tels quels correspondent exactement aux interfaces TypeScript du frontend (§5.3). Inutile d'ajouter `#[serde(rename_all = …)]`.
> ℹ️ Le champ `cleaning_status` est posé à l'import (détection automatique, §4.5-bis). Absent dans les registres antérieurs → `"clean"` (rétrocompatibilité via `#[serde(default)]`).

### 4.7 Gestion des erreurs

| Cas | Retour |
|-----|--------|
| XML mal formé | `Err` descriptif |
| Aucun point | `Err("Le fichier GPX ne contient aucun point.")` |
| Doublon SHA256 | `Err("Cette trace a déjà été importée.")` |
| Annulation dialog | `Err("Aucun fichier sélectionné.")` |
| Erreur disque | `Err` avec message système |

### 4.8 Bonus

- Vérifier l'extension `.gpx` du fichier sélectionné (sécurité).
- Journaliser le temps d'import (le projet utilise `println!`/`eprintln!` ; `log` est déjà transitif dans `Cargo.lock`).
- Nettoyer les noms de fichiers : espaces et caractères spéciaux → `_`.

---

## 5. ⚙️ Intégration backend dans le projet existant

### 5.1 Dépendances Cargo à ajouter dans `src-tauri/Cargo.toml`

**Déjà présents** (ne pas réajouter) : `serde` (+ `derive`), `serde_json`, `chrono`, `tauri`.
**Déjà transitifs** dans `Cargo.lock` (à promouvoir en dépendances directes si usage explicite) : `sha2`, `uuid`, `hex`, `log`.

À ajouter explicitement :

```toml
[dependencies]
# … dépendances existantes conservées …
gpx = "0.10"
geo = "0.29"
sha2 = "0.10"
uuid = { version = "1", features = ["v4"] }
hex = "0.4"
tauri-plugin-dialog = "2"
```

### 5.2 Plugin dialog dans `lib.rs`

```rust
mod display;
#[allow(non_snake_case)]
mod gestionMode;
mod settings;
mod import_gpx;                          // ← nouveau module

use display::{get_displays, open_second_window, close_second_window};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())           // ← plugin dialog
        .setup(|app| {
            let settings_state = settings::init_settings_state(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.manage(settings_state);
            display::setup_display(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            exit_app,
            get_displays,
            open_second_window,
            close_second_window,
            gestionMode::get_execution_env,
            gestionMode::get_modes,
            gestionMode::create_mode,
            gestionMode::update_mode,
            gestionMode::delete_mode,
            gestionMode::select_mode,
            settings::get_settings,
            settings::update_setting,
            settings::reset_setting,
            settings::get_setting_value,
            import_gpx::import_gpx_file,               // import
            import_gpx::get_traces,                    // liste
            import_gpx::delete_trace,                  // suppression
            import_gpx::update_trace                   // mise à jour partielle (favorite/is_displayed)
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> ⚠️ **Ne pas modifier `main.rs`** : il se contente d'appeler `tauri_app_lib::run()`.

### 5.3 Capabilities – `src-tauri/capabilities/default.json`

Ajouter la permission du plugin dialog. **On n'active pas `fs`** : le frontend ne lit pas le disque directement, tout transite par les commandes Tauri (le backend résout le chemin du mode actif).

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main", "screen-bis"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:default"
  ]
}
```

---

## 6. 🖥️ Frontend – Store Pinia `src/stores/traces.ts`

Le projet utilise le **pattern Setup Store** partout (`app.ts`, `settings.ts`). On crée un store dédié aux traces, qui centralise l'appel aux commandes Tauri et l'état réactif.

```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// --- Types (miroir exact des structs Rust, snake_case) ---
export interface Point3D {
  lat: number
  lon: number
  alt: number | null
}

export interface TraceStats {
  start_point: Point3D
  end_point: Point3D
  distance_m: number
  positive_elevation_m: number
  negative_elevation_m: number
  alt_min_m: number | null
  alt_max_m: number | null
  points_count: number
  duration_s: number | null
}

export interface TraceMetadata {
  id: string
  name: string
  source: string
  source_url: string | null
  activity_type: string | null
  filename: string
  import_date: string
  stats: TraceStats
  hash: string
  /** Trace marquée comme favorite (persisté). */
  favorite: boolean
  /** Trace affichée sur la carte (persisté). */
  is_displayed: boolean
}

export const useTracesStore = defineStore('traces', () => {
  // State
  const traces = ref<TraceMetadata[]>([])
  const loading = ref(false)

  // Getters
  const traceCount = computed(() => traces.value.length)

  // Actions
  async function loadTraces() {
    loading.value = true
    try {
      traces.value = await invoke<TraceMetadata[]>('get_traces')
    } catch (error) {
      console.error('Failed to load traces:', error)
      traces.value = []
    } finally {
      loading.value = false
    }
  }

  /** Importe un GPX. Lève une erreur (string) en cas de doublon / fichier invalide. */
  async function importerGpx(): Promise<TraceMetadata> {
    loading.value = true
    try {
      const result = await invoke<TraceMetadata>('import_gpx_file')
      await loadTraces() // recharger la liste depuis le backend (source de vérité)
      return result
    } finally {
      loading.value = false
    }
  }

  return {
    traces,
    loading,
    traceCount,
    loadTraces,
    importerGpx,
  }
})
```

**Pourquoi un store plutôt que `readTextFile` ?**
- Le frontend ignore le mode d'exécution actif : seul le backend peut résoudre `{app_data_dir}/{active_mode}/traces.json`.
- Cohérence avec `useSettingsStore` et `useAppStore` (même pattern).
- État réactif partagé entre `CircuitsDrawer.vue` et tout autre composant (ex : future galerie de traces).

---

## 7. 🔔 Notifications – store/composable de snackbar

Plutôt qu'un snackbar local dans `CircuitsDrawer.vue`, mutualiser la notification. Deux options acceptables selon le projet :

- **Option recommandée** : un mini store `src/stores/ui.ts` (pattern setup store) exposant `showSuccess / showError / showWarning` et l'état `snackbar`.
- Ou un composable `src/utils/useSnackbar.ts`.

État du snackbar :

```typescript
const snackbar = reactive({
  show: false,
  message: '',
  color: 'success' as 'success' | 'error' | 'warning' | 'info'
})
```

Composant `v-snackbar` (rendu une seule fois au niveau de `App.vue` ou d'`Accueil.vue`) :

```vue
<v-snackbar
  v-model="snackbar.show"
  :color="snackbar.color"
  :timeout="4000"
  location="bottom right"
  rounded="pill"
>
  {{ snackbar.message }}
  <template #actions>
    <v-btn icon="mdi-close" variant="text" @click="snackbar.show = false" />
  </template>
</v-snackbar>
```

---

## 8. 🎨 Intégration UI – `CircuitsDrawer.vue` et `Circuit.vue`

### 8.1 `CircuitsDrawer.vue` – câblage du bouton et rendu dynamique

Le bouton `mdi-image-plus-outline` existe déjà mais **sans `@click`**. On le câble et on rend la liste **réactive** depuis le store :

```vue
<script setup lang="ts">
import { onMounted } from 'vue'
import Circuit from './Circuit.vue'
import { useTracesStore } from '@/stores/traces'
import { useUiStore } from '@/stores/ui'   // snackbar mutualisé

const tracesStore = useTracesStore()
const ui = useUiStore()

async function importerGpx() {
  try {
    const trace = await tracesStore.importerGpx()
    ui.showSuccess(`Trace « ${trace.name} » importée avec succès.`)
  } catch (error) {
    const msg = typeof error === 'string' ? error : ''
    if (msg.includes('Aucun fichier')) {
      // annulation utilisateur : ne rien afficher
      return
    }
    if (msg.includes('déjà été importée')) {
      ui.showWarning(msg)
      return
    }
    ui.showError(msg || 'Une erreur est survenue lors de l\'import.')
  }
}

onMounted(() => {
  tracesStore.loadTraces()
})
</script>

<template>
  <v-navigation-drawer permanent width="500" class="py-2">
    <v-btn
      icon="mdi-image-plus-outline"
      variant="flat"
      :disabled="tracesStore.loading"
      title="Importer une trace GPX"
      @click="importerGpx"
    />
    <v-btn icon="mdi-image-edit-outline" variant="flat" />
    <v-btn icon="mdi-image-search-outline" variant="flat" />

    <v-progress-linear
      v-if="tracesStore.loading"
      indeterminate
      color="primary"
    />

    <v-list-item class="pt-4 px-0">
      <Circuit
        v-for="trace in tracesStore.traces"
        :key="trace.id"
        :trace="trace"
      />
    </v-list-item>
  </v-navigation-drawer>
</template>
```

> ℹ️ Les circuits codés en dur actuellement dans `CircuitsDrawer.vue` sont remplacés par `tracesStore.traces`. La logique provisoire peut être conservée temporairement le temps de valider l'import.

### 8.2 `Circuit.vue` – consommation des métadonnées

Aujourd'hui `Circuit.vue` affiche `Distance : 123.5 km | Dénivelé : 2094 m` **en dur**. Adapter les props pour recevoir un objet `TraceMetadata` :

```typescript
import type { TraceMetadata } from '@/stores/traces'

defineProps<{
  trace: TraceMetadata
  backgroundColor?: string
}>()
```

Affichage des stats calculées :

```vue
<v-card-text style="display: flex; align-items: center" class="pt-2">
  <span>
    Distance : {{ (trace.stats.distance_m / 1000).toFixed(1) }} km
    | Dénivelé : {{ Math.round(trace.stats.positive_elevation_m) }} m
  </span>
  <v-spacer />
  <v-icon v-if="trace.source_url" icon="mdi-link" @click="ouvrirSource" />
</v-card-text>
```

Helpers de formatage (ex : `src/utils/format.ts`) :

```typescript
export function formatDistance(m: number): string {
  return `${(m / 1000).toFixed(1)} km`
}
export function formatElevation(m: number): string {
  return `${Math.round(m)} m`
}
```

---

## 9. 🧪 Cas d'erreur et comportements attendus

| Cas | Comportement |
|-----|--------------|
| Fichier non `.gpx` | Le filtre natif empêche la sélection |
| Annulation du dialogue | Ne rien afficher, ne rien faire |
| Doublon SHA256 | Snackbar warning : « Cette trace a déjà été importée. » |
| GPX invalide | Snackbar error avec le message du backend |
| Erreur disque | Snackbar error : « Erreur lors de l'enregistrement du fichier. » |
| Import réussi | Snackbar success : « Trace « Nom » importée avec succès. » + rechargement de la liste |

**État de chargement** : `tracesStore.loading` désactive le bouton et affiche une `v-progress-linear` (cf. §8.1).

---

## 10. ✅ Livrables attendus

### Backend
1. **`src-tauri/src/import_gpx.rs`** : structs, parsing, calculs, détection d'éditeur, gestion du registre (chemin du mode actif), écriture atomique, commandes `import_gpx_file` et `get_traces`. Commentaires en français.
2. **`src-tauri/Cargo.toml`** : ajout de `gpx`, `geo`, `sha2`, `uuid`, `hex`, `tauri-plugin-dialog`.
3. **`src-tauri/src/lib.rs`** : déclaration `mod import_gpx;`, `.plugin(tauri_plugin_dialog::init())`, enregistrement des deux commandes dans `invoke_handler`.
4. **`src-tauri/capabilities/default.json`** : ajout de `"dialog:default"`.

### Frontend
5. **`src/stores/traces.ts`** : `useTracesStore` (pattern setup store), types `TraceMetadata` / `TraceStats` / `Point3D`, actions `loadTraces` et `importerGpx`.
6. **`src/stores/ui.ts`** (ou composable) : snackbar mutualisé.
7. **`src/components/Accueil/CircuitsDrawer.vue`** : `@click="importerGpx"` sur le bouton existant, liste pilotée par le store, indicateur de chargement.
8. **`src/components/Accueil/Circuit.vue`** : props refactorisées pour consommer `TraceMetadata` (distance, dénivelé, source).
9. **`src/utils/format.ts`** : helpers de formatage distance/élévation.

---

## 11. 📐 Conventions respectées (cohérence avec `docs/`)

- **Composition API + `<script setup lang="ts">`** partout (cf. `CONTEXT.md`).
- **Setup Store Pinia**, un store par domaine (`traces`, `ui`) (cf. `ARCHITECTURE.md`).
- **Commandes Tauri** comme seule passerelle d'E/S disque (cohérent avec `settings.rs` / `gestionMode.rs`).
- **Résolution des chemins dépendante du mode d'exécution** (cohérent avec le stockage des `config.toml`).
- **Auto-import Vuetify** : pas d'import explicite des composants `v-*`.
- **TypeScript strict** : typage fort des paramètres et retours.
- **Commentaires explicatifs en français**.

---

**Version** : 1.2 — 2026-08-18. Détection automatique des anomalies à l'import : champ `cleaning_status` sur `TraceMetadata` (défaut `"clean"`, `"needs_review"` si anomalies), tolérance `Nettoyage.Cap.toleranceDeg`, blocage de l'édition caméra tant que la trace n'est pas « clean » (vue `/nettoyage`).
**Version** : 1.1 — 2026-07-10. Ajout des commandes `delete_trace` et `update_trace` (persistance favori/affichage), champs `favorite`/`is_displayed` sur `TraceMetadata` (avec `#[serde(default)]` pour la rétrocompatibilité).
**Version** : 1.0 — adapté à l'état du dépôt `VisuGPS2/stage` au 2026-07-08.
