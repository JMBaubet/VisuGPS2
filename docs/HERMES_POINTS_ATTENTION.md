# Points d'attention — vue Hermes

> État au 24/11/2026, branche `feature/hermes-integration`.
> Ce document ne décrit pas le fonctionnement nominal : il rassemble les
> décisions confirmées, les points ouverts et les pièges à ne pas réintroduire
> lors des prochaines sous-étapes.

## 0. Décisions du 23/11/2026

- [x] **Onglet Régions supprimé.** Les régions indexées ne sont plus une liste
      sélectionnable : elles restent affichées **en permanence** sur la carte
      Hermes, en couche de fond jaune, sur les deux onglets Secteurs et Jeu de
      données.
- [x] **Création de polygone** : le panneau droit s'ouvre dès le clic sur
      « Nouveau », avant le tracé. Il affiche un brouillon (nombre de sommets,
      surface estimée, avertissement si la limite est déjà dépassée) et un
      bouton « Redessiner » qui relance le tracé sans perdre le nom saisi. Une
      fois le polygone fermé, la surface finale est revalidée, mais les sommets
      restent modifiables (`direct_select`) pour repasser sous la limite.
- [x] **Couleur des polygones retirée de l'IHM.** Le champ n'est plus saisi ni
      envoyé ; il reste dans la réponse serveur (`HermesPolygon.color`) jusqu'à
      son retrait de l'API.
- [x] **Édition complète des sommets** : ajout par clic sur un point
      intermédiaire, déplacement par glisser, suppression du sommet sélectionné
      par `Suppr`/`Backspace`. La suppression était inopérante car
      `displayControlsDefault: false` met `controls.trash` à `false` et
      désactive le gestionnaire clavier natif de Mapbox Draw ; un écouteur
      dédié appelle `draw.trash()` en `direct_select`.
- [x] **Marqueur distance indépendant** : disponible en permanence sur les deux
      onglets, il n'est plus effacé au changement de session de dessin ni au
      retour en `idle`.
- [x] **Datasets composés uniquement de polygones.** Le mode « régions » n'est
      plus proposé à la création ; les datasets historiques restent lisibles.
- [x] **Limites paramétrables** (`Hermes.Limites.*`, unité **km²**) :
      `maxPolygonesParDataset` (défaut 2, 1 à 10),
      `surfaceMaxPolygoneKm2` (défaut 10 000, 1 000 à 100 000, pas 500) et
      `surfaceMaxDatasetKm2` (défaut 50 000, 1 000 à 100 000, pas 500).
      La surface du dataset est la **somme de ses polygones** : elle est
      distincte de la surface maximale d'un polygone pris isolément.
      Contrôle dans les formulaires et revalidation côté Rust.

## 0 bis. Décisions du 24/11/2026

- [x] **Carte Serveur allégée.** Elle n'affiche plus ni URL, ni dernier ping,
      ni sélecteur de dataset, ni états aux, ni bouton « Tester ». Elle montre
      l'état du service et le nom du jeu de données sélectionné, avec **une**
      action : Arrêter (rouge) / Lancer (vert), remplacée par Réveiller (bleu)
      quand le ping échoue. Les deux actions sont désactivées pendant une
      construction (build de dataset ou chaîne aux).
- [x] **Arrêter conserve la sélection** : `POST /instances/{id}/stop` puis
      `POST /instances/{id}/start` pour Lancer. `DELETE /serving` n'est plus
      appelé par la carte, le nom du dataset reste affiché.
- [x] **Sélection depuis la liste des jeux de données** : icône par ligne
      (sélectionné / sélectionnable / indisponible) puis dialogue de
      confirmation semi-transparent (50 %). Le dataset est d'abord
      **prévisualisé** sur la carte (surbrillance bleue + cadrage sur son
      périmètre), puis la confirmation s'ouvre après un court délai.
- [x] **Reconstruction d'un dataset « à mettre à jour »** : bouton dédié sur la
      ligne, avec confirmation qui prévient de l'interruption du service. Flux
      serveur : `DELETE /datasets/{id}` puis `POST /datasets` avec le même nom
      (le contrat conserve la sélection par id). Refusé si une autre
      construction est en cours (un `POST` concurrent renverrait 409 après la
      suppression).
- [x] **Onglets renommés et réordonnés** : « Secteurs » à gauche,
      « Jeu de données » à droite. Chaque changement d'onglet relance un
      `refresh()` (serveur, datasets, aux, régions, secteurs).
- [x] **Lignes Secteurs** : nom, puis surface · date de modification ·
      révision ; l'id n'apparaît qu'en mode debug. La poubelle est masquée dès
      qu'un jeu de données assemble le secteur.
- [x] **Lignes Jeu de données** : nom (champ serveur `name`, repli `id`),
      noms des secteurs assemblés, chips aux orange (`Communes`, `Tunnels`,
      `f(Tunnels/mm)`) avec bouton de relance `force=true`, taille/ORS en mode
      debug.
- [x] **État « à mettre à jour »** : heuristique locale
      (`polygon.updated_at > dataset.created_at`) affichée en chip. L'état
      fiable (`needsUpdate` + `zonesRevisions`) est spécifié dans
      `features/Serveur OSM/hermes-api-evolutions.md` ; le serveur doit aussi
      exposer `name` et garantir `stopped` après un arrêt d'instance.
- [x] **Relance aux par dataset** : le bouton de la carte Serveur est supprimé
      et réapparaît sur chaque jeu de données dont au moins un aux est dégradé.
- [x] **Serveur et service sont distincts.** Le **serveur** (machine +
      hermes-api) a ses propres états : `online`, `offline` (ping en échec),
      `waking` (WoL envoyé), `lost` (réveil resté sans réponse). Le **service**
      (instance ORS) garde `active|waiting|starting|stopped|none|failed`.
      Le chip d'en-tête décrit le service : bleu « Attente fin de
      construction »/« Démarrage en cours », vert « Actif », orange « Arrêté »,
      rouge « Aucun service » (serveur joignable sans dataset) ou « Échec ».
      « Actif » n'apparaît qu'une fois le build **et** la chaîne aux terminés :
      tant qu'un dataset — le servi ou un autre — est en construction, le chip
      reste « Attente fin de construction », cohérent avec les boutons
      Arrêter/Démarrer le service désactivés. Cet état **prime sur
      « Démarrage en cours »** : le serveur peut repasser brièvement en
      `starting` pendant la chaîne aux, sans faire clignoter le chip. Quand le
      serveur est muet, le chip affiche **« Inconnu » (gris)** : « Aucun
      service » est réservé au serveur joignable. Après un réveil échoué
      (`lost`), le message « Perte du serveur » s'affiche et le bouton devient
      **« Réessayer le réveil »**.
      Le dataset servi est rendu avec la même ligne que l'onglet Jeu de données
      (partagée via `HermesDatasetRow`), sans poubelle, avec une coche colorée
      comme son chip d'état.
- [x] **Ping API permanent (service), ping système en secours (machine).** Le
      moniteur global (App.vue, fenêtre principale) envoie `GET /ping` pendant
      toute la durée de l'application : période
      `Hermes.Connexion.pingApiSec`, défaut **10 s**, bornes **3 à 60 s**.
      Il détecte l'arrêt du service sans attendre l'ouverture de la vue
      Hermes ; comme il touche l'API, il maintient le service éveillé tant que
      l'application tourne. Quand l'API ne répond pas, un ping **ICMP** de
      l'hôte de `Hermes.Connexion.baseUrl` est lancé **une fois** pour lever le
      doute : machine joignable ⇒ chip orange « Service arrêté » ; machine
      muette ⇒ chip « Inconnu ». Les deux situations ouvrent une **popup**
      dédiée (service ou serveur), **rendue dans la vue Hermes uniquement** :
      la carte n'affiche aucun message pour la perte de service, seulement le
      chip. La popup serveur recommande Tailscale, Internet et le réveil ;
      chaque alerte n'est ouverte qu'une fois par panne. Le ping système n'est
      pas périodique ; il sert aussi à la boucle de réveil.
- [x] **Réveil WoL au lancement.** Le serveur se met en veille par défaut : une
      trame WoL est envoyée **systématiquement** à l'ouverture de l'application
      (fenêtre principale uniquement, avant le moniteur de santé). L'état
      `waking` est posé avant le premier `await` pour que le moniteur ne
      déclare pas le serveur injoignable pendant le démarrage ; la boucle suit
      ensuite le ping système (repli API) jusqu'à `Hermes.Reveil.delaiMaxSec`.
- [x] **Build et état du service.** Un build arrête ORS côté serveur.
      `createDataset` relit `GET /serving` juste après le `POST` et force
      l'état **« Attente fin de construction » (bleu)** si le serveur n'a pas
      encore publié la transition ; `pollDataset` recharge `/serving` à chaque
      passage, puis reprend le suivi de l'instance au retour du graphe. Le chip
      ne reste plus bloqué sur « Actif » pendant une construction — y compris
      pendant la chaîne aux, où `constructionInProgress` couvre `preparing` et
      les artefacts aux en cours. Le message « construction en cours » n'est
      affiché qu'une fois, à droite des boutons, en bleu.
- [x] **Chips Initialisation/Construction en bleu** (au lieu d'orange), pour
      distinguer la progression des états dégradés (orange).
- [x] **Erreurs réseau adoucies** : `parseHermesError` remplace les messages
      bruts (`tcp connect error`, `Connection refused`, `error sending
      request`) par « Serveur hermes injoignable (Tailscale ou veille) » ; la
      carte Serveur masque l'alerte hors ligne (chip « Inconnu » + Réveiller
      suffisent).

## 1. Dataset servi et couverture

- [x] **Sélection persistée côté serveur** : `PUT /serving/{id}` sélectionne et
      démarre ORS ; `GET /serving` fournit `state`, `stateLabel`,
      `datasetState`, `since` et `reason` ; `DELETE /serving` arrête ORS et
      efface la sélection. États : `none`, `waiting`, `starting`, `active`,
      `stopped`, `failed`.
- [x] **La sélection se fait dans la liste des jeux de données** (icône de
      ligne + confirmation) ; la carte Serveur ne gère plus que l'état du
      service et les boutons « Arrêter le service »/« Démarrer le service ».
      `PUT /serving/{id}` reste la sélection.
- [x] **Datasets sélectionnables** : un dataset en construction donne
      `waiting` (sélection possible, service en attente), un dataset `failed`
      ou `unavailable` est verrouillé dans la liste (icône cadenas).
- [x] **Nouveaux états de dataset** : `state`, `stateLabel`, `phase`,
      `artifacts` (`pbf`, `ors`, `communes`, `tunnels`, `mm`) et `runId` sont
      affichés, avec repli sur `status` pour les manifests antérieurs à v1.9.
- [x] Le paramètre client `Hermes.Dataset.actif` est supprimé : `/serving` est
      la seule source de vérité, y compris pour `ensureReady` et la couverture
      de l'accueil.
- [ ] **Couverture et états** : la carte de l'accueil dessine en bleu la
      couverture du dataset servi quel que soit son état (`ready`, `building`,
      `unavailable`, `degraded*`, `stale`). Décider s'il faut la limiter aux
      datasets exploitables.
- [ ] **Sélection conservée après suppression** : le contrat conserve l'id
      sélectionné après `DELETE /datasets/{id}` pour permettre un flux
      `DELETE` puis `POST`. L'IHM affiche l'état servi correspondant ; décider
      si elle doit proposer une re-sélection automatique.
- [ ] **Dataset `stale`** : aucun style spécifique n'est appliqué à sa
      couverture ni à ses couches. Décider si un signal visuel est nécessaire.
- [ ] **Préparation aux d'un dataset déjà `ready`** : l'appel automatique ne
      se produit que sur la transition vers `ready` observée par le polling de
      build. Un dataset déjà `ready` avant l'ouverture de l'application
      (ex. `EB-Menton`) doit être préparé avec le bouton manuel.

## 2. Préparation des artefacts aux (`/aux/prepare`)

- [x] Commande Tauri `hermes_prepare_aux` :
      `POST /datasets/{id}/aux/prepare` avec `{ force }` **uniquement**.
      Le contrat v1.10 (`AuxPrepareRequest`, `additionalProperties: false`) a
      supprimé `skipMm` ; l'envoyer provoquait un 422.
- [x] Appel automatique dès que le **graphe ORS est construit**
      (`artifacts.ors.state === 'ready'` ou `status/state === 'ready'`), et non
      plus seulement sur `status === 'ready'`. Avec v1.10, `status` reste
      `preparing` et `phase` vaut `communes` pendant la chaîne aux.
- [x] **Relance aux par jeu de données** : le bouton n'est plus dans la carte
      Serveur ; il apparaît sur chaque ligne dont au moins un aux est dégradé.
      Il est activé dès que le graphe ORS est prêt (même si `status=preparing`).
      `force=true` (régénération complète).
- [x] Un `409` est absorbé : une préparation (ou un build) est déjà en cours ;
      le store se contente de suivre `GET /datasets/{id}/aux`.
- [x] Suivi : polling toutes les 3 s, budget 45 min, keep-alive actif pendant
      la préparation.
- [x] Succès = `preparing=false`, `lastError=null` et **les trois artefacts
      `state=ready`** (dont `mm` : un dossier non vide ne suffit pas). L'IHM
      relit ensuite `GET /datasets/{id}` pour mettre à jour le tag
      (`Construction` → `Prêt`/`degraded*`) sans quitter la vue.
- [x] Les erreurs `lastError` du serveur sont remontées par les toasts du
      store ; la ligne affiche les chips aux dégradés et le bouton de relance.
- [x] **`GET /datasets` en 500** (23/11/2026) : corrigé côté serveur
      (`enrich_manifest(d.name, …)`). La liste répond de nouveau 200.
- [x] **Portée du bouton** : une relance par ligne existe désormais dans
      l'onglet Jeu de données, uniquement si un aux est dégradé.
- [ ] **Préparations concurrentes** : un seul suivi à la fois. Un appel
      automatique pour un autre dataset pendant une préparation est ignoré
      silencieusement. À revoir si le serveur autorise ou ordonnance plusieurs
      préparations.
- [ ] **Cas d'un `409` pour un autre motif** : le code traite tout `409` comme
      « préparation déjà en cours ». Si le serveur renvoie `409` pour un build
      en cours ou une autre préparation, le suivi peut conclure à des
      artefacts incomplets sans expliquer la vraie cause.
- [x] **Contrats locaux** : `features/Serveur OSM/hermes-api-contract.md`
      (v1.10, suivi mais modifié localement) et `sequencement.md` (non suivi)
      sont la référence. Ils couvrent `/aux/prepare` sans jeton, la règle
      `mm.state == ready` et la sélection server-side `/serving`. Ne pas
      committer ces modifications sans décision.

### Correctifs serveur requis (constatés le 23/11/2026)

1. ~~`GET /datasets` : `d.id` sur un `PosixPath`~~ → `d.name` (appliqué).
2. ~~`hermes-prepare-aux.sh` : `state_write` échoue sur un marqueur `pending`
   sans `startedAt`~~ → extraction tolérante + trap protégé (appliqué).
3. **`PUT /serving/{id}` en 500 : `/srv/hermes` en lecture seule dans le
   sandbox systemd.** Le journal donne :
   ```text
   OSError: [Errno 30] Read-only file system: '/srv/hermes/serving.json.tmp'
   ```
   `hermes-api.service` a `ProtectSystem=strict` et
   `ReadWritePaths=/srv/hermes/datasets /srv/hermes/logs …` : le fichier
   `serving.json` à la racine de `/srv/hermes` n'est pas inscriptible. Ajouter
   un drop-in :
   ```ini
   # /etc/systemd/system/hermes-api.service.d/rwpaths-serving.conf
   [Service]
   ReadWritePaths=/srv/hermes
   ```
   La directive systemd seule ne suffit pas : l'ACL de `/srv/hermes` est
   `group:hermes-datasets:--x`. Accorder l'écriture au compte du service :
   ```bash
   sudo setfacl -m u:hermes-api:rwx /srv/hermes
   ```
   puis `sudo systemctl daemon-reload && sudo systemctl restart hermes-api`.

## 3. Carte Hermes

- [x] **Style du polygone édité** centralisé dans
      `HERMES_DRAW_POLYGON_STYLE` (`src/components/Hermes/hermesMapLayers.ts`),
      appliqué via l'option `styles` du constructeur Mapbox Draw
      (`buildDrawStyles` dans `HermesMap.vue`).
- [x] Ne pas revenir à un `setPaintProperty` immédiat après `addControl` :
      Mapbox Draw diffère `addLayers` tant que `map.loaded()` est faux, et les
      couches n'existent pas encore à cet instant.
- [x] Formulaire polygone en **tiroir droit non modal** : la carte reste
      interactive pour ajouter, déplacer ou supprimer les sommets et utiliser
      le marqueur distance.
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
- [x] Filtre ajouté dans l'onglet Jeu de données (nom, identifiant, régions,
      secteurs).
- [ ] **Traces favorites/affichées** : couleurs et épaisseurs lues au chargement
      de la carte ; pas de mise à jour à chaud si les paramètres `Carte.*`
      changent pendant que la vue Hermes est ouverte.

## 4. Carte de l'accueil

- [x] Couverture du dataset servi : source et couches
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
- [x] **Contrats locaux** : `features/Serveur OSM/hermes-api-contract.md`
      (v1.10, suivi mais modifié localement) et `sequencement.md` (non suivi)
      intègrent `/aux/prepare` sans jeton et la sélection `/serving`. Ne pas
      committer ces modifications sans décision.
- [ ] **Évolutions serveur demandées** :
      `features/Serveur OSM/hermes-api-evolutions.md` (nom des datasets,
      `needsUpdate`/`zonesRevisions`, `stopped` après arrêt d'instance). À
      transmettre et suivre côté hermes-api ; l'IHM fonctionne avec des replis
      documentés en attendant.
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
