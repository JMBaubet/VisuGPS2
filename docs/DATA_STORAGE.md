# Stockage des données

> Schéma complet du stockage disque de VisuGPS2.
> Comprendre où vit chaque donnée et comment elle est résolue.

## Racine de données

Toutes les données persistantes vivent dans le `app_data_dir` de Tauri, résolu via `app.path().app_data_dir()`.

| OS | Chemin |
|---|---|
| **macOS** | `~/Library/Application Support/com.jean-marc.baubet.visugps2/` |
| **Windows** | `%APPDATA%\com.jean-marc.baubet.visugps2\` |
| **Linux** | `~/.local/share/com.jean-marc.baubet.visugps2/` |

> L'identifier `com.jean-marc.baubet.visugps2` est défini dans `tauri.conf.json` (`identifier`).

## Arborescence complète

```
{app_data_dir}/
├── .env                         # Mode actif (APP_ENV_DEV / APP_ENV_PROD)
├── ModeExe.toml                 # Registre des modes d'exécution
├── visugps2_master_key          # (prod uniquement) Clé maîtresse chiffrée, base64 (keyring OS)
│
├── OPE/                         # Mode "OPE" (toujours présent, non supprimable)
│   ├── config.toml              # Surcharges utilisateur en PROD
│   ├── config-dev.toml          # Surcharges utilisateur en DEV
│   ├── traces.json              # Registre des traces importées (Vec<TraceMetadata>)
│   ├── gpx/                     # Fichiers GPX copiés (nom sanitizé + unique si conflit)
│   │   ├── trace1.gpx
│   │   └── trace2.gpx
│   ├── geojson/                 # LineString GeoJSON (un fichier par trace)
│   │   ├── {uuid}.geojson
│   │   └── ...
│   └── keyframes/               # Keyframes persistés (vue d'édition caméra)
│       ├── {uuid}.json
│       └── ...
│
└── EVAL_xxx/                    # Un dossier par mode d'évaluation créé
    ├── config.toml
    ├── config-dev.toml
    ├── traces.json
    ├── gpx/*.gpx
    ├── geojson/{uuid}.geojson
    └── keyframes/{uuid}.json
```

## Détail des fichiers

### `.env` — Mode actif

```env
APP_ENV_DEV=OPE
APP_ENV_PROD=OPE
```

- `APP_ENV_DEV` : mode actif en développement (`cfg!(debug_assertions)`).
- `APP_ENV_PROD` : mode actif en production (build release).
- Créé automatiquement avec `OPE`/`OPE` si absent.
- **Ne pas confondre** avec le `.env.local` à la racine du repo (variables `VITE_`/`TAURI_` pour le dev frontend).

### `ModeExe.toml` — Registre des modes

```toml
[[Modes]]
nom = "OPE"
descrition = "Mode opérationnel"
création = "2026-06-01"
révision = "2026-06-01"

[[Modes]]
nom = "EVAL_test"
descrition = "Mode de test"
création = "2026-07-01"
révision = "2026-07-05"
```

> Note : les champs `création`/`révision` conservent leurs accents via `#[serde(rename)]` en Rust.

### `traces.json` — Registre des traces

Tableau JSON de `TraceMetadata`, sérialisé en pretty-print (indentation 2 espaces). **Écriture atomique** via `save_registry` (fichier `.tmp` puis `rename`).

```json
[
  {
    "id": "cd9e49cb-40fb-43a6-896e-ef2fdde357de",
    "name": "Itinéraire vélo de Reims",
    "source": "Strava",
    "source_url": "https://www.strava.com/routes/...",
    "activity_type": "cycling",
    "filename": "Itine_raire_ve_lo_de_Reims.gpx",
    "import_date": "2026-07-10T13:00:53.133053+00:00",
    "stats": {
      "start_point": { "lat": 49.03, "lon": 4.03, "alt": 83.0 },
      "end_point": { "lat": 49.03, "lon": 4.03, "alt": 83.0 },
      "distance_m": 45213.4,
      "positive_elevation_m": 320.5,
      "negative_elevation_m": 318.2,
      "alt_min_m": 72.0,
      "alt_max_m": 215.0,
      "points_count": 1240,
      "duration_s": 5400.0
    },
    "hash": "sha256:91a5d3aa7185523b717b4169884d6ee48afa613cd10c0a9bdae75d18a900becb",
    "favorite": false,
    "is_displayed": false
  }
]
```

**Rétrocompatibilité** : les champs `favorite` et `is_displayed` ont `#[serde(default)]` en Rust. Un `traces.json` antérieur (sans ces champs) se charge avec `false`/`false` sans erreur.

### `geojson/{uuid}.geojson` — LineString GeoJSON

Feature GeoJSON (LineString) d'une trace, générée à l'import et mise en cache.
Le fichier est nommé d'après l'UUID de la trace (`{id}.geojson`).
`properties.id` contient l'UUID pour la liaison avec `TraceMetadata`.
Écriture atomique (tmp + rename).

### `keyframes/{uuid}.json` — Keyframes persistés (édition caméra)

Jeux de keyframes sérialisés en JSON pour la vue d'édition caméra.
Un fichier par trace, nommé d'après l'UUID de la trace (`{trace_id}.json`).
Le contenu est un `KeyframeSet` (type TS, sérialisé par le frontend) :
`trace_id`, `total_distance_m`, `total_duration_ms`, `viewport`, `sample_rate_m`, `keyframes[]`.
Le backend traite le JSON de manière transparente (`serde_json::Value`), sans validation structurelle côté Rust.
Écriture atomique (tmp + rename). Le dossier `keyframes/` est créé automatiquement à la première sauvegarde.

> **Suppression en cascade** : quand une trace est supprimée (`delete_trace`), le fichier `keyframes/{uuid}.json` associé est supprimé en même temps que le `.gpx` et le `.geojson`.

### `config.toml` / `config-dev.toml` — Surcharges de paramètres

- Ne contiennent **que les valeurs modifiées** par rapport au défaut (pas de recopie intégrale).
- `config-dev.toml` est lu en développement (`cfg!(debug_assertions)`), `config.toml` en production.
- Le schéma de référence est `settings.default.toml` (embarqué comme ressource bundlée).

### `settings.default.toml` — Schéma de paramètres

Situé dans `src-tauri/settings.default.toml`, **embarqué dans l'exécutable** via `tauri.conf.json` (`resources`). Lu au démarrage via `resource_dir()` par `init_settings_state`. Définit tous les paramètres avec `description`, `documentation` (Markdown), `type`, `default`, et optionnellement `min`/`max`/`step`/`unit`/`choices`/`critical`/`icon` (icône MDI du drawer).

Ce fichier contient également une **table spéciale `[_meta]`** (placée en tête, avant les tables de paramètres) qui décrit l'organisation du drawer : vues (associées aux noms de routes Vue Router), groupes système communs à toutes les vues, actions (entrées non-paramètres), handlers (catégories à carte dédiée) et libellés/icônes des catégories. Cette table est **exclue du « flatten »** des paramètres (`flatten_settings` ignore la clé `_meta`) et n'est lue que par la commande `get_settings_meta`.

## Résolution des chemins (backend)

Les fonctions privées dans `import_gpx.rs` résolvent les chemins en fonction du mode actif :

```rust
get_mode_dir(app)          → {app_data_dir}/{active_mode}     // créé si absent
get_gpx_dir(mode_dir)      → {mode_dir}/gpx                   // créé si absent
get_geojson_dir(mode_dir)  → {mode_dir}/geojson               // créé si absent
get_geojson_path(mode_dir, trace_id) → {mode_dir}/geojson/{trace_id}.geojson
get_keyframes_dir(mode_dir) → {mode_dir}/keyframes             // créé si absent
get_keyframes_path(mode_dir, trace_id) → {mode_dir}/keyframes/{trace_id}.json
get_traces_path(mode_dir)  → {mode_dir}/traces.json
```

Le mode actif est déterminé par `gestionMode::read_active_mode(app_data_dir, is_dev)` qui lit `.env`.

## Sécurité des secrets

Les paramètres de type `secret` sont chiffrés en **AES-256-GCM** avant écriture dans le TOML.

| Contexte | Clé maîtresse |
|---|---|
| **Dev** (`cfg!(debug_assertions)`) | Clé statique `dev_master_key_32_bytes_long_!!!` (évite les popups keyring macOS sur app non-signée). |
| **Prod** | Générée aléatoirement (32 octets), stockée dans le trousseau système (`keyring`, entry `visugps2_master_key`/`system`), encodée Base64. |

**Format chiffré** : nonce aléatoire (12 octets) + ciphertext, combinés en Base64.

Côté frontend, les secrets arrivent toujours masqués (`********`) via `get_settings`. Seule la commande `get_setting_value` déchiffre (usage interne, ex. token Mapbox).

## Isolation par mode

Changer de mode d'exécution isole **complètement** les données :
- `traces.json`, `gpx/`, `geojson/` et `keyframes/` sont propres à chaque mode.
- `config.toml` et `config-dev.toml` sont propres à chaque mode.

Cela permet de tester/démontrer sans polluer l'environnement de production (`OPE`).

## Accès frontend

> ⚠️ Le frontend **n'accède jamais directement** au système de fichiers. Aucune permission `fs:` n'est déclarée dans `capabilities/default.json`.

Tout passe par les commandes Tauri, car **seul le backend connaît le mode d'exécution actif** et donc le bon dossier de stockage. Les stores Pinia (`traces`, `settings`, `app`) encapsulent ces appels.

---

**Dernière mise à jour** : 2026-08-05
