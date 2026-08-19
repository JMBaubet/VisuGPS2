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
│   └── traces/                  # **Un dossier par trace** (le dossier est le discriminant)
│       └── {trace_id}/
│           ├── {filename}.gpx          # GPX (nom d'origine sanitizé)
│           ├── {filename}.gpx.orig     # backup de l'original (si nettoyage)
│           ├── trace.geojson           # LineString GeoJSON
│           ├── keyframes_169.json      # keyframes ratio 16:9
│           ├── keyframes_43.json       # keyframes ratio 4:3
│           ├── cleaning.{phase}.json          # fichier de travail par étape
│           └── cleaning.{phase}.decisions.json # décisions « faux positif » par étape
│
└── EVAL_xxx/                    # Un dossier par mode d'évaluation créé
    ├── config.toml
    ├── config-dev.toml
    ├── traces.json
    └── traces/{trace_id}/
        ├── {filename}.gpx
        ├── trace.geojson
        ├── keyframes_169.json / keyframes_43.json
        └── cleaning.{phase}.json
```

> **Migration automatique** : au premier accès à un mode (point de passage
> `get_mode_dir`), `migrate_mode_storage` déplace les fichiers de l'ancien
> agencement plat (`gpx/`, `geojson/`, `keyframes/`, `cleaning/`) vers les
> dossiers par trace, puis retire les anciens dossiers vides. Idempotente
> (no-op si `{mode}/traces` existe déjà).

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
    "is_displayed": false,
    "cleaning_status": "needs_review",
    "cleaning_phase": "spike"
  }
]
```

**Statut de nettoyage** (`cleaning_status`) : `"clean"` (aucune anomalie détectée ou trace déjà nettoyée), `"needs_review"` (anomalies détectées à l'import, corrections en attente), `"in_progress"` (corrections commencées, fichier de travail présent). Une trace non `"clean"` **n'est pas candidate** à l'édition caméra (la vue `/nettoyage` est présentée à la place).

**Phase de nettoyage** (`cleaning_phase`) : étape du pipeline en cours — `"spike"` (pts hors trace), `"roundabout"` (ronds-points), `"out_and_back"` (aller/retour, étape 3 à venir), ou `""` quand la trace est propre. Posée à l'import (`"spike"` si anomalies) et avancée à chaque **validation d'étape** (`validate_phase`). Tant que l'étape 3 n'est pas implémentée, une trace **reste `needs_review`** même après les étapes 1 et 2.

**Rétrocompatibilité** : les champs `favorite` et `is_displayed` ont `#[serde(default)]`, `cleaning_status` a `#[serde(default = "default_cleaning_status")]` et `cleaning_phase` a `#[serde(default)]` en Rust. Un `traces.json` antérieur se charge sans erreur. En complément, `load_registry` **normalise** toute chaîne vide en `"clean"` et déduit `cleaning_phase` : `""` pour une trace « clean », sinon `"spike"` (re-détection en chaîne).

### `traces/{trace_id}/trace.geojson` — LineString GeoJSON

Feature GeoJSON (LineString) d'une trace, générée à l'import et mise en cache,
dans le dossier de la trace (nom **uniforme** `trace.geojson`).
`properties.id` contient l'UUID pour la liaison avec `TraceMetadata`.
Écriture atomique (tmp + rename). Régénérée depuis le GPX si absente.

### `traces/{trace_id}/keyframes_169.json` / `keyframes_43.json` — Keyframes persistés (édition caméra)

Jeux de keyframes sérialisés en JSON pour la vue d'édition caméra, dans le
dossier de la trace.
**Un fichier par ratio d'écran** : `keyframes_169.json` (16:9) et
`keyframes_43.json` (4:3) — chaque ratio dispose de son propre cadrage
(les viewports de référence sont 1920×1080 et 1440×1080, même hauteur, seul le
champ horizontal diffère). Le ratio d'un jeu sauvegardé est déduit de son champ
`viewport` côté frontend (`saveKeyframes`).
Le contenu est un `KeyframeSet` (type TS, sérialisé par le frontend) :
`trace_id`, `total_distance_m`, `total_duration_ms`, `viewport`, `sample_rate_m`, `keyframes[]`.
Chaque keyframe porte deux champs optionnels :
- `locked` (booléen) : le verrou du segment qui **part de ce keyframe** en mode
  validation (absent = déverrouillé, sans signification sur le dernier keyframe) ;
- `marks` (tableau de nombres) : les **traits bleus** posés en mode validation
  dans ce segment — distances (m) du curseur d'avancement au moment de chaque
  pose (plusieurs possibles). **Persistés** : réaffichés à la réouverture de la
  vue, supprimés quand le segment est verrouillé.
Verrous et marques sont réinitialisés à la régénération des keyframes.
Le backend traite le JSON de manière transparente (`serde_json::Value`), sans validation structurelle côté Rust.
Écriture atomique (tmp + rename). Le dossier de la trace est créé automatiquement à la première sauvegarde.

> **Suppression** : quand une trace est supprimée (`delete_trace`), son **dossier entier**
> `traces/{trace_id}/` est supprimé (GPX, backup `.orig`, GeoJSON, keyframes, nettoyage) —
> la cascade est implicite.

### `traces/{trace_id}/cleaning.{phase}.json` — Fichiers de travail du nettoyage (par étape)

Décisions de correction d'une trace, persistées à chaque **sauvegarde partielle** de la phase (commande `save_cleaning_state`, écriture atomique tmp + rename), dans le dossier de la trace. **Un fichier par phase** (`cleaning.spike.json`, `cleaning.roundabout.json`, `cleaning.out_and_back.json`) : les index de cas sont propres à la version du GPX traitée, donc re-créés à chaque étape. Le GPX reste **intact** tant que la phase n'est pas validée.

```json
{
  "trace_id": "cd9e49cb-…",
  "tolerance_deg": 5.0,
  "phase": "roundabout",
  "cases": [
    {
      "id": "rp1",
      "kind": "roundabout",
      "start_index": 4100,
      "end_index": 4115,
      "apex_indices": [],
      "bearing_delta_deg": 450.0,
      "total_angle_deg": 450.0,
      "suggested_delete_ranges": [],
      "state": "corrected",
      "correction": { "delete_ranges": [[4103, 4112]], "moved_points": [] }
    }
  ]
}
```

- `state` : `"pending"` (à traiter), `"corrected"` (corrigé par l'utilisateur), `"kept"` (faux positif). La **validation de chaque cas est de la responsabilité de l'utilisateur**.
- `correction.delete_ranges` : plages d'index **originaux** à supprimer ; `correction.moved_points` : points dont les coordonnées sont **remplacées** (déplacement géographique).
- Les cas `manual` (« Modification de segment », créés via le bouton éponyme sous la liste) sont persistés comme les autres et peuvent être supprimés **même après validation**.
- La présence d'un fichier pose `cleaning_status = "in_progress"` et `cleaning_phase = phase`.
- À la **validation d'étape** (`validate_phase`), le GPX est remplacé par la version nettoyée (entrée de l'étape suivante), l'original est sauvegardé en `{filename}.gpx.orig` (**une seule fois**, à l'étape 1), les dérivés (geojson, stats, hash) sont régénérés, `cleaning_phase` avance et le fichier de travail de la phase est supprimé. Une étape sans anomalie est **auto-validée** (avancement de phase sans réécriture).

### `traces/{trace_id}/cleaning.{phase}.decisions.json` — Décisions validées par phase (faux positifs)

Persiste, à chaque **validation d'étape** (`validate_phase`), les cas validés **sans modification effective** (conservés tel quel, ou corrigés sans correction) : leur **coordonnée représentative** (apex ou centroïde de la zone) + état. Écriture atomique. Le fichier **survit** à la validation (le fichier de travail, lui, est supprimé) et **n'est effacé qu'au `reset_cleaning`** (ou à la suppression de la trace, avec le dossier).

À la **re-détection** d'une étape déjà validée (retour via le widget de la toolbar), les cas détectés dont la coordonnée représentative est à moins de ~40 m d'une décision `"kept"` sont **automatiquement re-marqués « faux positif »** — l'utilisateur retrouve ses décisions. Exemple :

```json
[
  { "kind": "roundabout", "state": "kept", "lat": 41.58477, "lon": 2.54636 }
]
```

### `config.toml` / `config-dev.toml` — Surcharges de paramètres

- Ne contiennent **que les valeurs modifiées** par rapport au défaut (pas de recopie intégrale).
- `config-dev.toml` est lu en développement (`cfg!(debug_assertions)`), `config.toml` en production.
- Le schéma de référence est `settings.default.toml` (embarqué comme ressource bundlée).

### `settings.default.toml` — Schéma de paramètres

Situé dans `src-tauri/settings.default.toml`, **embarqué dans l'exécutable** via `tauri.conf.json` (`resources`). Lu au démarrage via `resource_dir()` par `init_settings_state`. Définit tous les paramètres avec `description`, `documentation` (Markdown), `type`, `default`, et optionnellement `min`/`max`/`step`/`unit`/`choices`/`critical`/`icon` (icône MDI du drawer).

Ce fichier contient également une **table spéciale `[_meta]`** (placée en tête, avant les tables de paramètres) qui décrit l'organisation du drawer : vues (associées aux noms de routes Vue Router), groupes système communs à toutes les vues, actions (entrées non-paramètres), handlers (catégories à carte dédiée) et libellés/icônes des catégories. Cette table est **exclue du « flatten »** des paramètres (`flatten_settings` ignore la clé `_meta`) et n'est lue que par la commande `get_settings_meta`.

> **Paramètres « cachés »** : un paramètre n'apparaît pas dans le drawer s'il n'est listé dans **aucun** groupe `_meta` (`system.groups` ou `views.<route>.groups`). C'est le cas du groupe `Carte.Vue.*` (centreLat / centreLng / zoom), écrit automatiquement par la carte Accueil à chaque déplacement/zoom et lu à son montage pour restaurer la dernière vue.

## Résolution des chemins (backend)

Les fonctions dans `import_gpx.rs` résolvent les chemins en fonction du mode actif :

```rust
get_mode_dir(app)     → {app_data_dir}/{active_mode}     // créé si absent ; déclenche la migration
get_trace_dir(mode_dir, trace_id)      → {mode_dir}/traces/{trace_id}         // créé si absent
get_trace_gpx_path(mode_dir, trace_id, filename) → {mode_dir}/traces/{trace_id}/{filename}
get_geojson_path(mode_dir, trace_id)  → {mode_dir}/traces/{trace_id}/trace.geojson
get_keyframes_path(mode_dir, trace_id, viewport_aspect) → {mode_dir}/traces/{trace_id}/keyframes_169.json | keyframes_43.json
get_traces_path(mode_dir)             → {mode_dir}/traces.json
get_cleaning_path(mode_dir, trace_id, phase) → {mode_dir}/traces/{trace_id}/cleaning.{phase}.json  (cleaning.rs)
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
- `traces.json`, `gpx/`, `geojson/`, `keyframes/` et `cleaning/` sont propres à chaque mode.
- `config.toml` et `config-dev.toml` sont propres à chaque mode.

Cela permet de tester/démontrer sans polluer l'environnement de production (`OPE`).

## Accès frontend

> ⚠️ Le frontend **n'accède jamais directement** au système de fichiers. Aucune permission `fs:` n'est déclarée dans `capabilities/default.json`.

Tout passe par les commandes Tauri, car **seul le backend connaît le mode d'exécution actif** et donc le bon dossier de stockage. Les stores Pinia (`traces`, `settings`, `app`) encapsulent ces appels.

---

**Dernière mise à jour** : 2026-08-19
