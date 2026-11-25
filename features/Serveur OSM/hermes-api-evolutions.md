# Évolutions API hermes demandées par l'IHM — proposition v1.11

> Référence : `hermes-api-contract.md` v1.10 (23 novembre 2026) et
> `sequencement.md` v1.10.
> Public : développement du serveur `hermes-api`.
> Contexte : vue « Hermes » de l'application VisuGPS2 (carte Serveur, onglets
> Secteurs et Jeu de données).

Ce document liste les informations dont l'IHM a besoin et qui ne sont pas
exposées aujourd'hui par l'API. Chaque point indique le comportement de repli
implémenté côté client tant que le serveur n'a pas évolué.

## Synthèse

| # | Évolution | Nécessité |
|---|---|---|
| 1 | `name` dans les manifests de dataset | Indispensable pour afficher un nom plutôt que l'id |
| 2 | `needsUpdate` / `zonesRevisions` (secteurs modifiés depuis le build) | Indispensable pour un état « à mettre à jour » fiable |
| 3 | `GET /serving` → `stopped` après `POST /instances/{id}/stop`, `datasetId` conservé | À vérifier / garantir (les boutons Arrêter/Lancer en dépendent) |

Les points 4 à 6 sont **déjà couverts** par le contrat actuel : aucune
modification n'est nécessaire.

---

## 1. Nom d'affichage des jeux de données

**Problème** : `POST /datasets` accepte déjà un champ `name` optionnel, mais
`GET /datasets` et `GET /datasets/{id}` ne le retournent pas. L'IHM ne peut
afficher que l'identifiant technique.

**Demande** : ajouter `name` (string ou `null`) à chaque manifest, y compris
dans la réponse de `GET /datasets/{id}`.

```json
[
  {
    "id": "provence-alpes-cote-dazur",
    "name": "Provence-Alpes-Côte d'Azur",
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

Règles attendues :

- `name` reprend la valeur fournie à la création (`POST /datasets`), stockée
  dans `manifest.json` ;
- `null` (ou champ absent) pour les manifests créés avant l'ajout du champ ;
- aucun impact sur les identifiants de fichiers ou de routes.

**Repli IHM** : tant que le champ est absent, l'IHM affiche `id`.

---

## 2. État « à mettre à jour » d'un dataset en mode zones

**Problème** : un dataset en mode `zones` assemble des polygones. Si un
polygone est modifié (`PUT /polygons/{id}`) après la construction du dataset,
rien ne signale que le dataset ne correspond plus au terrain. Le champ `stale`
est réservé aux régions et vaut toujours `false` en mode `zones` (Q5.2, v1.8).

**Demande** : mémoriser les révisions réellement utilisées au build, puis
exposer un booléen `needsUpdate` dans `GET /datasets` et
`GET /datasets/{id}`.

### Manifest enrichi

```json
{
  "id": "Monaco",
  "name": "Monaco",
  "regions": null,
  "zones": ["ma-zone-nice", "ma-zone-monaco"],
  "zonesRevisions": {
    "ma-zone-nice": 3,
    "ma-zone-monaco": 1
  },
  "created_at": "2026-11-23T12:00:00Z",
  "status": "ready",
  "state": "ready",
  "stateLabel": "Prêt",
  "size_bytes": 133000000,
  "ors_version": "9.9.0",
  "stale": false,
  "needsUpdate": true
}
```

### Règles de calcul

- `zonesRevisions` est écrit dans `manifest.json` **au moment du build**, avec
  la révision courante de chaque polygone utilisé (celle qui a servi au
  clippage). La valeur peut être `null` pour les manifests antérieurs.
- `needsUpdate = true` si, au moment de la lecture :
  - un polygone de `zones` n'existe plus, ou
  - la `revision` courante d'un polygone de `zones` diffère de
    `zonesRevisions[id]`.
- `needsUpdate = false` en mode `regions` (le champ `stale` reste la référence).
- Champ optionnel `needsUpdateReason` (`"zones"` ou `null`) si vous souhaitez
  distinguer plusieurs causes plus tard.
- Ce calcul est **dérivé à la lecture** : aucun rebuild automatique, la
  reconstruction reste une décision de l'utilisateur.

**Repli IHM** : en attendant, l'IHM compare `polygon.updated_at` à
`dataset.created_at` (un polygone modifié après la création du dataset affiche
le chip « à mettre à jour »). Cette heuristique est approximative et sera
remplacée par `needsUpdate` dès qu'il sera disponible.

**Mise à jour côté client** : le bouton « Mettre à jour » de l'IHM applique le
flux documenté `DELETE /datasets/{id}` puis `POST /datasets` avec le même
`name`. Il faut donc que la recréation conserve **le même `id`** (la sélection
du dataset servi est conservée par id) ; l'IHM avertit l'utilisateur si l'id
retourné diffère.

---

## 3. Arrêt sans perte de sélection

**Problème** : l'IHM propose un bouton « Arrêter » puis « Lancer » pour
l'instance ORS, tout en gardant affiché le nom du jeu de données sélectionné.
`DELETE /serving` ne convient pas : il efface la sélection.

**Demande** : garantir la machine à états déjà documentée dans
`sequencement.md` §8 :

```text
active --> stopped       : POST /api/v1/instances/{id}/stop
stopped --> starting     : POST /api/v1/instances/{id}/start
```

- après `POST /instances/{id}/stop`, `GET /serving` renvoie
  `state: "stopped"` en **conservant** `datasetId` ;
- après `POST /instances/{id}/start`, `GET /serving` renvoie
  `state: "starting"`, puis `active` lorsque `GET /ors/v2/health` est `ready` ;
- `DELETE /serving` reste l'unique route qui efface la sélection ; l'IHM ne
  l'utilise plus depuis la carte Serveur.

Exemple attendu après un arrêt :

```json
{
  "datasetId": "Monaco",
  "state": "stopped",
  "stateLabel": "Arrêté",
  "datasetState": "ready",
  "since": "2026-11-24T09:30:00Z",
  "reason": null
}
```

**À vérifier sur le serveur déployé** : que `POST /instances/{id}/stop` met
bien à jour `serving.json` (et pas seulement l'instance systemd). Si ce n'est
pas le cas, l'IHM affichera encore « Arrêter » après l'arrêt.

---

## 4. Noms des polygones dans un dataset — pas nécessaire

L'IHM charge `GET /polygons` et joint les `zones` du dataset sur `id` pour
afficher les noms. Aucune évolution n'est nécessaire.

Si vous préférez une réponse auto-suffisante, la forme optionnelle serait :

```json
"zonesDetails": [
  { "id": "ma-zone-nice", "name": "Zone Nice - Col de Turini", "revision": 3 }
]
```

Ce n'est **pas demandé** : la jointure locale est déjà implémentée.

## 5. Superficie d'un polygone — pas nécessaire

La superficie (km²) est calculée côté client (`polygonAreaKm2`, Shoelace +
correction `cos(lat)`), conformément au contrat §8 qui ne l'expose pas.
Aucune évolution n'est nécessaire.

## 6. Aux dégradés, taille et artefacts — déjà couverts

- `state` (`degradedCommunes`, `degradedTunnels`, `degradedAux`) et
  `artifacts[*].state` suffisent à afficher les chips `Communes`, `Tunnels`,
  `f(Tunnels/mm)` et à proposer la relance
  `POST /datasets/{id}/aux/prepare` avec `{ "force": true }`.
- `size_bytes` est déjà exposé pour l'affichage de debug.
- Aucune évolution n'est nécessaire.

---

## Récapitulatif des replis IHM

| Donnée | Repli actuel |
|---|---|
| Nom du dataset | `id` |
| « À mettre à jour » | `polygon.updated_at > dataset.created_at` |
| Arrêt/Lancer | `POST /instances/{id}/stop|start` (déjà utilisés) |
