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
│           ├── {filename}.gpx.orig     # backup de l'original (posé par audit_validate)
│           ├── trace.geojson           # LineString GeoJSON
│           ├── keyframes_169.json      # keyframes ratio 16:9
│           └── keyframes_43.json       # keyframes ratio 4:3
│
└── EVAL_xxx/                    # Un dossier par mode d'évaluation créé
    ├── config.toml
    ├── config-dev.toml
    ├── traces.json
    └── traces/{trace_id}/
        ├── {filename}.gpx
        ├── trace.geojson
        └── keyframes_169.json / keyframes_43.json
```

> **Migration automatique** : au premier accès à un mode (point de passage
> `get_mode_dir`), `migrate_mode_storage` déplace les fichiers de l'ancien
> agencement plat (`gpx/`, `geojson/`, `keyframes/`) vers les dossiers par
> trace, puis retire les anciens dossiers vides. Idempotente (no-op si
> `{mode}/traces` existe déjà).
>
> **Purge des artefacts de l'ancien module (D2c)** : toujours dans `get_mode_dir`,
> `cleanup_obsolete_cleaning_files` supprime les fichiers de travail
> `cleaning.*.json` résiduels de tous les dossiers de traces. Idempotente et
> **silencieuse** (D3b — aucun log, aucune erreur remontée). Le dossier hérité
> `cleaning/` de l'ancien agencement plat est supprimé par `migrate_mode_storage`.
> Le module Audit GPX **n'écrit aucun fichier de travail** : ses findings sont
> volatils (décision 6) et vivent uniquement en mémoire dans le store Pinia.

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
    "audit_status": "needs_review"
  }
]
```

**Statut d'audit** (`audit_status`) : `"clean"` (auditée sans anomalie, ou corrections appliquées) ou `"needs_review"` (anomalies détectées à l'import). Une trace non `"clean"` **n'est pas candidate** à l'édition caméra : elle est redirigée vers la vue `/audit`. Le statut est posé à l'import (détection AR + RP sur les points du GPX) et repasse à `"clean"` par `audit_validate`.

**Champs retirés** : `cleaning_status` et `cleaning_phase` ont disparu avec l'ancien module de nettoyage. Les registres qui les contiennent sont traités comme « pré-audit » (voir ci-dessous).

**Rétrocompatibilité et registres pré-audit (D1)** : `favorite` et `is_displayed` ont `#[serde(default)]`, `audit_status` a `#[serde(default = "default_audit_status")]` (valeur `"needs_review"`) — un `traces.json` sans ce champ se charge donc sans erreur. En revanche, un registre au **format pré-audit**, c'est-à-dire contenant la clé `"cleaning_status"`, est **détecté et ignoré** par `load_registry` : la liste retournée est vide et le fichier **n'est jamais réécrit par le chargement**. Les traces concernées disparaissent de l'interface, mais leurs dossiers et fichiers GPX restent **intacts sur disque** (aucune perte de données). Le registre est réécrit au nouveau format au prochain import ; les entrées de l'ancien format ne sont alors plus référencées.

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
> `traces/{trace_id}/` est supprimé (GPX, backup `.orig`, GeoJSON, keyframes) —
> la cascade est implicite.

### Fichiers de travail du module Audit — **aucun**

Le module Audit GPX **n'écrit aucun fichier de travail**. Les findings, la trace de travail
(`AuditPoint[]`) et les enregistrements d'annulation sont **volatils** (décision 6) : ils vivent
uniquement en mémoire, dans le store Pinia `src/stores/audit.ts`, et sont perdus à la sortie de
la vue (`auditStore.reset()` dans `onBeforeRouteLeave`).

Seuls deux effets sont persistés, et uniquement par la commande `audit_validate` : le **GPX
réécrit** (la trace de travail y est écrite telle quelle, invariant C10) et le champ
`audit_status` du registre. Le backup `{filename}.gpx.orig` est posé à cette occasion,
**une seule fois** — un `.orig` existant n'est jamais écrasé.

> **Héritage** : les fichiers `cleaning.{phase}.json` et `cleaning.{phase}.decisions.json` de
> l'ancien module de nettoyage sont supprimés par la purge **D2c** (voir la note de migration
> ci-dessus). Ils ne sont plus lus ni écrits par aucune version du code.

### `config.toml` / `config-dev.toml` — Surcharges de paramètres

- Ne contiennent **que les valeurs modifiées** par rapport au défaut (pas de recopie intégrale).
- `config-dev.toml` est lu en développement (`cfg!(debug_assertions)`), `config.toml` en production.
- Le schéma de référence est `settings.default.toml` (embarqué comme ressource bundlée).

> **Surcharges orphelines** : d'éventuelles surcharges `Nettoyage.*` dans ces deux fichiers —
> namespace de l'ancien module de nettoyage, supprimé du schéma — sont **sans effet** : elles ne
> correspondent à aucun paramètre déclaré et sont ignorées au chargement (aucune erreur, aucun
> avertissement). Elles peuvent être supprimées manuellement si souhaité.

### `settings.default.toml` — Schéma de paramètres

Situé dans `src-tauri/settings.default.toml`, **embarqué dans l'exécutable** via `tauri.conf.json` (`resources`). Lu au démarrage via `resource_dir()` par `init_settings_state`. Définit tous les paramètres avec `description`, `documentation` (Markdown), `type`, `default`, et optionnellement `min`/`max`/`step`/`unit`/`choices`/`critical`/`icon` (icône MDI du drawer).

Ce fichier contient également une **table spéciale `[_meta]`** (placée en tête, avant les tables de paramètres) qui décrit l'organisation du drawer : vues (associées aux noms de routes Vue Router), groupes système communs à toutes les vues, actions (entrées non-paramètres), handlers (catégories à carte dédiée) et libellés/icônes des catégories. Cette table est **exclue du « flatten »** des paramètres (`flatten_settings` ignore la clé `_meta`) et n'est lue que par la commande `get_settings_meta`.

> **Paramètres « cachés »** : un paramètre n'apparaît pas dans le drawer s'il n'est listé dans **aucun** groupe `_meta` (`system.groups` ou `views.<route>.groups`). C'est le cas du groupe `Carte.Vue.*` (centreLat / centreLng / zoom), écrit automatiquement par la carte Accueil à chaque déplacement/zoom et lu à son montage pour restaurer la dernière vue.

## Résolution des chemins (backend)

Les fonctions dans `import_gpx.rs` résolvent les chemins en fonction du mode actif :

```rust
get_mode_dir(app)     → {app_data_dir}/{active_mode}     // créé si absent ; migration + purge D2c
get_trace_dir(mode_dir, trace_id)      → {mode_dir}/traces/{trace_id}         // créé si absent
get_trace_gpx_path(mode_dir, trace_id, filename) → {mode_dir}/traces/{trace_id}/{filename}
get_geojson_path(mode_dir, trace_id)  → {mode_dir}/traces/{trace_id}/trace.geojson
get_keyframes_path(mode_dir, trace_id, viewport_aspect) → {mode_dir}/traces/{trace_id}/keyframes_169.json | keyframes_43.json
get_traces_path(mode_dir)             → {mode_dir}/traces.json
```

> Le module Audit GPX n'introduit **aucun chemin nouveau** : il ne persiste rien en dehors du
> GPX réécrit par `audit_validate` (via `get_trace_gpx_path`) et du registre.

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
- `traces.json` et `traces/` (dossiers par trace : GPX, `.gpx.orig`, geojson, keyframes) sont propres à chaque mode.
- `config.toml` et `config-dev.toml` sont propres à chaque mode.

Cela permet de tester/démontrer sans polluer l'environnement de production (`OPE`).

## Accès frontend

> ⚠️ Le frontend **n'accède jamais directement** au système de fichiers. Aucune permission `fs:` n'est déclarée dans `capabilities/default.json`.

Tout passe par les commandes Tauri, car **seul le backend connaît le mode d'exécution actif** et donc le bon dossier de stockage. Les stores Pinia (`traces`, `settings`, `app`) encapsulent ces appels.

---

**Dernière mise à jour** : 2026-09-12
