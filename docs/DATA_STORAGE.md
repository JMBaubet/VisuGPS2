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
│           ├── audit.json              # archive de l'audit (anomalies + traitements)
│           ├── multiride.json          # description des passages multiples (contrat)
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
        ├── audit.json
        ├── multiride.json
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
> Le module Audit GPX n'écrit plus de fichiers de travail par phase, mais **une
> archive d'audit unique** par trace : `traces/{trace_id}/audit.json` (voir
> « Archive d'audit » ci-dessous).

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
    "audit_status": "needs_review",
    "audit_archived": false,
    "multiride_status": "pending"
  }
]
```

**Statut d'audit** (`audit_status`) : `"clean"` (auditée sans anomalie, ou corrections appliquées) ou `"needs_review"` (anomalies détectées à l'import). Une trace non `"clean"` **n'est pas candidate** à l'édition caméra : elle est redirigée vers la vue `/audit`. Le statut est posé à l'import (détection AR + RP sur les points du GPX) et repasse à `"clean"` par `audit_validate`.

**Archivage de l'audit** (`audit_archived`) : `true` quand un audit **appliqué** a laissé une archive dans le dossier de la trace (`audit.json`). C'est ce drapeau, lu par la carte du circuit, qui **rend** le bouton « Voir les anomalies de la source » (anomalies et traitements consultables) : il est **absent** tant qu'il est faux — il n'existe plus d'état gris informatif. Posé par `audit_validate`, en même temps que `"clean"` ; absent des registres antérieurs → `false` (ces audits n'ont pas laissé d'archive, et les corrections ne sont pas restituables).

**Statut des passages multiples** (`multiride_status`) : `"none"` (détection jouée, aucune portion répétée), `"pending"` (au moins un passage reste à valider — **l'édition caméra est fermée**) ou `"validated"` (passages validés). Il vaut `null` pour une trace dont la détection n'a pas été jouée : la valeur est **permissive**, la barrière ne s'appliquant qu'aux traces détectées depuis l'introduction du module. Posé par `multiride_detect` (`none`/`pending`) et par `multiride_validate` (`validated`) ; les ajustements (`multiride_merge_segment`, `multiride_toggle_fp`, `multiride_reset`) le laissent **inchangé**. Comme `audit_archived`, ce champ évite à la carte du circuit de lire un fichier pour connaître l'état de la trace.

**Champs retirés** : `cleaning_status` et `cleaning_phase` ont disparu avec l'ancien module de nettoyage. Les registres qui les contiennent sont traités comme « pré-audit » (voir ci-dessous).

**Rétrocompatibilité et registres pré-audit (D1)** : `favorite`, `is_displayed` et `audit_archived` ont `#[serde(default)]`, `audit_status` a `#[serde(default = "default_audit_status")]` (valeur `"needs_review"`) — un `traces.json` sans ces champs se charge donc sans erreur. En revanche, un registre au **format pré-audit**, c'est-à-dire contenant la clé `"cleaning_status"`, est **détecté et ignoré** par `load_registry` : la liste retournée est vide et le fichier **n'est jamais réécrit par le chargement**. Les traces concernées disparaissent de l'interface, mais leurs dossiers et fichiers GPX restent **intacts sur disque** (aucune perte de données). Le registre est réécrit au nouveau format au prochain import ; les entrées de l'ancien format ne sont alors plus référencées.

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
> `traces/{trace_id}/` est supprimé (GPX, backup `.orig`, archive d'audit, GeoJSON, keyframes) —
> la cascade est implicite.

### `traces/{trace_id}/audit.json` — Archive d'audit

État complet du travail d'audit, écrit **au fil des actions** (avenant à la décision 6 : les
findings ne sont plus volatils). Sans cette archive, ni la reprise d'une session interrompue ni
la consultation d'un audit appliqué ne seraient possibles.

**Quand le fichier est écrit** (commande `audit_save_state`, écriture **atomique** tmp + rename) :

| Événement | `validated` écrit |
|---|---|
| Détection initiale (`audit_run_detection` par la vue) | `false` |
| Chaque traitement : suppression, routage, faux positif, retrait du faux positif, annulation | `false` |
| Validation (`audit_validate`), après la réécriture du GPX | `true` |

Les **aperçus** — réglage continu des curseurs de suppression ou de routage — n'écrivent
**jamais** : seuls les traitements effectifs sont archivés.

**Format** (JSON en camelCase, miroir TS `AuditArchive` dans `src/stores/audit.ts`) :

```json
{
  "version": 1,
  "traceId": "cd9e49cb-40fb-43a6-896e-ef2fdde357de",
  "updatedAt": "2026-09-16T13:54:01Z",
  "validated": false,
  "params": { "consolM": 0.5, "tolDeg": 20.0, "pairM": 50.0,
              "maxpairs": 5, "segM": 200.0, "closeM": 15.0, "angleDeg": 270 },
  "totalDistanceM": 45213.4,
  "points": [ { "id": 1, "lat": 49.03, "lon": 4.03, "ele": 83.0 } ],
  "findings": [ { "id": "ar-1", "kind": "ar", "status": "corrected",
                  "correction": "delete", "undo": { }, "zoneIds": [12, 13] } ]
}
```

- `points` est la **trace de travail** : elle porte l'espace d'index des findings (invariant C4)
  et permet de restituer la carte, la liste et les zones d'anomalie **sans réexécuter la
  détection**.
- `findings` porte les statuts (`pending` / `corrected` / `fp`) et les corrections, ainsi que les
  **enregistrements d'annulation** — ces derniers conservent les points supprimés et le tracé ORS
  remplaçant, seule source du « avant / après » lors d'une consultation.
- Les **paramètres du détecteur** voyagent avec l'archive : aperçus et éléments de rendu de la
  carte sont reconstruits à l'identique.

**Lecture** (commande `audit_load_archive`) : **tolérante**. Archive absente, illisible, d'une
version de format inconnue ou rattachée à une autre trace → `null`, et le module retombe sur la
détection, qui reste la source de vérité. Aucune erreur n'est remontée à l'utilisateur.

**Écrasement** : le fichier reflète toujours le **dernier** état écrit (pas d'historique de
versions, pas de fusion). Il n'est supprimé que par la suppression de la trace.

**Effets persistés au total** par le module Audit : le **GPX réécrit** (la trace de travail y est
écrite telle quelle, invariant C10), les champs `audit_status` et `audit_archived` du registre, et
cette archive. Le backup `{filename}.gpx.orig` est posé par `audit_validate`,
**une seule fois** — un `.orig` existant n'est jamais écrasé.

> **Héritage** : les fichiers `cleaning.{phase}.json` et `cleaning.{phase}.decisions.json` de
> l'ancien module de nettoyage sont supprimés par la purge **D2c** (voir la note de migration
> ci-dessus). Ils ne sont plus lus ni écrits par aucune version du code.

### `traces/{trace_id}/multiride.json` — Description des passages multiples

Portions de trace **parcourues plusieurs fois** : aller-retour sur un tronçon,
reconnaissance repassant sur une section, boucle locale. Le fichier est le
**contrat de sortie** du module Multiride — il sera consommé par la Visualisation
—, et sert en même temps d'état de travail à la vue `/multiride`.

**Quand le fichier est écrit** (écriture **atomique**, tmp + rename) :

| Événement | Effet |
|---|---|
| Détection (`multiride_detect`) | État neuf, `valide: false` |
| Fusion, faux positif (`multiride_merge_segment`, `multiride_toggle_fp`) | État ajusté, `valide` **conservé** |
| Réinitialisation (`multiride_reset`) | Détection rejouée à l'identique, `valide` conservé |
| Validation (`multiride_validate`) | `valide: true` |

**Format** : une `FeatureCollection` GeoJSON à la nomenclature de la
spécification — `properties` global porte le contexte de la trace, les paramètres
actifs, les compteurs d'ajustements et une note expliquant le format ; **une
Feature par emprunt**, dont la géométrie est un `LineString` à **exactement deux
coordonnées** (les bornes `[lon, lat]` d'entrée et de sortie).

```json
{
  "type": "FeatureCollection",
  "properties": {
    "version": 1,
    "trace_id": "cd9e49cb-40fb-43a6-896e-ef2fdde357de",
    "valide": false,
    "source": "CalpePhoto.gpx",
    "date": "2026-09-16T13:54:01Z",
    "trace": { "point_count": 2207, "length_km": 50.54 },
    "parametres": { "tolerance_m": 10.0, "longueur_min_m": 100.0,
                    "pas_echantillonnage_m": 4.0, "fusion_references_m": 100.0,
                    "pas_plafonne": false },
    "ajustements": { "fusions_manuelles": 0, "faux_positifs_exclus": 0 },
    "note": "Chaque Feature représente un passage. La géométrie LineString ne contient que les 2 points bornes (entrée, sortie). Pour reconstituer la portion de trace, joindre point_entree / point_sortie avec la trace d'origine."
  },
  "features": [
    {
      "type": "Feature",
      "properties": {
        "segment": 1, "passage": 1, "sens": "reference",
        "faux_positif": false,
        "point_entree": 240, "point_sortie": 842,
        "km_entree": 7.62, "km_sortie": 23.06,
        "longueur_km": 15.44, "fusionne": false
      },
      "geometry": { "type": "LineString",
                    "coordinates": [[7.49912, 43.77584], [7.49278, 43.79012]] }
    }
  ]
}
```

- `sens` vaut `reference` (premier emprunt du segment), `aller` (même sens que
  celle-ci) ou `retour` (sens inverse) ;
- `point_entree` / `point_sortie` sont des **numéros de points du GPX d'origine**
  (1-based) : c'est la clé de jointure avec la trace, et non un index de la trace
  nettoyée — un point écarté au dédoublonnage ne décale donc pas la
  correspondance ;
- `km_entree` / `km_sortie` sont des distances cumulées **le long de la trace**
  (km), mesurées dans la même métrique que `trace.length_km` ;
- `faux_positif` marque les emprunts d'un segment écarté par l'utilisateur :
  ils **restent** dans le fichier (c'est l'export qui les exclut), sinon une
  réouverture de la vue ne pourrait plus distinguer un segment écarté d'un
  segment ordinaire ;
- `fusionne` marque les emprunts d'un segment ayant subi une fusion manuelle.

**Trois ajouts au format de la spécification** : `version` et `trace_id` (version
du format et rattachement à la trace, sans quoi la lecture ne pourrait pas
refuser un fichier étranger), `valide` (levée de la barrière) et `faux_positif`
(conservation des segments écartés, cf. ci-dessus).

**Lecture** (commande `multiride_load`) : **tolérante** — fichier absent,
illisible, d'une version inconnue ou rattaché à une autre trace → `null`, et la
vue relance la détection, qui reste la source de vérité. Aucune erreur n'est
remontée à l'utilisateur.

> **Suppression** : le fichier disparaît avec le dossier de la trace
> (`delete_trace`), comme les autres artefacts.

### `config.toml` / `config-dev.toml` — Surcharges de paramètres

- Ne contiennent **que les valeurs modifiées** par rapport au défaut (pas de recopie intégrale).
- `config-dev.toml` est lu en développement (`cfg!(debug_assertions)`), `config.toml` en production.
- Le schéma de référence est `settings.default.toml` (embarqué comme ressource bundlée).

> **Surcharges orphelines** : d'éventuelles surcharges `Nettoyage.*` dans ces deux fichiers —
> namespace de l'ancien module de nettoyage, supprimé du schéma — sont **sans effet** : elles ne
> correspondent à aucun paramètre déclaré et sont ignorées au chargement (aucune erreur, aucun
> avertissement). Elles peuvent être supprimées manuellement si souhaité.
>
> **Clés de licence (2026-09-12)** : les clés OpenRouteService ont été déplacées de
> `Audit.OpenRouteService.clePrimaire` / `cleSecondaire` vers le groupe système `Systeme.Key`
> (`Systeme.Key.openRouteServiceClePrimaire` / `openRouteServiceCleSecondaire`), aux côtés de
> `Systeme.Key.mapBox` — chemin, lui, **inchangé**. Une surcharge `Audit.OpenRouteService.*`
> présente dans `config.toml` / `config-dev.toml` devient donc **orpheline** et sans effet : le
> routage OpenRouteService réclame une ressaisie des deux clés (les valeurs restent lisibles en
> clair dans le fichier de surcharge, elles n'y sont pas supprimées).

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
archive_path(mode_dir, trace_id)      → {mode_dir}/traces/{trace_id}/audit.json
file_path(mode_dir, trace_id)         → {mode_dir}/traces/{trace_id}/multiride.json
get_traces_path(mode_dir)             → {mode_dir}/traces.json
```

> Le module Audit GPX utilise **un seul chemin nouveau** — `archive_path`, l'archive d'audit
> décrite plus haut — en plus du GPX réécrit par `audit_validate` (via `get_trace_gpx_path`) et
> du registre. Le module Multiride en ajoute un second : `file_path`, la description des
> passages multiples (elle aussi dans le dossier de la trace).

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
- `traces.json` et `traces/` (dossiers par trace : GPX, `.gpx.orig`, archive d'audit, geojson, keyframes) sont propres à chaque mode.
- `config.toml` et `config-dev.toml` sont propres à chaque mode.

Cela permet de tester/démontrer sans polluer l'environnement de production (`OPE`).

## Accès frontend

> ⚠️ Le frontend **n'accède jamais directement** au système de fichiers. Aucune permission `fs:` n'est déclarée dans `capabilities/default.json`.

Tout passe par les commandes Tauri, car **seul le backend connaît le mode d'exécution actif** et donc le bon dossier de stockage. Les stores Pinia (`traces`, `settings`, `app`) encapsulent ces appels.

---

**Dernière mise à jour** : 2026-09-17
