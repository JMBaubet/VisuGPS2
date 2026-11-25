# Contrat d'interface — API hermes et réveil

> Version 1.10 — 23 novembre 2026
> Fichier : ~/notes/hermes-api-contract.md
> Référence : sequencement.md v1.10, code hermes-api du 23 novembre 2026
> Public : app M1 (Mac), PC Windows, app admin, scripts de test

Ce document définit l'interface entre l'application cliente (Mac M1 /
PC Windows), l'app admin, et le serveur `hermes`.

**Changements v1.10** :

- Nouveaux états individuels de dataset : `initializing`, `building`,
  `ready`, `unavailable`, `degradedCommunes`, `degradedTunnels`,
  `degradedAux`.
- Nouvel objet `artifacts` : `pbf`, `ors`, `communes`, `tunnels`, `mm`,
  avec les états `pending|building|ready|failed`.
- Nouvelles routes du dataset servi : `GET /serving`,
  `PUT /serving/{dataset_id}`, `DELETE /serving`.
- Documentation contractuelle des routes auxiliaires :
  `GET /datasets/{id}/aux` et `POST /datasets/{id}/aux/prepare`.
- `skipMm` est supprimé ; toute requête qui l’envoie reçoit HTTP 422.
- MM ne passe à `ready` que sur marqueur explicite de fin ; l’existence du
  dossier `mm/` ne suffit pas.
- Le champ `color` des polygones est supprimé du contrat.
- L’URL d’API est celle de l’interface privée Tailscale, jamais
  `127.0.0.1` sauf déploiement `--bind-local`.

**Changements v1.9** (par rapport à v1.8) — clôture des candidats T57 :

- **T57-I** : refonte complète de l'UI admin (voir §11 réécrit) :
  - Vue unifiée 3 colonnes (Catalogue | Carte | Régions) sur `/admin/`
  - Pico.css + thème clair/sombre/auto (toggle + `prefers-color-scheme`)
  - Toolbar permanente + modal `<dialog>` natif (remplace la page plein écran)
  - Cache IndexedDB du catalogue Geofabrik (TTL 24 h, ~3 Mo)
  - Drapeaux `flagcdn.com`, icônes `@mdi/font`
  - Traduction FR des continents/pays via `Intl.DisplayNames`
  - Filtre pays persistant (défaut `France, Italie, Suisse, Espagne`)
  - Garde-fou téléchargement (limite 500 Mo configurable)
  - URLs `/admin/regions` et `/admin/geofabrik` conservées (redirection JS
    vers `/admin/?focus=…`)
- **T57-H** : `created_at` des **nouveaux** manifests au format
  `YYYY-MM-DDTHH:MM:SSZ` (secondes + Z), comme `updated_at` de
  `regions.json`. Les anciens manifests (`isoformat()` avec µs + offset
  `+00:00`) restent **lisibles** par le client — voir §1.2.
- **T57-B, C, D, E, F** : décisions **backend sans impact sur le contrat
  client**. Détails dans `hermes.md` §8 (v1.9).
- **T57-A, T57-G** : déjà documentés en v1.8.

**Changements v1.8** (par rapport à v1.7) — implémentation des décisions
T50–T56 (étape D) :

- `GET /geofabrik/regions` : **enveloppe** `{fetched_at, regions: [...]}`
  (E6), **11 champs** par région (E1–E4), **state à 4 valeurs** (E3),
  `?refresh=true` (E5), `size`/`last_update` obtenus par HEAD lazy (E7).
- `GET /api/v1/regions` : ajout de `geofabrik_id` et `updated_at` (E13).
- `POST /api/v1/regions` : validation `id` = `^[a-zA-Z0-9_-]+$` + normalisation
  lowercase (E9/C2), `Lock` asyncio anti-race (C3).
- `PUT /api/v1/regions/{id}` : `affected_datasets` = `[{id, status, stale}]`
  (E15), `bbox` recalculée (E16).
- `POST /api/v1/datasets` : XOR neutre (messages support), 3 limites T53,
  pas de mode `bbox`.
- `GET /datasets` et `GET /datasets/{id}` : `stale` calculé (T55), mode `zones`
  → `stale=false` (Q5.2).
- Auth admin : `X-Admin-Token` (header), UI web utilise `localStorage` (E17).
- **Nouveau §11** : UI admin (E17–E18).
- Correctifs UI implémentation : `id` local dans le bouton « Mettre à jour »
  (C4), `disabled` anti double-clic.
- Amendements v1.8 : E1–E18 + C1–C4.

**Changements v1.7** (par rapport à v1.6) :
- `POST /datasets` : suppression du mode `bbox`, ajout du mode `zones`
  (T50, T52). Payload = `regions` XOR `zones`.
- Nouveaux endpoints CRUD `/polygons` (T54).
- Nouveaux endpoints admin `/regions` (T55) et `/geofabrik/regions`
  (T56).
- Authentification admin par token `X-Admin-Token` (T55).
- Champ `stale` dans `GET /datasets` (T55).
- Section « Validation géométrique des polygones » (§8).
- Section « Limites de construction » (T53, §10).
- Section « Codes d'erreur HTTP » étendue.
- Accents corrigés sur `GET /regions`.

**Changements v1.6** (par rapport à v1.5) :
- Découpage des itinéraires côté M1 (T47).
- Note `maximum_distance = 100 km` conservée comme garde-fou.
- Note « ORS non redémarré après veille ».
- Note « reprise ORS post-restart ~70 s » (T46).
- Codes HTTP exacts, statuts de dataset, cycle de vie complet type.

---

## 1. API Contrôleur (port 9000)

- URL de base production : `http://100.118.62.10:9000/api/v1`
- URL de base de la machine de développement : utiliser son IP Tailscale
  courante, par exemple `http://100.118.62.12:9000/api/v1`.
- `127.0.0.1` ne fonctionne que si l’API a été installée avec
  `--bind-local`. Ne pas l’utiliser comme adresse par défaut de test.
- Format : JSON
- Préfixe : `/api/v1` (obligatoire)
- Pas d'authentification pour les endpoints M1 (réseau privé Tailscale)
- Authentification admin : header `X-Admin-Token` (voir §9)

### 1.1 Keep-alive et Ping

**`GET /ping`**

- Réponse : `200` + `{"status": "ok"}`
- Rôle : signaler au serveur que l'application est ouverte (toutes les
  30 secondes). Empêche la mise en veille automatique.
- Recommandé tant que l'app M1 est ouverte.

```bash
curl -s http://10.15.58.131:9000/api/v1/ping
# {"status":"ok"}
```

### 1.2 Gestion des Datasets

#### `GET /datasets`

- Réponse : `200` + tableau JSON de manifest(s)
- Champs : `id`, `regions`, `zones`, `created_at`, `status`, `state`,
  `stateLabel`, `phase`, `artifacts`, `runId`, `size_bytes`, `ors_version`,
  `last_log_tail` (si le PBF/ORS est indisponible), `stale`.

`state` est l’état de référence pour l’orchestrateur :
`initializing`, `building`, `ready`, `unavailable`, `degradedCommunes`,
`degradedTunnels`, `degradedAux`. Le champ historique `status` reste exposé
avec le mapping `preparing|ready|failed`. `artifacts` expose séparément
`pbf`, `ors`, `communes`, `tunnels` et `mm`.

Exemple historique (dataset créé **avant v1.9**, format `isoformat()`) :
```json
[
  {
    "id": "provence-alpes-cote-dazur",
    "regions": ["provence-alpes-cote-dazur"],
    "zones": null,
    "created_at": "2026-11-04T16:27:00+00:00",
    "status": "ready",
    "size_bytes": 1748992918,
    "ors_version": "9.9.0",
    "stale": false
  }
]
```

Les manifests actuels ajoutent `state`, `stateLabel`, `phase`, `artifacts`,
`runId` et `stateUpdatedAt` à ces champs historiques.

Exemple (dataset créé **à partir de v1.9**, format uniformisé) :
```json
[
  {
    "id": "mon-dataset",
    "regions": ["monaco"],
    "zones": null,
    "created_at": "2026-11-08T16:19:29Z",
    "status": "ready",
    "size_bytes": 133000000,
    "ors_version": "9.9.0",
    "stale": false
  }
]
```

**Note `created_at` (T57-H, v1.9)** : deux formats coexistent :
- **Anciens datasets** (`isoformat()` Python) : `2026-11-04T16:27:00+00:00`
  (microsecondes + offset, ex. `2026-11-04T16:27:00.123456+00:00`)
- **Nouveaux datasets** (v1.9+) : `2026-11-08T16:19:29Z`
  (`strftime("%Y-%m-%dT%H:%M:%SZ")`, secondes + Z)

Le client **DOIT** utiliser un parser ISO 8601 robuste (`datetime.fromisoformat`
Python, `new Date()` JS, etc.), pas une comparaison string naïve. Le serveur
lui-même utilise `datasets._parse_iso()` qui accepte les deux formats.

- `stale: true` : au moins une région du dataset a été mise à jour
  (via `PUT /regions/{id}`) après sa création, OU une région du dataset
  n'existe plus dans `regions.json` (supprimée via `DELETE /regions/{id}`).
  La reconstruction est à la charge du client (voir §4, cycle de vie).
- **Mode `zones`** (Q5.2, v1.8) : `stale=false` toujours (pas d'info
  fiable sur les régions sous-jacentes utilisées pour le clipping).
  Reconstruction déclenchée par le client quand il le souhaite.

#### `GET /datasets/{id}`

- Réponse : `200` + manifest, ou `404` si dataset introuvable
- Si `state: "unavailable"`, `last_log_tail` contient les 50 dernières
  lignes utiles (log contrôleur + journal ORS).
- `status` reste un champ de compatibilité : utiliser `state` pour toute
  nouvelle logique.
- Inclut `stale` (même calcul que `GET /datasets`)

#### `POST /datasets`

Lance la préparation d'un dataset en arrière-plan.

**Payload** (XOR strict, T52 — le mode `bbox` est supprimé) :

Mode `regions` (merge des PBF entiers) :
```json
{"name": "mon-dataset", "regions": ["provence-alpes-cote-dazur", "italy-nord-ovest"]}
```

Mode `zones` (clipping osmium, T50) :
```json
{"name": "mon-dataset", "zones": ["ma-zone-nice", "ma-zone-marseille"]}
```

- `name` : optionnel, max 64 chars, regex `^[a-zA-Z0-9_-]+$`
  (si absent, généré `ds-<timestamp>-<slug>`)
- `regions` : liste de clés de `regions.json`
- `zones` : liste d'IDs de polygones custom (créés via `POST /polygons`)
- **Pas de mixte `regions` + `zones`** (T52)
- **Pas de `bbox`** (T52)

**Réponses :**

| HTTP | Cas | Corps |
|---|---|---|
| `202` | Accepté, build lancé | `{"id":"...","status":"preparing","state":"initializing","stateLabel":"Initialisation","phase":"pbf","runId":"..."}` |
| `400` | Payload invalide, PBF manquant, zone manquante, limite dépassée (T53), XOR violé | `{"detail": "..."}` |
| `409` | Un build est déjà en cours | `{"detail": "build déjà en cours: <id>"}` |

**Messages d'erreur neutres + support (Q2.1 ter, v1.8)** :

Le handler `create_dataset` vérifie le XOR **avant** d'appeler les résolveurs.
Les messages ont deux parties : fait factuel + invitation support.

| Cas | `detail` |
|---|---|
| `{}` ou `{"name":"x"}` | `"Aucun périmètre fourni (ni région ni zone). Si vous venez de sélectionner un périmètre dans l'application, un problème inattendu est survenu : réessayez, puis signalez-le au support."` |
| `{"regions": []}` seul | `"La liste des régions est vide. Si vous venez de sélectionner une région dans l'application, un problème inattendu est survenu : réessayez, puis signalez-le au support."` |
| `{"zones": []}` seul | `"La liste des zones est vide. Si vous venez de sélectionner une zone dans l'application, un problème inattendu est survenu : réessayez, puis signalez-le au support."` |
| `{"regions": [...], "zones": [...]}` (les deux non vides) | `"Régions et zones ne peuvent pas être fournies ensemble. Si vous n'avez sélectionné qu'un seul type de périmètre dans l'application, un problème inattendu est survenu : réessayez, puis signalez-le au support."` |

Les `[]` seuls sont traités comme « absent » (Q2.1 bis) : `{"regions": ["paca"], "zones": []}` → mode `regions`, `zones` ignoré.

**Limites** (T53, voir §10) : nombre de régions, surface totale,
nombre de zones. Dépassement → `400` avec messages factuels :
- `"trop de zones: N (max M)"`
- `"trop de régions: N (max M)"`
- `"surface totale N km² (max M km²)"`

**Durée** :
- mode `regions` : ~12 min (1 région) à ~26 min (2 régions, 2 profils).
  Mesuré jusqu'à ~9 min sur système chaud (rebuild PACA D.6).
- mode `zones` : quelques secondes à quelques minutes (clipping
  osmium + build ORS sur dataset réduit). Mesuré ~80 s pour Nice+Monaco.

**Le client DOIT interroger `GET /datasets/{id}` pour suivre
l'avancement.** Un POST réussi n'indique que le démarrage, pas la
fin.

#### `DELETE /datasets/{id}`

- Stoppe l'instance ORS si elle tourne, supprime le dataset
- Réponse : `204` ou `404`
- **Précaution** : supprime graphes et caches d'élévation
- Si le dataset supprimé est le dataset sélectionné, la sélection est conservée
  par id pour permettre le flux update `DELETE` puis `POST`.

### 1.3 Régions (M1, lecture seule)

#### `GET /regions`

Liste les régions indexées dans `regions.json`, avec leur contour
GeoJSON (T48).

- Réponse : `200` + tableau JSON

Exemple :
```json
[
  {
    "id": "provence-alpes-cote-dazur",
    "bbox": [4.144746, 42.31539, 7.748528, 45.12887],
    "has_poly": true,
    "geojson": {
      "type": "Polygon",
      "coordinates": [[[7.729716, 43.48231], [7.555371, 43.48099], ...]]
    },
    "geofabrik_id": "europe/france/provence-alpes-cote-d-azur",
    "updated_at": "2026-11-06T21:04:41Z"
  },
  {
    "id": "monaco",
    "bbox": [7.408583, 43.48382, 7.595671, 43.75293],
    "has_poly": true,
    "geojson": {...},
    "geofabrik_id": "europe/monaco",
    "updated_at": "2026-11-07T10:30:33Z"
  }
]
```

- `has_poly` : `true` si un fichier `.poly` a été trouvé et converti,
  `false` si fallback sur `bbox` (rectangle)
- `geojson` : objet `Polygon` standard (lon/lat WGS84)
- `geofabrik_id` : **ajout v1.8** (E13), identifiant Geofabrik complet
  (`null` si migration non faite)
- `updated_at` : **ajout v1.8** (E13), timestamp ISO 8601 UTC de la
  dernière mise à jour locale (POST/PUT `/regions`). `null` si inconnu.
- Utilisation M1 : affichage des régions sur la carte (peut ignorer
  `geofabrik_id` et `updated_at`, non-breaking)

### 1.4 Polygones custom (CRUD, T54)

Les polygones custom sont stockés côté serveur dans
`/srv/hermes/polygons/` (T51). Ils servent au mode `zones` de
`POST /datasets`.

#### `GET /polygons`

- Réponse : `200` + tableau JSON de polygones complets
- Chaque polygone contient tous ses champs (pas de version allégée) :
  `id`, `name`, `description`, `geometry`, `bbox`,
  `created_at`, `updated_at`, `revision`

Exemple :
```json
[
  {
    "id": "ma-zone-nice",
    "name": "Zone Nice - Col de Turini",
    "description": "Zone tampon autour de la trace",
    "geometry": {"type": "Polygon", "coordinates": [[...]]},
    "bbox": [7.20, 43.65, 7.40, 43.80],
    "created_at": "2026-11-06T10:30:00Z",
    "updated_at": "2026-11-06T10:30:00Z",
    "revision": 1
  }
]
```

- Pas de filtre (T54) : M1 filtre côté client
- Pas de pagination (T54)

#### `GET /polygons/{id}`

- Réponse : `200` + objet complet, ou `404`
- `400` si id invalide (regex)
- `403` si permissions insuffisantes (cas rare)

#### `POST /polygons`

Crée un polygone. L'`id` est fourni par le client (T54).

**Payload :**
```json
{
  "id": "ma-zone-nice",
  "name": "Zone Nice",
  "description": "optionnel",
  "geometry": {"type": "Polygon", "coordinates": [[[7.26,43.71],[7.28,43.71],[7.28,43.73],[7.26,43.73],[7.26,43.71]]]}
}
```

**Validation** (voir §8) :
- `id` : regex `^[a-zA-Z0-9_-]+$`, 1-64 chars, `index` réservé
- `geometry` : `Polygon` uniquement, 1000 sommets max, auto-fermeture,
  arrondi serveur à 5 décimales

**Réponses :**

| HTTP | Cas | Corps |
|---|---|---|
| `201` | Créé | objet complet |
| `400` | `id` invalide, `geometry` invalide | `{"detail": "..."}` |
| `409` | `id` déjà utilisé | `{"detail": "polygone existe déjà: <id>"}` |

#### `PUT /polygons/{id}`

Remplacement complet (pas de `PATCH`, T54). Le client envoie
`name`, `description?`, `geometry`. Le serveur recalcule
`bbox`, incrémente `revision`, met à jour `updated_at`.

**Payload :**
```json
{
  "name": "Zone Nice v2",
  "geometry": {"type": "Polygon", "coordinates": [[...]]}
}
```

- Champs modifiables : `name`, `description`, `geometry`
- Champs ignorés : `id`, `created_at`, `bbox`, `revision` (calculés
  serveur). Un `id` fourni dans le corps est **ignoré silencieusement**
  (Q4.2 v1.7, confirmé D.2).
- Remplacement complet : `description` absent → remis à
  `null` (Q8 du contrat v1.7).

**Réponses :**

| HTTP | Cas | Corps |
|---|---|---|
| `200` | Modifié | objet complet mis à jour |
| `400` | Payload invalide | `{"detail": "..."}` |
| `404` | Polygone introuvable | `{"detail": "polygone introuvable"}` |

#### `DELETE /polygons/{id}`

- Réponse : `204` ou `404`
- `400` si id invalide
- **Précaution** : si un dataset a été créé avec ce polygone, il
  reste fonctionnel (son `merged.osm.pbf` est déjà clippé). Seul le
  polygone lui-même est supprimé.

### 1.5 Instances ORS

#### `GET /instances`

- Réponse : `200` + tableau d'instances actives

```json
[{"id": "provence-alpes-cote-dazur", "status": "active"}]
```

#### `POST /instances/{id}/start`

- Démarre ORS pour le dataset spécifié
- Réponse : `200` + `{"id": "...", "status": "starting"}`, `404` si
  dataset introuvable, `409` si `ors-config.yml` manquant
- `status: "starting"` ≠ `ready`. Polling `GET /ors/v2/health` requis.
- Délai : ~15 s (1 profil) à ~70 s (2 profils, T46)

#### `POST /instances/{id}/stop`

- Arrête ORS
- Réponse : `200` + `{"id": "...", "status": "stopped"}`, ou `404`
- Après stop, nouveau `start` recharge le graphe (15–70 s)

### 1.6 Dataset servi par Hermes

La sélection est indépendante du dataset en cours de construction. Elle est
persistée dans `/srv/hermes/serving.json`.

États : `none`, `waiting`, `starting`, `active`, `stopped`, `failed`.

#### `GET /serving`

```json
{
  "datasetId": "Monaco",
  "state": "waiting",
  "stateLabel": "En attente",
  "datasetState": "building",
  "since": "2026-11-23T12:00:00Z",
  "reason": "construction en cours"
}
```

#### `PUT /serving/{dataset_id}`

- persiste le dataset sélectionné ;
- démarre automatiquement ORS lorsqu’il est exploitable ;
- ne remplace pas automatiquement la sélection lors d’un build d’un autre
  dataset.

#### `DELETE /serving`

- arrête ORS ;
- efface la sélection.

### 1.7 Artefacts auxiliaires

Les trois artefacts sont `communes.geojson`, `tunnels-logiques.json` et
`mm/`. Chaque artefact expose indépendamment :

```text
pending | building | ready | failed
```

Un artefact `ready` doit posséder son marqueur de terminaison sous
`/srv/hermes/datasets/<id>/.state/`. En particulier, un dossier `mm/`
non vide n’est pas `ready` tant que `mm.json` ne l’indique pas.

#### `GET /datasets/{id}/aux`

Réponse `200` :

```json
{
  "datasetId": "EB-Monaco",
  "communes": {
    "name": "communes.geojson",
    "state": "ready",
    "exists": true,
    "sizeBytes": 123456,
    "completedAt": "2026-11-23T20:00:00Z"
  },
  "tunnels": {
    "name": "tunnels-logiques.json",
    "state": "ready",
    "exists": true
  },
  "mm": {
    "name": "mm/",
    "state": "building",
    "exists": true
  },
  "preparing": true,
  "startedAt": "2026-11-23T20:01:00Z",
  "lastError": null,
  "loadedInMemory": false,
  "activeDataset": null,
  "runId": "f7..."
}
```

- `404` si le dataset n’existe pas.
- `preparing=true` uniquement pour le dataset effectivement en préparation.
- `lastError` est propre au dernier run du dataset.

#### `POST /datasets/{id}/aux/prepare`

Payload strict :

```json
{"force": false}
```

- `force=true` régénère les trois artefacts même s’ils existent.
- `skipMm` n’existe plus. Pydantic refuse les champs inconnus : envoyer
  `skipMm` produit HTTP **422**.
- Réponse nominale : HTTP **200**, pas 202.

```json
{
  "datasetId": "EB-Monaco",
  "status": "preparing",
  "startedAt": "2026-11-23T20:01:00Z",
  "runId": "f7...",
  "message": "Préparation en tâche de fond. Consulter GET /datasets/{id}/aux"
}
```

- `404` : dataset inexistant.
- `409` : build ORS en cours, aux déjà en cours, ou PBF absent/non validé.
- La route n’exige pas `X-Admin-Token` dans l’implémentation actuelle ; son
  accès est protégé par le réseau privé.

Condition complète de succès :

```text
preparing == false
lastError == null
communes.state == ready
tunnels.state == ready
mm.state == ready
```

### 1.8 Admin — Régions (T55)

**Tous les endpoints de cette section nécessitent le header
`X-Admin-Token`** (voir §9).

#### `POST /regions`

Ajoute une région depuis le catalogue Geofabrik. Télécharge le PBF +
le `.poly`, valide, place dans `/srv/hermes/pbf/`, met à jour
`regions.json`.

**Payload :**
```json
{"id": "monaco", "geofabrik_id": "europe/monaco"}
```

- `id` : validation `^[a-zA-Z0-9_-]+$`, 1-64 chars, `regions` et `index`
  réservés, **normalisation lowercase automatique** (E9, C2) : `"Monaco"`
  → `"monaco"`
- `geofabrik_id` : identifiant Geofabrik complet (path), ex.
  `europe/france/provence-alpes-cote-d-azur`

**Pipeline serveur (T58 — tâche de fond) :**
1. Valide `id` (lowercase)
2. Vérifie que `id` n'existe pas déjà (`409`)
3. Vérifie qu'aucun autre téléchargement n'est en cours (`409` + `job_id`)
4. Cherche `geofabrik_id` dans le catalogue Geofabrik (404 interne si absent → `400`)
5. Répond immédiatement `202` avec un `job_id`, puis en arrière-plan :
   télécharge le PBF (streaming reprise `Range`, whitelist
   `GEOFABRIK_BASE_URL`, taille max 5 Go, blocage détecté après 60 s sans
   octet, vérification taille + MD5), valide `osmium fileinfo`, télécharge
   le `.poly` (best-effort), écriture atomique puis `regions.json`
6. **Un seul job à la fois** (C3) : un second POST/PUT renvoie `409`

**Réponses :**

| HTTP | Cas | Corps |
|---|---|---|
| `202` | Job accepté | `{"job_id": "...", "id": "...", "status": "queued"}` |
| `400` | Payload invalide, URL non whitelistée, id réservé, région Geofabrik inconnue | `{"detail": "..."}` |
| `401` | Token absent ou invalide | `{"detail": "X-Admin-Token absent ou invalide"}` |
| `409` | Région déjà existante, ou téléchargement déjà en cours | `{"detail": "..."}` ou `{"detail": {"message": "...", "job_id": "..."}}` |
| `500` | `ADMIN_TOKEN` non configuré côté serveur | `{"detail": "ADMIN_TOKEN non configuré côté serveur"}` |
| `503` | Catalogue Geofabrik injoignable | `{"detail": "..."}` |
| `507` | Disque plein | `{"detail": "..."}` |

- Whitelist : URLs commençant par `GEOFABRIK_BASE_URL` (T56)
- Taille max : 5 Go
- Suivi : `GET /regions/downloads/{job_id}` (voir §1.6.1)

**Path traversal** (E14) : un id contenant `/` est bloqué **au niveau du
router HTTP** (404, résolution URL) **et** par `validate_region_id` (400).
Défense en profondeur.

#### `PUT /regions/{id}`

Met à jour une région existante (re-téléchargement du PBF + poly).

- Réponse : `202` + `{"job_id": "...", "id": "...", "status": "queued"}`
  (téléchargement en tâche de fond, T58)
- Le résultat final (`affected_datasets`) est disponible dans le job :
  `GET /regions/downloads/{job_id}` → champ `result` :
  ```json
  {
    "id": "provence-alpes-cote-dazur",
    "updated_at": "...",
    "affected_datasets": [
      {"id": "provence-alpes-cote-dazur", "status": "ready", "stale": true}
    ]
  }
  ```
  Permet à l'admin de signaler aux utilisateurs que leur dataset est
  `stale`.
- **Ne reconstruit PAS les datasets existants** (responsabilité
  utilisateur)
- `404` si région introuvable, `400` si id invalide ou `geofabrik_id`
  manquant, `409` si build/téléchargement en cours
- **Un seul job à la fois** (C3) : sérialise avec `POST /regions`
- `bbox` recalculée depuis la `geometry` Geofabrik (E16)

#### `DELETE /regions/{id}`

- Supprime les fichiers `.pbf` et `.poly` et l'entrée dans `regions.json`
- Réponse : `204` ou `404`
- `400` si id invalide
- `409` si un téléchargement est en cours sur cette région (`job_id` fourni)
- **N'échoue pas** si des datasets existants utilisent encore cette
  région (leur `merged.osm.pbf` reste intact). Les datasets
  deviennent `stale: true` (région absente de `regions.json`) mais
  restent fonctionnels.

#### Jobs de téléchargement (T58)

**Tous les endpoints de cette sous-section nécessitent `X-Admin-Token`.**

| Méthode | Route | Description |
|---|---|---|
| `GET` | `/regions/downloads?active=true` | Liste des jobs (50 derniers, ou actifs seulement) |
| `GET` | `/regions/downloads/{job_id}` | État + progression d'un job |
| `POST` | `/regions/downloads/{job_id}/cancel` | Annulation (le partiel est conservé) |
| `POST` | `/regions/downloads/{job_id}/resume` | Reprise depuis le fichier partiel |

Un job expose notamment :

```json
{
  "job_id": "dl-20261118T150643-092549",
  "action": "add",
  "region_id": "monaco",
  "status": "downloading_pbf",
  "bytes_downloaded": 312000000,
  "total_bytes": 708123000,
  "percent": 44.1,
  "speed_bps": 3400000,
  "eta_s": 116,
  "attempt": 2,
  "max_attempts": 6,
  "retry_reason": "aucune donnée reçue pendant 60 s (connexion bloquée)",
  "stalled": false,
  "seconds_since_progress": 3,
  "error": null,
  "result": null
}
```

**Statuts :** `queued`, `downloading_pbf`, `verifying_pbf`,
`downloading_poly`, `committing`, `done`, `failed`, `cancelled`,
`interrupted`.

- `interrupted` : le service a redémarré pendant le téléchargement. Le
  fichier partiel (`/srv/hermes/pbf/.tmp-<id>.osm.pbf`) et ses
  métadonnées sont conservés ; `POST .../resume` reprend à l'offset exact.
- `stalled: true` : aucun octet reçu depuis `GEOFABRIK_READ_TIMEOUT_S`
  (60 s par défaut) pendant une phase de téléchargement. La reprise est
  automatique (backoff, jusqu'à `GEOFABRIK_MAX_ATTEMPTS`).
- `error` / `retry_reason` sont des messages lisibles destinés à l'IHM.

### 1.9 Admin — Catalogue Geofabrik (T55, T56)

**Tous les endpoints de cette section nécessitent `X-Admin-Token`.**

#### `GET /geofabrik/regions`

Renvoie le catalogue Geofabrik enrichi. Cache local 7 jours.

**Paramètres :**
- `refresh` (query, bool, défaut `false`) : force le re-téléchargement
  du catalogue (E5)

**Réponse : enveloppe** (E6, **rupture vs v1.7 qui retournait un tableau**) :

```json
{
  "fetched_at": "2026-11-06T20:16:08Z",
  "regions": [
    {
      "id": "europe/monaco",
      "name": "Monaco",
      "parent": "europe",
      "path": ["europe", "monaco"],
      "level": 2,
      "size": 707098,
      "last_update": "2026-11-05T16:43:47Z",
      "already_added": true,
      "state": "stale",
      "age_days": 1,
      "geometry": {"type": "MultiPolygon", "coordinates": [[...]]}
    },
    {
      "id": "europe/france/provence-alpes-cote-d-azur",
      "name": "Provence Alpes-Cote-d'Azur",
      "parent": "europe/france",
      "path": ["europe", "france", "provence-alpes-cote-d-azur"],
      "level": 3,
      "size": 391496840,
      "last_update": "2026-11-05T18:08:41Z",
      "already_added": true,
      "state": "stale",
      "age_days": 2,
      "geometry": {"type": "MultiPolygon", "coordinates": [[...]]}
    },
    {
      "id": "europe/italy/nord-ovest",
      "name": "Nord-Ovest",
      "parent": "europe/italy",
      "path": ["europe", "italy", "nord-ovest"],
      "level": 3,
      "size": 594760218,
      "last_update": "2026-11-05T20:11:36Z",
      "already_added": true,
      "state": "stale",
      "age_days": 1,
      "geometry": {...}
    },
    {
      "id": "europe/andorra",
      "name": "Andorra",
      "parent": "europe",
      "path": ["europe", "andorra"],
      "level": 2,
      "size": 3508150,
      "last_update": "2026-11-05T15:30:12Z",
      "already_added": false,
      "state": "absent",
      "age_days": null,
      "geometry": {...}
    }
  ]
}
```

**Champs par région (11, E1–E4) :**

| Champ | Type | Sens |
|---|---|---|
| `id` | str | path complet (`europe/france/provence-...`), reconstruit via parent |
| `name` | str | nom lisible |
| `parent` | str | path complet du parent (`europe/france`), `null` si racine |
| `path` | list[str] | segments (`["europe","france","provence-..."]`), pour construire un arbre UI |
| `level` | int | `1`=continent, `2`=pays, `3`=région (`len(path)`) |
| `size` | int \| null | taille PBF en octets (HEAD lazy, `null` si échec) |
| `last_update` | str \| null | ISO 8601 UTC de dernière MAJ Geofabrik (HEAD lazy, `null` si échec) |
| `already_added` | bool | `true` si `id` présent dans `regions.json` (via `geofabrik_id`) |
| `state` | str | `absent` / `current` / `stale` / `old` (voir table ci-dessous) |
| `age_days` | int \| null | jours depuis `updated_at` local (`null` si `absent`) |
| `geometry` | object | GeoJSON `MultiPolygon` (catalogue source, pour carte) |

**Logique `state` (E3, priorité `stale` > `old` > `current`) :**

| `state` | Condition | Couleur UI |
|---|---|---|
| `absent` | pas dans `regions.json` | rouge |
| `stale` | présent + Geofabrik a publié une version plus récente (`last_update > updated_at`), **indépendant de l'âge** | vert-jaune |
| `old` | présent + pas de MAJ Geofabrik + `age_days >= 60` (2 mois) | orange |
| `current` | présent + pas de MAJ Geofabrik + `age_days < 60` | vert |

**`size` et `last_update` (E7)** : le catalogue Geofabrik `index-v1.json`
ne contient **pas** ces champs. Le serveur les obtient par **HEAD** sur
l'URL PBF (ou GET `Range: bytes=0-0` en fallback), **uniquement pour
les régions déjà ajoutées** (`already_added=true`) — typiquement 2 à 5
requêtes, coût négligeable. En cas d'échec réseau, `size` et
`last_update` restent `null` (état limité à `current` ou `old`).

**Cache** :
- Fichier : `/srv/hermes/pbf/geofabrik_catalog.json`
- TTL : **7 jours** (`GEOFABRIK_CATALOG_URL` refetch automatique)
- `?refresh=true` force le re-fetch
- Si Geofabrik inaccessible + cache existant (même périmé) : cache
  servi + warning log
- Si Geofabrik inaccessible + pas de cache : `503`
- `fetched_at` : timestamp du dernier fetch réussi

**Filtrage** : uniquement les entrées avec `urls.pbf` **et** `geometry`
présentes (exclut les continents sans PBF téléchargeable).

**Usage UI admin** : arbre dépliable par `path` (continent → pays →
région), survol/clic affiche `geometry` sur carte Leaflet, couleurs
selon `state`, boutons « Ajouter » (state=absent) ou « Mettre à jour »
(state=current/stale/old).

---

## 2. API ORS (port 8002)

- URL de base (LAN ou ZeroTier) : `http://10.15.58.131:8002/ors/v2`
- Format : JSON
- Profils actifs : **`driving-car`**, **`cycling-road`** (T36)

### 2.1 Health check

**`GET /ors/v2/health`**

- Réponse : `200` + `{"status": "ready"}`, ou `503` + `{"status": "not ready"}`
- À utiliser après chaque `POST /instances/{id}/start` ou après un
  redémarrage serveur

### 2.2 Calcul d'itinéraire

**`POST /ors/v2/directions/{profile}/geojson`**

- Profils : `driving-car`, `cycling-road`
- **Limite** : distance à vol d'oiseau entre les waypoints **≤ 100 km**
  (code 2004 au-delà). Voir T47 ci-dessous.

**Payload :**

```json
{
  "coordinates": [[lon1, lat1], [lon2, lat2]],
  "elevation": true,
  "instructions": true,
  "geometry_simplify": false
}
```

- `coordinates` : 2 à 50 waypoints `[lon, lat]` (WGS84)
- `elevation` : `true` → coordonnées 3D
- `instructions` : `true` → segments + steps
- `geometry_simplify` : `false` → tous les points

**Réponse :**

- `200` + GeoJSON `FeatureCollection`
- `400` + `{"error": {"code": 2004, ...}}` si route > 100 km
- `404` + `{"error": {"code": 2010, ...}}` si waypoint hors du réseau routable (rayon 400 m)
- `503` si ORS pas `ready`

**Champs exploités par M1 :**

| Champ | Signification |
|---|---|
| `features[0].geometry.coordinates` | `[[lon, lat, ele], ...]` |
| `features[0].properties.ascent` | Dénivelé positif (m) |
| `features[0].properties.descent` | Dénivelé négatif (m) |
| `features[0].properties.summary.distance` | Distance (m) |
| `features[0].properties.summary.duration` | Durée (s) |
| `features[0].properties.segments[].steps[]` | Guidage (optionnel) |

### 2.3 Découpage des itinéraires (T47)

**L'app M1 découpe systématiquement tout itinéraire en segments
de ≤ 50 km entre waypoints consécutifs.**

Règles :
- Distance mesurée à vol d'oiseau (Haversine)
- Si > 50 km : insérer un waypoint intermédiaire (interpolation
  géodésique) jusqu'à ce que tous les segments soient ≤ 50 km
- Les waypoints intermédiaires ne sont pas affichés dans l'UI
- Concaténation finale : distance, durée, ascent/descent, coordonnées
  (supprimer le doublon aux jonctions)

**Garde-fou ORS** (`maximum_distance = 100 km`, inchangé) : un
segment > 100 km (bug de découpage) renvoie `code 2004`.

---

## 3. Réveil à distance (Wake-on-LAN)

Le serveur se met en veille après **15 minutes** d'inactivité.

- **Mécanisme** : paquet UDP (port 9) vers l'IP publique de la box FAI
- **IP cible** : `87.91.31.205` (Box FAI Bouygues)
- **MAC cible** : `68:5b:35:a8:5c:65` (interface `enp1s0f0`)
- **Format** : 6 octets `0xFF` + 16 répétitions de la MAC

**Exemple Python :**

```python
import socket
mac = bytes.fromhex("685b35a85c65")
pkt = b"\xff" * 6 + mac * 16
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
for _ in range(3):
    s.sendto(pkt, ("87.91.31.205", 9))
s.close()
```

**Recommandation** : 3 trames (espacées de 1 s). Attendre ~50 s
(phase 4) à ~2 min (phase 5, mesuré 1min52s). Le client doit
réessayer `GET /ping` en boucle (timeout global 3 min).

---

## 4. Cycle de vie complet type

```
┌─ App M1 ouverte ──────────────────────────────────────────────┐
│                                                                │
│ 1. GET /api/v1/ping (toutes les 30 s)                         │
│                                                                │
│ 2. [Affichage carte, optionnel]                                │
│    GET /api/v1/regions                                        │
│    GET /api/v1/polygons                                       │
│                                                                │
│ 3. [Création zone custom, optionnel]                          │
│    POST /api/v1/polygons {id, name, geometry}                 │
│    → 201 objet complet                                        │
│                                                                │
│ 4. POST /api/v1/datasets {name, regions} OU {name, zones}    │
│    → 202 {"id":"...","status":"preparing"}                    │
│                                                                │
│ 5. GET  /api/v1/datasets/{id} (polling 30 s)                 │
│    → preparing → ready (ou failed + last_log_tail)           │
│                                                                │
│ 6. POST /api/v1/instances/{id}/start                          │
│    → 200 starting                                             │
│                                                                │
│ 7. GET  /ors/v2/health (polling 3 s)                         │
│    → 503 → 200 ready (~15-70 s)                               │
│                                                                │
│ 8. [Pour chaque segment ≤ 50 km, T47]                         │
│    POST /ors/v2/directions/{profile}/geojson                  │
│                                                                │
│ 9. [Concaténation côté M1]                                     │
│                                                                │
│ 10. POST /api/v1/instances/{id}/stop                          │
│     → 200 stopped                                              │
│                                                                │
└────────────────────────────────────────────────────────────────┘

┌─ App admin ───────────────────────────────────────────────────┐
│                                                                │
│ 1. [Ouvrir UI] http://10.15.58.131:9000/admin/               │
│    - Saisir token si localStorage vide                        │
│    - Vue unifiée 3 colonnes (Catalogue | Carte | Régions)    │
│                                                                │
│ 2. [Catalogue] colonne 1, arbre + carte au survol             │
│    → couleurs state, boutons Ajouter/MAJ                     │
│                                                                │
│ 3. [Ajouter une région]                                       │
│    POST /api/v1/regions {id, geofabrik_id}                    │
│    → 201, téléchargement + ajout automatique                  │
│                                                                │
│ 4. [Mettre à jour]                                            │
│    PUT /api/v1/regions/{id}                                   │
│    → 200, affected_datasets retournés                         │
│                                                                │
│ 5. [Supprimer]                                                │
│    DELETE /api/v1/regions/{id}                                │
│    → 204, datasets stale=true                                 │
│                                                                │
└────────────────────────────────────────────────────────────────┘

┌─ Si l'utilisateur ferme l'app M1 ──────────────────────────────┐
│ Après 15 min sans ping → veille auto (S3)                     │
│ Réveil : WoL UDP/9, puis GET /ping en boucle                  │
│ Si ORS était stoppé avant veille → reste stoppé (refaire      │
│ POST /instances/{id}/start)                                    │
└────────────────────────────────────────────────────────────────┘
```

**Notes importantes :**

- **ORS non redémarré après veille** : S3 préserve l'état. Si instance
  `active` avant veille → toujours `active` (health ready en 10-30 s
  après réveil). Si `stopped` → reste `stopped`.
- **API toujours active après veille** : systemd relance, ~5-10 s.
- **Redémarrage ORS** : 15-70 s selon dataset. Utiliser
  `GET /ors/v2/health`, pas un timer fixe.
- **Dataset `stale`** : si `stale: true` dans `GET /datasets`, une
  région a été mise à jour ou supprimée après la création. Reconstruire
  avec `POST /datasets` si on veut bénéficier des nouvelles données.
- **Dataset `zones` et stale** (v1.8) : `stale=false` toujours pour les
  datasets mode `zones`, car le manifest ne stocke pas les régions
  sous-jacentes utilisées pour le clipping. Le client peut reconstruire
  à la demande (build rapide, ~1-3 min).

---

## 5. États de dataset (référence)

| `state` | Signification | Action client |
|---|---|---|
| `initializing` | PBF en construction | Poller `GET /datasets/{id}` |
| `building` | PBF validé ; ORS ou auxiliaires en cours | Attendre ; déclencher les aux si nécessaire |
| `ready` | PBF, ORS, Communes, Tunnels et MM prêts | Dataset complet |
| `unavailable` | PBF ou graphe ORS inutilisable | Lire `last_log_tail`, corriger/reconstruire |
| `degradedCommunes` | Communes en échec uniquement | Le routage ORS reste possible |
| `degradedTunnels` | Tunnels et/ou MM en échec | Le routage reste possible ; Tunnels indisponibles |
| `degradedAux` | Communes et Tunnels/MM en échec | Dataset partiellement utilisable |

Mapping de compatibilité :

| `state` | `status` historique |
|---|---|
| `initializing`, `building` | `preparing` |
| `ready`, `degraded*` | `ready` |
| `unavailable` | `failed` |

Conséquences pour M1 :

- ne pas déclencher les aux sur `status === "ready"` ;
- attendre `artifacts.ors.state === "ready"` pour activer le bouton aux ;
- utiliser `state` pour déterminer si le dataset est terminal et/ou dégradé ;
- un dataset peut avoir `status=preparing` pendant toute la phase aux.

**Champ `stale`** (indépendant du statut) : `true` si une région a
été mise à jour ou supprimée depuis la création du dataset.
Reconstruire si on veut bénéficier des nouvelles données.

- **Mode `regions`** : `stale` calculé selon `regions.json`
- **Mode `zones`** (v1.8) : `stale=false` par design

---

## 6. Codes d'erreur HTTP et ORS

### Codes HTTP (API contrôleur)

| Code | Signification | Causes fréquentes |
|---|---|---|
| `200` | OK | — |
| `201` | Créé | POST /polygons, POST /regions |
| `202` | Accepté | POST /datasets (build lancé) |
| `204` | Supprimé | DELETE /datasets, /polygons, /regions |
| `400` | Requête invalide | Payload mal formé, XOR violé, géométrie invalide, limite dépassée, id invalide, PBF invalide |
| `401` | Non autorisé | X-Admin-Token absent ou invalide (endpoints admin) |
| `404` | Introuvable | Dataset, polygone, région inexistants |
| `409` | Conflit | Build/aux en cours, PBF non prêt, id déjà utilisé, `ors-config.yml` manquant |
| `422` | Validation | Payload invalide, notamment `skipMm` dans `/aux/prepare` |
| `500` | Erreur serveur | Exception interne, configuration admin invalide |
| `503` | Service indisponible | Catalogue Geofabrik inaccessible + cache absent |
| `507` | Stockage insuffisant | Disque plein (POST/PUT /regions) |

### Codes d'erreur ORS

| Code | Message | Cause | Action |
|---|---|---|---|
| `2004` | `Request parameters exceed the server configuration limits` | Route > 100 km à vol d'oiseau | Découper en segments ≤ 50 km (T47) |
| `2010` | `Could not find routable point within a radius of 400m` | Waypoint hors réseau routable | Choisir un point plus proche d'une route ; vérifier que le point est dans le dataset chargé |
| `2099` | `Unable to compute route` | Pas de connexion | Vérifier les coordonnées |

### Messages d'erreur admin (v1.8)

- `401` : `{"detail":"X-Admin-Token absent ou invalide"}` (fixe)
- `500` : `{"detail":"ADMIN_TOKEN non configuré côté serveur"}` (fixe)
- `409` : `{"detail":"build déjà en cours: <id>"}` ou `{"detail":"région existe déjà: <id>"}`

---

## 7. Contraintes de robustesse côté client

- **Toujours** `GET /ping` toutes les 30 s tant que l'app est ouverte
- **Toujours** poller `GET /datasets/{id}` sur `state`, et non uniquement
  sur le champ historique `status`
- **Toujours** attendre `artifacts.ors.state == "ready"` avant de lancer
  `POST /datasets/{id}/aux/prepare`
- **Toujours** considérer les aux terminées uniquement avec
  `communes.state == "ready"`, `tunnels.state == "ready"` et
  `mm.state == "ready"`, plus `preparing == false` et `lastError == null`
- **Toujours** poller `GET /ors/v2/health` après un `start`
- **Toujours** vérifier `features[0].geometry.coordinates` avant
  affichage (parfois vide si route invalide)
- **Toujours** découper les itinéraires en segments ≤ 50 km (T47)
- **Toujours** gérer les codes d'erreur ORS explicitement (2004, 2010, 2099)
- **Toujours** gérer `stale: true` dans `GET /datasets` (avertir
  l'utilisateur)
- **Toujours** vérifier `already_added` avant `POST /regions` (app admin)
- **UI admin** (v1.9) : le token est dans `localStorage`, ne doit
  jamais être loggé côté client

---

## 8. Validation géométrique des polygones

Règles appliquées par `hermes-api` à `POST /polygons` et
`PUT /polygons/{id}` (T54) :

| Règle | Valeur |
|---|---|
| Type | `Polygon` uniquement (pas de `MultiPolygon`, pas de trous) |
| Fermeture | Auto-fermeture silencieuse si 1er ≠ dernier point |
| Sens | Horaire ou anti-horaire (libre) |
| Sommets | Max 1000 (après auto-fermeture) |
| Précision | Arrondi serveur à 5 décimales (~1 m) |
| Coordonnées | WGS84 (lon/lat), ordre lon-lat, bornes ±180 / ±90 |
| `id` | Regex `^[a-zA-Z0-9_-]+$`, 1-64 chars, `index` réservé |
| `name` | Non vide après strip, ≤ 200 caractères |
| `description` | ≤ 2000 caractères (optionnel) |

**Aire du polygone** (v1.8) : `polygons.polygon_area_km2(geometry)`
calcule une aire approximative par Shoelace + correction `cos(lat)`
moyenne. Précision < 0.1 % pour zones < 100 km de côté. Utilisée pour
valider `MAX_DATASET_AREA_KM2` (T53). **Pas exposée dans l'API** —
c'est un garde-fou interne.

**Raison du choix `Polygon` uniquement** : cohérent avec la
décision T52 (pas de mixte). Pour plusieurs zones disjointes,
créer plusieurs polygones et les référencer dans `zones: [...]`.

---

## 9. Authentification admin

Les endpoints admin (`/regions` POST/PUT/DELETE, `/geofabrik/regions`)
exigent un header `X-Admin-Token` (T55).

- **Token stocké** : `/etc/hermes/hermes.conf` (`ADMIN_TOKEN=...`)
- **Permissions** (T57-A, v1.8) : fichier `root:hermes-api 640`
  - root (owner) et hermes-api (group) peuvent lire
  - ors ne peut pas lire directement (systemd lit le fichier en root
    via `EnvironmentFile=`)
- **Génération** : `openssl rand -hex 32` à l'installation
- **Transmission** : header HTTP `X-Admin-Token: <valeur>`
- **Erreur si absent ou invalide** : `401 Unauthorized`
  `{"detail":"X-Admin-Token absent ou invalide"}`
- **Comparaison** : `hmac.compare_digest` (temps constant)

**Note** : le token n'est PAS nécessaire pour les endpoints M1
(`/ping`, `/datasets`, `/regions` GET, `/polygons`, `/instances`).

**UI admin — refonte v1.9 (T57-I)** : l'interface web utilise le même
mécanisme `X-Admin-Token` mais avec une **ergonomie différente** :

**Flux utilisateur** :
1. **Premier chargement sans token** : overlay `Token requis` avec fond
   flouté. Toolbar affiche `○ Token absent` (rouge).
2. **Cliquer « Configurer le token »** (overlay) ou **« Changer le token »**
   (toolbar) → ouvre un `<dialog>` HTML natif (pas de modal custom JS).
3. **Champ `type=password`** + boutons `Enregistrer` / `Annuler`.
   `Échap` ferme. `Enregistrer` écrit dans localStorage et recharge la page.
4. **Après saisie** : toolbar affiche `● Token configuré` (vert), overlay
   disparaît, contenu de la vue unifiée visible.
5. **Cliquer « Déconnexion »** : `clearToken()` + reload → retour overlay.

**Clés de stockage côté client (v1.9)** :

| Clé | Type | Usage |
|---|---|---|
| `hermes_admin_token` | string | Token admin (X-Admin-Token) |
| `hermes_admin_theme` | `auto` \| `light` \| `dark` | Thème UI |
| `hermes_admin_countries` | string (CSV) | Filtre pays. Défaut 1er lancement : `France, Italie, Suisse, Espagne` |
| `hermes_admin_max_download_mb` | int | Limite téléchargement. Défaut : 500 |
| IndexedDB `hermes_admin` / store `catalog` | object | Cache du catalogue Geofabrik (TTL 24 h) |

Ces clés ne sont **jamais** transmises au serveur. Seul `hermes_admin_token`
sort du navigateur, dans le header `X-Admin-Token`.

**Recommandation** : le token ne doit jamais être loggé côté client.
Utiliser HTTPS si possible (le réseau ZeroTier est privé, mais un
attaquant avec accès au réseau pourrait intercepter le token).

---

## 10. Limites de construction (T53)

Configurables dans `/etc/hermes/hermes.conf` :

| Variable | Valeur par défaut | Effet |
|---|---|---|
| `MAX_DATASET_REGIONS` | 4 | Nombre max de régions (PBF) concernées par un dataset |
| `MAX_DATASET_AREA_KM2` | 50000 | Surface totale max des zones (clipping) |
| `MAX_DATASET_ZONES` | 100 | Nombre max de zones dans un dataset |

**Dépassement → `400`** avec message clair :
- `"trop de régions: N (max M)"`
- `"surface totale N km² (max M)"`
- `"trop de zones: N (max M)"`

**Ordre de vérification** (v1.8) :
1. `MAX_DATASET_ZONES` avant `resolve_zones` (fail fast)
2. `MAX_DATASET_REGIONS` après résolution
3. `MAX_DATASET_AREA_KM2` après calcul de surface (mode zones)

**Modification** : éditer `hermes.conf` puis
`sudo systemctl restart hermes-api.service`.

**Raison** : éviter les abus et garder `hermes` prévisible. Valeurs
configurables pour suivre l'évolution matérielle (passage à plus de
RAM, CPU plus rapide).

---

## 11. UI admin (T55, refonte T57-I) — v1.9

Interface web d'administration, servie par `hermes-api` sur `/admin/*`.
Refondue en v1.9 (T57-I) : vue unifiée, thème, Pico.css, cache client,
traduction FR, garde-fou téléchargement.

### 11.1 Pages et navigation

| URL | Contenu |
|---|---|
| `/admin/` | Vue unifiée 3 colonnes (Catalogue \| Carte \| Régions) |
| `/admin/regions` | Stub → redirection JS vers `/admin/?focus=regions` |
| `/admin/geofabrik` | Stub → redirection JS vers `/admin/?focus=geofabrik` |
| `/admin/static/*` | Assets statiques |

**Deep-links** : `?focus=regions` ou `?focus=geofabrik` positionne le
scroll sur la colonne concernée avec un outline bleu qui s'efface
après ~2,5 s. Permet de conserver les anciens bookmarks
`/admin/regions` et `/admin/geofabrik`.

**Layout 3 colonnes** (gauche → droite) :

1. **Catalogue Geofabrik** : arbre dépliable + filtre pays + carte
   au survol (hover).
2. **Carte Leaflet** : affiche les régions configurées en permanence
   (colorées par `state`), et la région survolée (catalogue ou liste).
3. **Régions configurées** : liste verticale avec drapeaux, badges,
   boutons MAJ/Suppr.

**Toolbar** (header permanent, sticky) :
- Brand `hermes admin` cliquable → `/admin/`
- Indicateur d'état token (`● Token configuré` / `○ Token absent`)
- Bouton `Changer le token` (ouvre modal)
- Bouton `Déconnexion` (vide localStorage + retour overlay)
- Bouton thème : cycle `auto → light → dark → auto`

### 11.2 Authentification

Voir §9 pour le détail. Modèle : token dans `localStorage.hermes_admin_token`,
header `X-Admin-Token` sur chaque `fetch()`, aucun cookie, aucune
session serveur.

### 11.3 Comportement des actions

#### Colonne 1 — Catalogue Geofabrik

- **Arbre `<details>`** ouvert par défaut sur `europe` (niveau 1).
  Les pays (niveau 2) restent **fermés** par défaut pour ne pas noyer
  l'utilisateur.
- **Filtre pays** (input) : saisie libre `France, Italie, Suisse…` (CSV).
  - **Matching bilingue** : tape `Italie` matche `italy` (slug) **et**
    `Italie` (nom FR via `Intl.DisplayNames`). Idem `Switzerland` / `Suisse`.
  - **Persistant** dans `localStorage.hermes_admin_countries`.
  - **Défaut 1er lancement** : `France, Italie, Suisse, Espagne`.
  - **Bouton `✕`** : vide le champ (persiste `''` = tout afficher).
    Ne remet pas le défaut.
- **Boutons** :
  - `⊞` : tout ouvrir (tous les `<details>`)
  - `⊟` : tout fermer
  - `⟳` (icône `mdi-refresh`) : vide le cache IndexedDB + fetch forcé
    `GET /geofabrik/regions?refresh=true`
- **Survol** d'une ligne → affiche la `geometry` sur la carte.
- **Ajouter** (state=`absent`) :
  - Confirm avec **slug pré-rempli** : `europe/france/alsace` → `alsace`.
    Le slug est auto-généré, affiché dans le confirm. L'utilisateur
    peut Annuler s'il ne veut pas.
  - `POST /api/v1/regions {id: slug, geofabrik_id: gid}`
  - **Désactivé** si :
    - `size > hermes_admin_max_download_mb` (défaut 500 Mo) → tooltip
      `Taille ~X Go > limite 500 Mo`
    - OU `size == null` ET la région a des enfants (agrégat) → tooltip
      `Région agrégée — ajoutez plutôt une sous-région précise`
- **Mettre à jour** (state=`current`/`stale`/`old`) :
  - Confirm avec `id` local + taille (si connue).
  - `PUT /api/v1/regions/{local_id}` (mapping `geofabrik_id → id_local`
    chargé via `GET /regions`, C4).

#### Colonne 2 — Carte

- **Leaflet 1.9.4** (CDN unpkg), tuiles OSM publiques.
- **Régions configurées** affichées en permanence dans une couche
  persistante, avec couleur selon `state` (voir §11.4). `fitBounds` au
  chargement pour cadrer l'ensemble.
- **Hover** sur une ligne du catalogue ou de la liste régions →
  surbrillance bleue de la `geometry` (couche temporaire au-dessus
  des persistentes).
- **Légende** sous la carte : 3 entrées (À jour / À mettre à jour /
  Ancien). L'entrée `Absent` a été **retirée en v1.9** (pas d'intérêt,
  aucun polygone d'une région absente à afficher).

#### Colonne 3 — Régions configurées

Liste verticale (une carte-ligne par région) :
- Drapeau (via `flagcdn.com` si code ISO connu, fallback 🌐)
- Nom d'affichage (traduit FR si connu, sinon `humanizeId(id)`)
- Badge `state` (couleur voir §11.4)
- Breadcrumb traduit (`Europe · France · Provence-Alpes-Côte d'Azur`)
- `~taille · maj age` (taille PBF **Geofabrik**, pas local — cf. §11.5)
- Badge `has_poly` : icône `mdi-map-outline` (contour réel) ou
  `mdi-square-outline` (fallback bbox)
- Actions : `mdi-refresh` (MAJ) + `mdi-delete` (Suppr)

#### Feedback utilisateur

- **Bandeau d'alerte** sticky sous le header :
  - Succès (vert) : auto-dismiss 5 s
  - Erreur (rouge) : persistant jusqu'au prochain succès
  - Info (bleu) : auto-dismiss 5 s (ou persistant si `duration: 0`)
- **Spinner** sur bouton pendant les actions longues (`POST`/`PUT`/`DELETE`)
- **Anti double-clic** : `btn.disabled = true` avant chaque `fetch`.
  Défense supplémentaire côté serveur : `_OP_LOCK` asyncio (C3).

### 11.4 Couleurs d'état

Utilisées à la fois dans les badges (colonne 1 et 3) et sur la carte :

| `state` | Couleur | Badge | Carte (polygone) |
|---|---|---|---|
| `absent` | rouge | `bg #fde2e2 / fg #b00020` | non affiché (pas de polygone dans regions.json) |
| `current` | vert | `bg #d4f4dd / fg #0a7a3b` | `#0a7a3b`, fillOpacity 0.15 |
| `stale` | vert-jaune | `bg #fff3c4 / fg #8a6d00` | `#8a6d00`, fillOpacity 0.15 |
| `old` | orange | `bg #ffe0c2 / fg #b25e00` | `#b25e00`, fillOpacity 0.15 |

Logique `state` : voir §1.9 (priorité `stale > old > current`).

### 11.5 Notes techniques

**Socle UI** :
- **Pico.css** 2.0.6 (CDN jsdelivr) + **surcharge densification** :
  `--pico-font-size: 87.5%`, `--pico-form-element-spacing-vertical: 0.5rem`,
  `--pico-border-radius: 0.375rem`, `--pico-spacing: 0.75rem`. Objectif :
  interface compacte adaptée à une app d'administration (pas une landing page).
- **Thème** : `data-theme` sur `<html>` (light/dark) + fallback
  `prefers-color-scheme` (auto). Persistance `localStorage.hermes_admin_theme`.
  Toggle dans la toolbar : cycle `auto → light → dark → auto`.

**Carte** : Leaflet 1.9.4 (CDN unpkg), tuiles OSM publiques
(`tile.openstreetmap.org`). Boutons zoom Leaflet corrigés via CSS
ciblé `.leaflet-bar a` (Pico.css cassait leur centrage).

**Icônes** : `@mdi/font` 7.4.47 (CDN jsdelivr), webfont ~1.4 Mo cachée
par le navigateur après premier chargement. Utilisation via classes
`mdi mdi-{nom}`. Aucun SVG inline dans le HTML.

**Drapeaux** : `flagcdn.com/w20/{iso2}.png` (CDN gratuit, toutes
régions ISO 3166-1). Table `COUNTRY_ISO` dans le JS mappe les slugs
Geofabrik → codes ISO (`france → FR`, `italy → IT`, `monaco → MC`, etc.,
~80 entrées). Fallback : emoji 🌐 si code inconnu.

**Traduction** : `Intl.DisplayNames(['fr'], {type: 'region'})` natif
(supporté Safari 14.1+, Chrome 81+, Firefox 86+). Table `CONTINENT_NAMES`
manuelle pour les 8 continents Geofabrik. Les noms anglais originaux
restent accessibles via `title` (survol) sur les nœuds traduits.

**Cache catalogue (T57-I)** :
- Store IndexedDB `hermes_admin` / `catalog`
- Contenu : `{ts, data}` où `data` = réponse `GET /geofabrik/regions`
- TTL : **24 h** côté client (indépendant du cache serveur 7 j)
- **Usage** : au chargement, si cache valide → pas de `GET /geofabrik/regions`.
  Le bouton **Rafraîchir** vide le cache + force `?refresh=true`.
- **Pourquoi IndexedDB et pas localStorage** : le payload complet pèse
  ~3 Mo (principalement `geometry` des 554 régions). localStorage Safari
  est limité à ~5 Mo UTF-16 (2 octets/char) → quota dépassé.
  IndexedDB n'a pas cette limite (stockage binaire efficace).

**Garde-fou téléchargement (T57-I)** :
- Le serveur ne pré-charge `size` que pour les régions `already_added`
  (HEAD lazy, cf. §1.9). Une région `absent` a donc `size = null`.
- Pour éviter un téléchargement continental accidentel (ex. ajouter
  `europe` = ~50 Go), le client **bloque** les Ajouter si :
  - `size > hermes_admin_max_download_mb` (défaut 500 Mo, configurable
    dans `localStorage.hermes_admin_max_download_mb`)
  - OU `size == null` ET la région a des enfants (agrégat type `france`,
    `europe`). Ces agrégats n'ont pas de PBF direct téléchargeable.
- **Conséquence UX** : le tooltip « Ajouter (taille inconnue) » n'existe
  plus. Pour les feuilles sans `size` (ex. `finland`), le tooltip
  s'affiche simplement `Ajouter`. L'utilisateur découvre la taille au
  moment du confirm si connue, ou se fie à son intuition pour les
  petites régions.

**Templates Jinja2** (dans `/opt/hermes-api/app/templates/`) :
- `base.html` : layout + toolbar + modal `<dialog>` token + thème +
  variables CSS Pico + helpers JS (`hermesAdmin.authHeaders()`, etc.)
- `index.html` : vue unifiée (contenu du `{% block content %}`) +
  carte Leaflet + catalogue + liste régions + JS complet
- `regions.html` : stub minimal `{% extends "index.html" %}` + JS de
  redirection vers `/admin/?focus=regions`
- `geofabrik.html` : stub minimal `{% extends "index.html" %}` + JS de
  redirection vers `/admin/?focus=geofabrik`

**Note** : modifier `index.html` impacte les 3 pages (à cause des stubs).
Toujours tester les 3 URLs `/admin/`, `/admin/regions`, `/admin/geofabrik`
après modification (4 × 200 = non-régression).

---

*Contrat v1.10 — 23 novembre 2026. Référence : sequencement.md et
implémentation hermes-api du 23 novembre 2026.*

