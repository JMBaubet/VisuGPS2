# Mission

Tu es un **architecte logiciel senior** spécialisé en applications desktop Tauri et en intégration d'API REST. Tu dois produire un **plan de développement détaillé** pour remplacer, dans l'application **VisuGPS2**, les services locaux de détection **Communes** et **Tunnels** par des appels HTTP à un serveur backend existant nommé **hermes**.

Tu produiras ce plan en **4 grandes étapes minimum** (détaillées ci-dessous), avec pour chaque étape : objectif, fichiers à créer/modifier/supprimer, dépendances, ordre des tâches, risques, critères de validation, effort estimé.

---

## Contexte : l'application cliente VisuGPS2

VisuGPS2 est une application desktop :
- **Tauri v2** (Rust backend) + **Vue 3 Composition API** + **Vuetify 3**
- Édite et visualise des traces GPX (randonnée, vélo)
- Cible : **macOS (M1)** + **Windows**

L'architecture complète est fournie dans **`ARCHITECTURE.md`** (joint). Points essentiels à retenir :

### Modules Rust existants (à remplacer ou nettoyer)
- `src-tauri/src/gpx_communes/` — pipeline Rust : Overpass OSM → polygones → croisement trace
- `src-tauri/src/gpx_tunnels/` — pipeline Rust : lecture PBF depuis **clé USB** → R-tree → matching
- `src-tauri/src/osm/` — gestion des PBF sur **clé USB** (inventaire, catalogue Geofabrik, téléchargements, suppression, géométries)
- `src-tauri/src/gpx_audit/` — Audit GPX (à **conserver**)
- `src-tauri/src/gpx_multiride/` — Passages multiples (à **conserver**)
- `src-tauri/src/gpx_altimetrie/` — Altimétrie (à **conserver**, héberge l'onglet Tunnels)
- `src-tauri/src/trace_info.rs` — Agrégation lecture seule (à **conserver**, à adapter)

### Stores Pinia existants
- `src/stores/communes.ts` — file d'attente Overpass (à remplacer)
- `src/stores/tunnels.ts` — file d'attente locale, édition (à adapter)
- `src/stores/osm.ts` — gestion clé USB (à **supprimer**)
- `src/stores/traces.ts` — registre traces (à conserver)
- `src/stores/altimetrie.ts`, `audit.ts`, `multiride.ts` (à conserver)

### Composants Vue existants
- `src/components/Communes/CommunesDialog.vue` — consultation des communes (à adapter)
- `src/components/Osm/*` — tous à **supprimer** : `OsmFilesDialog.vue`, `OsmFileRow.vue`, `OsmRegionDownloadPanel.vue`, `OsmDeleteDialog.vue`, `OsmStatusButton.vue`, `OsmFlag.vue`
- `src/composables/useCommunesOverpass.ts` — client Overpass (à **supprimer**)
- `src/views/Altimetrie.vue` — héberge l'onglet Tunnels (à adapter)
- `src/views/Accueil.vue` — carte + drawer circuits (à adapter)

### Paramètres (`settings.default.toml`)
- `Osm.*` (inventaire, validité, taille max) — à **supprimer**
- `Communes.Detection.*` (seuils) — à **déplacer** côté serveur (déjà fait)
- `Communes.Overpass.*` — à **supprimer** (plus d'appel Overpass)
- `Tunnels.*` — à **supprimer** (plus de matching local)
- `Altimetrie.*` — à **conserver** (onglet Tunnels reste dans `/altimetrie`)

### Dépendances Rust actuelles (à nettoyer)
- `osmpbfreader` (lecture PBF)
- `rstar` (R-tree spatial)
- `fs4` (espace libre USB)
- `flate2`, `protobuf` (en-tête PBF)
- **Nouvelles à ajouter** : un client HTTP (ex. `reqwest` avec `tokio`)

### Dépendances frontend actuelles (à nettoyer)
- `flag-icons` (drapeaux pays OSM, à vérifier si utilisé ailleurs)

---

## Contexte : le serveur hermes

hermes est un backend **FastAPI (Python 3.12)** sur Mac mini 2012 (Ubuntu 24.04). Il est **déjà entièrement implémenté** (v1.10). L'app M1 doit **consommer** ses routes, sans le modifier.

### Accessibilité
- **ZeroTier uniquement** : `http://10.15.58.131:9000/api/v1`
- **Peut être en veille** (S3). Réveil via **Wake-on-LAN UDP port 9** vers `87.91.31.205` (box FAI) avec MAC `68:5b:35:a8:5c:65`
- Temps de réveil mesuré : ~50 s à ~2 min
- **Un seul dataset chargé en mémoire à la fois** : basculer sur un autre dataset décharge le précédent (rechargement ~2 s au retour)
- **API non authentifiée** pour M1 (sauf `POST /datasets/{id}/aux/prepare` qui est admin, avec `X-Admin-Token`)

### 18 routes exposées (voir `hermes-api-contract.md` joint)
- **Datasets** : `GET/POST/DELETE /datasets`, `GET /datasets/{id}`
- **Instances ORS** : `GET /instances`, `POST /instances/{id}/start|stop`
- **Régions** : `GET /regions`
- **Polygones custom** : `GET/POST/PUT/DELETE /polygons`, `GET /polygons/{id}`
- **Communes** (nouveau) : `POST /communes/detect`
- **Tunnels** (nouveau) : `POST /tunnels/detect`
- **Aux** (nouveau) : `GET /datasets/{id}/aux`, `POST /datasets/{id}/aux/prepare` (admin)
- **Système** : `GET /ping`

### Prérequis serveur pour que `communes/detect` et `tunnels/detect` fonctionnent
1. Un **dataset** doit exister (créé via `POST /datasets`)
2. Ses **artefacts auxiliaires** doivent être générés :
   - `communes.geojson` (pour `/communes/detect`)
   - `tunnels-logiques.json` + `mm/` (pour `/tunnels/detect`)
3. La génération se fait via `POST /datasets/{id}/aux/prepare` (**admin**, `X-Admin-Token`) ou CLI serveur
4. Sans ces artefacts, les routes retournent `404` avec un message explicite
5. **Un dataset couvre une région géographique** (PACA, Monaco, PACA+Italie NW…). Une trace hors couverture renverra `passages: []` ou `tunnels: []` (pas d'erreur)

### Limitation documentée
- **Tunnels urbains denses non détectés** (Monaco, Nice centre-ville) : le serveur renvoie `tunnels: []` même si la trace traverse un tunnel. Cause : map_matching_2 n'utilise pas l'altitude, GPX édités avec altitude DEM lissée. **L'app M1 doit avertir l'utilisateur** que la détection est indicative en ville, fiable sur tunnels isolés (alpins, autoroutiers).

---

## Objectif du plan

Produire un plan de développement **construit en minimum 4 grandes étapes**, avec pour chaque étape le niveau de détail décrit en fin de prompt.

### Étape 1 — Intégration CRUD des Datasets (polygones, régions, jeux de données)

Remplacer la gestion locale des PBF (clé USB) par une gestion des datasets **côté hermes**.

Sous-objectifs :
- **Nouveau store Pinia `hermes.ts`** (client HTTP vers l'API hermes) : datasets, régions, polygones, instances, ping, WoL.
- **CRUD Datasets** : création, liste, détail, suppression, démarrage/arrêt ORS (`POST /instances/{id}/start|stop`)
- **CRUD Polygones custom** : création, édition, suppression (mode `zones` de `POST /datasets`)
- **Liste des régions** : `GET /regions` → sélection par l'utilisateur
- **Gestion du statut serveur** :
  - Ping régulier (`GET /ping`, keep-alive 30 s)
  - Détection de veille (ping KO) → proposition de réveil WoL
  - Attente post-WoL (polling ping, timeout ~120 s)
- **UI** : nouvelle vue `/hermes` dédiée ? Intégration dans un drawer existant ? (à proposer)
- **Stratégie de sélection du dataset actif** : comment l'app choisit-elle le dataset à utiliser pour une trace ? (par région détectée ? manuellement ? mémorisé ?)
- **Migration** : que faire des traces existantes qui ont un `communes.json`/`tunnels.json` local ? Les garder (compatibilité lecture seule) ou les réinitialiser (recalcul via hermes) ?

### Étape 2 — Fonction recherche des Communes (via hermes)

Remplacer `useCommunesOverpass.ts` (appel direct Overpass) par des appels à `POST /communes/detect` sur hermes.

Sous-objectifs :
- **Modifier `src/stores/communes.ts`** :
  - Supprimer la file d'attente Overpass (plus de requêtes concurrentes OSM)
  - Ajouter un appel HTTP au dataset actif
  - Vérifier la disponibilité des artefacts (`GET /datasets/{id}/aux` → `communes.exists`)
  - Gérer les erreurs : 404 (artefacts absents), 500 (erreur interne), dataset non couvert
- **Supprimer `useCommunesOverpass.ts`**
- **Supprimer `src-tauri/src/gpx_communes/`**
- **Adapter `CommunesDialog.vue`** : le format de réponse hermes (`passages[]` avec `commune`, `passage`, `kmEntree`, `kmSortie`, `longueurKm`, `ambiguous`) est **différent** du format local actuel. Adapter le rendu.
- **Que faire de la persistance `communes.json` ?** Deux options à trancher :
  - Garder `communes.json` comme **cache local** (écrit après chaque appel, relu au démarrage)
  - Supprimer `communes.json` (appel systématique à hermes à la demande)
- **UX** :
  - Message clair si le dataset actif n'a pas `communes.geojson`
  - Message clair si la trace est hors couverture (passages vides)
  - Afficher `ambiguous: true` différemment (frontière OSM imprécise)
- **Barrière existante** : la détection de communes doit-elle rester une barrière pour l'édition caméra ? (actuellement non)

### Étape 3 — Fonction recherche des Tunnels (via hermes)

Remplacer `src-tauri/src/gpx_tunnels/` (lecture PBF USB) par des appels à `POST /tunnels/detect` sur hermes.

Sous-objectifs :
- **Modifier `src/stores/tunnels.ts`** :
  - Supprimer la file d'attente locale et le pipeline Rust
  - Ajouter un appel HTTP au dataset actif
  - Vérifier la disponibilité des artefacts (`GET /datasets/{id}/aux` → `tunnels.exists && mm.exists`)
  - Gérer les erreurs : 404 (artefacts absents), 500 (map_matching_2 absent), dataset non couvert
- **Supprimer `src-tauri/src/gpx_tunnels/`**
- **Adapter l'onglet Tunnels de `/altimetrie`** :
  - Le format de réponse hermes (`tunnels[]` avec `id`, `groupId`, `name`, `startKm`, `endKm`, `lengthM`, `wayIds`, `altimetricOk`) diffère du format local actuel
  - La sélection/marqueurs sur la carte (`AltimetrieMap.vue`) et le widget (`AltimetrieProfileWidget.vue`) doivent être adaptés
- **Limitation Monaco** : message d'avertissement pour les traces urbaines (`tunnels: []` peut être un vrai négatif)
- **Édition manuelle** : le store actuel permet de créer/fusionner/séparer des tunnels. Que devient cette édition avec hermes (pas d'endpoint d'édition) ?
  - Option A : consulter uniquement (lecture seule)
  - Option B : édition locale puis fusion avec la réponse hermes
  - Option C : conserver une édition locale séparée (overlay client)
- **Barrière existante** : la validation des tunnels doit-elle rester dans `/altimetrie` ?

### Étape 4 — Nettoyage du code mort

Une fois les étapes 1-3 validées, supprimer :
- **Rust** : `src-tauri/src/osm/`, `src-tauri/src/gpx_communes/`, `src-tauri/src/gpx_tunnels/`
- **Frontend** : `useCommunesOverpass.ts`, `src/components/Osm/*` (6 fichiers), `src/stores/osm.ts`
- **Paramètres** : `Osm.*`, `Communes.Detection.*`, `Communes.Overpass.*`, `Tunnels.*` de `settings.default.toml`
- **Commandes Tauri** : `osm_*`, `communes_*`, `tunnels_*` de `lib.rs` et `generate_handler!`
- **Capabilities** : entrées de `default.json` devenues inutiles
- **Dépendances Rust** : `osmpbfreader`, `rstar`, `fs4`, `flate2`, `protobuf` (vérifier qu'aucun autre module ne les utilise)
- **Dépendances npm** : `flag-icons` (vérifier usages résiduels)
- **Fichiers de données** : `.osm.pbf` sur la clé USB (suppression manuelle par l'utilisateur, hors code)
- **Stores** : `Osm.ts`, champs `Osm.*` dans `settings.ts`
- **Composants** : voir liste complète ci-dessus
- **Commandes** : `OsmStatusButton` dans la toolbar de l'Accueil (remplacé par `HermesStatusButton` ?)

Vérifier aussi :
- `trace_info.rs` : adapter `get_trace_info` (le volet Info de la carte lit `communes.json` et `tunnels.json` — si ces fichiers disparaissent, le volet Info doit être adapté)
- `settings.default.toml` : cohérence `_meta` après suppression des catégories

---

## Contraintes techniques à prendre en compte

1. **Serveur non disponible (veille S3)** :
   - WoL UDP/9 vers `87.91.31.205`
   - Polling `GET /ping` après WoL (timeout 120 s, poll 3 s)
   - Messages UX : "Serveur en veille, réveil en cours…" / "Serveur indisponible après 2 min"
   - Décider : WoL automatique ou manuel (bouton) ?

2. **Un seul dataset en mémoire** :
   - Si l'app travaille sur plusieurs datasets, chaque bascule invalide le cache des index (rechargement ~2 s)
   - Stratégie : garder un dataset actif **unique** pendant une session, ou basculer explicitement (message "Changement de dataset, rechargement…")

3. **Prérequis artefacts aux** :
   - `communes/detect` nécessite `communes.geojson` → `GET /aux` pour vérifier
   - `tunnels/detect` nécessite `tunnels-logiques.json` + `mm/`
   - **L'app M1 ne peut pas lancer `POST /aux/prepare` (admin)** → UX : message "Fonction indisponible pour ce dataset, contactez l'admin"

4. **Traces hors couverture** :
   - `POST /communes/detect` renvoie `passages: []` (pas d'erreur) si la trace est hors dataset
   - **Faux négatif ambigu** : comment distinguer "hors couverture" de "vraiment aucune commune" ? (suggestion : vérifier bbox trace vs bbox dataset en amont)

5. **Limitation Monaco** (tunnels urbains) :
   - Message d'avertissement si la trace traverse un dataset urbain dense
   - Documentation visible dans l'UI

6. **Migration** :
   - Traces existantes avec `communes.json` / `tunnels.json` locaux : garder (compatibilité) ou recalculer ?
   - Traces avec `audit_status = clean` mais sans artefacts hermes : comment réagir ?

7. **Auth admin** :
   - `POST /aux/prepare` est admin → l'app M1 ne peut pas déclencher la génération
   - **Alternative** : un mode "avancé" côté app M1 avec token admin stocké localement (secret chiffré) ?

8. **Multi-plateforme** :
   - HTTP client côté **Rust** (`reqwest` + `tokio`) recommandé pour la cohérence inter-modules, ou côté **JS** (`fetch` natif WebView) ?
   - Recommandation : Rust pour les commandes sensibles (WoL, auth admin), JS pour les appels lecture simples

9. **Cohérence avec les barrières existantes** :
   - `audit_status` (clean/needs_review)
   - `multiride_status` (pending/validated/none)
   - `multiride_complete`
   - La détection de communes/tunnels doit-elle devenir une 3ᵉ barrière ? (recommandation : non, rester une consultation)

10. **Cohérence des stores** :
    - Les stores actuels (`communes.ts`, `tunnels.ts`) exposent `request(traceId)`, `startCatchUp()`, état `loading`, file d'attente
    - L'API hermes étant **synchrone** (une requête = une réponse HTTP), la file d'attente et le "temps mort" entre traces **disparaissent** → simplifier les stores

---

## Format attendu

Pour **chaque étape**, fournis :

1. **Objectif** (2-3 phrases)
2. **Fichiers à créer** (chemin + rôle + contenu attendu)
3. **Fichiers à modifier** (chemin + nature du changement + extrait de code si pertinent)
4. **Fichiers à supprimer** (chemin + justification)
5. **Dépendances** (ajouts/retraits Rust + npm)
6. **Ordre des tâches** (séquencement logique, dépendances internes)
7. **Points de risque** (liste, avec mitigation pour chacun)
8. **Critères de validation** (tests unitaires, tests manuels, cas d'usage à couvrir)
9. **Effort estimé** (en heures, avec sous-décomposition)
10. **Stratégie de rollback** (si l'étape échoue en cours de route)

Et en **fin de plan**, ajoute :

- **Vue d'ensemble** (diagramme ou texte) : flux M1 → hermes après les 4 étapes
- **Dépendances entre étapes** (1 → 2, 1 → 3, 2+3 → 4, etc.)
- **Effort total estimé** (en jours)
- **Checklist de livraison finale** (ce qui doit être vrai à la fin)
- **Points ouverts** : décisions à trancher avec le product owner avant de commencer (ex. persistance `communes.json`, édition tunnels locale, mode admin optionnel, stratégie dataset unique/multi)

## Documents joints

- **`ARCHITECTURE.md`** — Architecture complète de VisuGPS2
- **`hermes-api-contract.md` v1.10** — Contrat client M1 ↔ hermes (routes, payloads, cycle de vie, gestion erreurs, limitations)
- **`communes & tunnels.md` v1.10** — Spécification des services Communes/Tunnels côté serveur (algorithmes, format de données, limitation Monaco A15, points de vigilance)

Si tu as besoin d'informations supplémentaires (ex. format exact d'un `communes.json` actuel, schéma complet de `settings.default.toml`, extrait de `tunnels.ts`), demande-les avant de commencer. Sinon, produis le plan complet en te basant sur ces 3 documents.