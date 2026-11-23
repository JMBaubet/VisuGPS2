# Points d'attention — vue Hermes

> État au 10/11/2026, branche `feature/hermes-integration`.
> Ce document ne décrit pas le fonctionnement nominal : il rassemble les
> décisions confirmées, les points ouverts et les pièges à ne pas réintroduire
> lors des prochaines sous-étapes.

## 1. Statut du dataset actif et couverture

- [x] **« Dataset sélectionné » = dataset actif**, persisté dans
      `Hermes.Dataset.actif` (paramètre caché).
- [x] **Seuls les datasets `ready` sont sélectionnables** dans le sélecteur de
      la carte Serveur.
- [ ] **Couverture et statuts** : la carte de l'accueil dessine en bleu la
      couverture du dataset actif quel que soit son statut (`ready`,
      `preparing`, `failed`, `stale`). Décider s'il faut la limiter aux
      datasets `ready`.
- [ ] **Dataset `failed` encore actif** : `loadDatasets` ne désactive que si le
      dataset a disparu du serveur. Un dataset `failed` reste donc actif, son
      chip affiche « échec » et sa couverture reste dessinée. Décider s'il faut
      le désactiver automatiquement.
- [ ] **Dataset `stale`** : aucun style spécifique n'est appliqué à sa
      couverture ni à ses couches. Décider si un signal visuel est nécessaire.
- [ ] **Préparation aux d'un dataset déjà `ready`** : l'appel automatique ne
      se produit que sur la transition vers `ready` observée par le polling de
      build. Un dataset déjà `ready` avant l'ouverture de l'application
      (ex. `EB-Menton`) doit être préparé avec le bouton manuel.

## 2. Préparation des artefacts aux (`/aux/prepare`)

- [x] Commande Tauri `hermes_prepare_aux` :
      `POST /datasets/{id}/aux/prepare` avec `{ force, skipMm }`.
- [x] Appel automatique après `dataset.status === 'ready'` dans le polling de
      build, avec `force=false`.
- [x] Bouton dans la carte Serveur pour le dataset actif :
      `force=true` si les trois artefacts existent (relance complète),
      `force=false` si l'un manque (génération des manquants).
- [x] Un `409` est absorbé : une préparation (ou un build) est déjà en cours ;
      le store se contente de suivre `GET /datasets/{id}/aux`.
- [x] Suivi : polling toutes les 3 s, budget 45 min, keep-alive actif pendant
      la préparation.
- [x] `skipMm` est toujours `false` depuis l'IHM : la préparation doit produire
      communes, tunnels **et** `mm/`.
- [x] Les erreurs `lastError` du serveur sont affichées sous le bouton
      uniquement si les artefacts manquent encore.
- [ ] **Portée du bouton** : il ne concerne que le dataset actif. Aucune action
      de préparation par ligne n'existe dans l'onglet Datasets. Décider si une
      action par dataset est nécessaire.
- [ ] **Préparations concurrentes** : un seul suivi à la fois. Un appel
      automatique pour un autre dataset pendant une préparation est ignoré
      silencieusement. À revoir si le serveur autorise ou ordonnance plusieurs
      préparations.
- [ ] **Cas d'un `409` pour un autre motif** : le code traite tout `409` comme
      « préparation déjà en cours ». Si le serveur renvoie `409` pour un build
      en cours ou une autre préparation, le suivi peut conclure à des
      artefacts incomplets sans expliquer la vraie cause.
- [ ] **Contrat local** : `features/Serveur OSM/hermes-api-contract.md` (non
      suivi) indique encore que `/aux/prepare` est réservé à l'admin. À mettre
      à jour ou à ignorer.

## 3. Carte Hermes

- [x] **Style du polygone édité** centralisé dans
      `HERMES_DRAW_POLYGON_STYLE` (`src/components/Hermes/hermesMapLayers.ts`),
      appliqué via l'option `styles` du constructeur Mapbox Draw
      (`buildDrawStyles` dans `HermesMap.vue`).
- [x] Ne pas revenir à un `setPaintProperty` immédiat après `addControl` :
      Mapbox Draw diffère `addLayers` tant que `map.loaded()` est faux, et les
      couches n'existent pas encore à cet instant.
- [x] Formulaire polygone en **tiroir droit non modal** : la carte reste
      interactive pour déplacer les sommets et utiliser le marqueur distance.
- [x] Marqueur distance : cercles de 10 à 150 km, renforcés tous les 50 km,
      étiquettes 50/100/150 km au nord, en mauve (`#8E24AA`). C'est un repère
      visuel, non persisté avec le polygone.
- [ ] **Couleurs Draw** : le bleu actuel (`#2196F3` / `#0D47A1`) est un choix
      par défaut ; d'autres contrastes restent à tester.
- [x] Filtres : `flex: 0 0 auto` sur `.hermes-section__filter`. Vuetify
      `.v-input` a `flex: 1 1 auto` ; sans cette règle, le champ grandit
      verticalement et pousse l'encart et la liste vers le bas.
- [x] États vides : `min-height: 0` et `justify-content: flex-start`. Le
      `v-empty-state` de Vuetify se centre et débordait du panneau.
- [x] Filtre ajouté dans l'onglet Datasets (identifiant, régions, zones).
- [ ] **Traces favorites/affichées** : couleurs et épaisseurs lues au chargement
      de la carte ; pas de mise à jour à chaud si les paramètres `Carte.*`
      changent pendant que la vue Hermes est ouverte.

## 4. Carte de l'accueil

- [x] Couverture du dataset actif : source et couches
      `hermes-dataset-coverage`, bleu Material (`#2196F3` à 18 %,
      contour `#1976D2`), insérées avant `clusters` pour rester sous les traces.
- [x] Aucun recadrage automatique lié à la couverture.
- [ ] **Double chargement** : le chip Hermes et `ensureCoverageData` appellent
      tous deux `loadDatasets` au montage de l'accueil. Sans conséquence
      fonctionnelle, mais un garde-fou de requête en cours peut être ajouté.
- [ ] **Couverture et statuts** : voir §1.

## 5. Contrat, sécurité et dépôt

- [x] Aucun `X-Admin-Token` n'est stocké ni transmis par le client.
- [x] Le serveur autorise désormais `POST /datasets/{id}/aux/prepare` sans
      jeton (selon la correction serveur du 10/11/2026).
- [ ] **Contrat local non suivi** : `features/Serveur OSM/hermes-api-contract.md`
      doit être actualisé (route aux non admin, préparation intégrée à la
      création de dataset).
- [ ] **Captures `tmp/`** : dossier non suivi ; ne pas committer.
- [ ] **Sous-étape 1.11** : tests manuels de bout en bout et documentation
      (`docs/HERMES.md`, `docs/CONTEXT.md`, `docs/ARCHITECTURE.md`,
      `docs/DATA_STORAGE.md`).

## 6. Commits de référence

| Commit | Objet |
|---|---|
| `507b138` | `fix(hermes): fiabiliser le polling des datasets` |
| `eb97514` | `feat(hermes): mettre en évidence les données de l'onglet actif` |
| `9aeee4c` | `feat(hermes): afficher les traces favorites et visibles` |
| `b024727` | `feat(hermes): ajouter l'outil de cercles de distance` |
| `276adcf` | `fix(hermes): appliquer le style du polygone en édition` |
| `be50bdc` | `fix(hermes): corriger les filtres et ajouter celui des datasets` |
| `1e439d9` | `feat(accueil): afficher la couverture du dataset hermes actif` |
| `b1321ae` | `feat(hermes): préparer les artefacts aux et permettre la relance` |
