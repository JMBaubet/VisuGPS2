# Roadmap — Vue d'édition caméra

> État au 2026-08-14 (branche `EditonCamera2`)

## ✅ Déjà réalisé

| Feature | Détail |
|---------|--------|
| **Génération de keyframes** (`keyframeGenerator.ts`) | Deux algorithmes au même format JSON figé : `simple` (échantillonnage régulier 1 km, MVP) et `frustum` (spec §3.3, cf. plus bas). Types JSON figés, helpers purs d'interpolation |
| **Algorithme de frustum — Phase 1** (spec §3.3, `frustum.ts`) | Placement **récursif par visibilité** : la caméra vole la corde A→Z, le traceur suit la trace ; chaque point est testé par **projection frustum perspective** (FOV ~36,87°, marge 0.85) + **ligne de visée contre le relief**. Échec latéral → insertion d'un keyframe au point de **moindre courbure** ; échec par le relief → **réorientation oblique** du cap (30°/45°/60°, jamais à 90°) ; anti-surabondance par `minKeyframeGapM` ; affinage en re-validant les segments avec le cap **interpolé** réel de la lecture |
| **Occlusion par le relief** | Grille terrain **fine** (`terrain-rgb` zoom 13 ≈ 7 m/px, décodée en canvas, pas ~100 m, exagérée ×1.5) sur l'emprise de la trace — `queryTerrainElevation` seul (~60 m/px au zoom de génération) ratait les buttes côtières. LOS tracée depuis la **position au sol de la caméra** (et non du centre de la corde). Module frustum indépendant de Mapbox via `TerrainSampler` |
| **Lissage du cap entre RdV** | `interpolateCam` interpole le bearing **sur le cercle** (`lerpAngle`, chemin le plus court) — la caméra tourne en douceur d'un point de RdV au suivant, sans à-coup |
| **Sélecteur d'algorithme + Gap min** | Gérés dans le **panneau Paramètres** (drawer, bouton `mdi-cog-outline` en flip-flop) : `Edition.Generation.algorithme` (list frustum/simple) et `Edition.Generation.gapMin` (int 200–5000, pas 50) → store édition → **régénération + re-persistance automatiques** (watchers d'EditionMap). Retirés de la toolbar |
| **Paramètres de la vue Édition** (`settings.default.toml`, `edition.ts`, `EditionCamera.vue`) | Panneau data-driven réutilisant `SettingsDrawer` (vue `editionCamera`). Catégories : **Génération** (algorithme, gap min), **Caméra** (`zoomDefaut` float pas 0.1, `pitchDefaut` int, `viewportDefaut` list 16:9/4:3, `dureeFlyTo` int 100–1000 ms — animation du déplacement caméra vers le curseur, `afficherTelemetrie` bool — HUD télémétrie masqué par défaut, `afficherCapBrutaux` bool — panneau des changements de cap brutaux masqué par défaut), **Lecture** (`acceleration` float 1.5–8 — facteur de la lecture accélérée) et **Couleurs** (`trace`, `epaisseurTrace` int 1–20 px, `curseur` — `material_extended`, couleurs Material strict). Appliqués via `editionStore.applySettings()` : seeding à l'ouverture (`applySettings(true)`, viewport initial), **live** ensuite (`applySettings(false)`) — couleurs réactives sur les couches Mapbox et le curseur timeline, zoom/pitch consommés à la génération et au reset des sliders du CameraEditor, **viewport appliqué en direct quand c'est ce paramètre qui change** (le flip-flop de session n'est pas écrasé par la sauvegarde d'un autre paramètre) |
| **Keyframes par ratio d'écran (16:9 / 4:3)** | **Un fichier par trace et par ratio** : `keyframes/{trace_id}_169.json` et `{trace_id}_43.json` (nommage explicite). Viewports de référence 1920×1080 / 1440×1080 (même hauteur → seul le champ horizontal FOV diffère, adapté aux vidéoprojecteurs 4:3 d'ancienne génération). **Bouton ViewPort** dans la toolbar (flip-flop 16:9 / 4:3) : le ratio sélectionné pilote le cadre affiché **et** le fichier keyframes exploité ; les deux fichiers sont chargés/générés dès l'ouverture (le ratio actif d'abord, l'autre en arrière-plan). Sauvegarde des éditions dans le bon fichier via le champ `viewport` du jeu |
| **Altitude dans les keyframes** | Propagation depuis les `tracePoints` backend → polyligne → keyframes → interpolation linéaire → HUD + tooltip |
| **Polyligne réelle** | Le curseur avance le long de la trace (et non entre les keyframes), suit les virages |
| **Carte satellite + terrain** (`EditionMap.vue`) | Style `standard-satellite`, DEM, pitch 60°, CircleLayer WebGL (synchronisé terrain) |
| **Ouverture directe au km 0 + écran vierge** (`EditionMap.vue`, `EditionCamera.vue`) | Au lancement, **plus de vue globale** (zoom bas initial et `fitBounds` d'emprise supprimés) : la caméra est positionnée **directement au départ (km 0)**. La carte est montée **masquée** (`editionViewReady` false) et la vue entière (carte + overlays + bandeau de lecture) n'est **révélée qu'au premier `idle`** de Mapbox (toutes les tuiles visibles chargées, repli temporisé 10 s) — `revealView` **force le recalcul de l'altitude caméra au km 0** (`repositionCameraForTerrain` : double saut sub-pixel car le setter `center` de Mapbox v3 ne recalcule que si le centre change — le relief chargé relève le terrain ; sans recalcul le curseur serait décalé vers le bas) — **`v-progress-circular` centré** (`size` 70, `width` 7, primary, indéterminé) pendant le chargement |
| **Boucle de lecture rAF** | `requestAnimationFrame`, delta réel, `tick()`, pause auto en fin de course |
| **Contrôles de lecture** — Composant A (`PlaybackControls.vue`) | Play/Pause, **lecture accélérée** (bouton double chevron → ×`acceleration`, paramètre `Edition.Playback.acceleration` 1.5–8, défaut 2), km0 / dernier point, distance parcourue |
| **Graphe SVG d'avancement** — §4.6 (`ProgressGraph.vue`) | Timeline proportionnelle (3px/100m), 3 zones (RdV / avancement / graduation), curseur rouge, repères 10km, clic→seek, tooltip distance+altitude, auto-scroll fluide (scroll DOM + détection par valeur). **Ticks RdV bicolores** : zone supérieure zoom (rouge si ≠ défaut), zone inférieure pitch (orange si ≠ défaut), vert sinon — identifie les RdV à tilt/zoom inhabituel |
| **Composant B — Édition fine des keyframes** (`CameraEditor.vue`) | Édition d'un keyframe sélectionné : switch **Cible**, sliders **Pitch / Zoom** (valeurs défaut issues des paramètres `pitchDefaut` / `zoomDefaut`, double-clic ou clic sur la valeur pour réinitialiser), **compas** (réglage du bearing), **Undo / Supprimer / Sauvegarder**, km 0 non supprimable. Alimenté par `currentKeyframe` + actions du store édition (`updateKeyframe`, `addKeyframe`, `removeKeyframe`, `saveKeyframes`) |
| **Changements de cap brutaux** (`headingChanges.ts`, `ProgressGraph.vue`, `HeadingChangesPanel.vue`) | Analyse des virages entre keyframes consécutifs (module pur) : Δcap signé, sens (horaire / anti-horaire), taux `|Δcap|/Δdist` en °/km, seuil réglable (10–1000, défaut 45). **Timeline** : **tous** les changements sont représentés **entre deux ticks RdV** — couleur = sens (**teal** horaire / **deep-purple** anti-horaire), **épaisseur** = intensité (trait 2 px sous le seuil, +2 px par bande de 30 °/km : 45, 75, 105, 135…), tooltip SVG, bandes **sous** le curseur. **Tableau à la demande** (bouton) : lignes cliquables (seek + sélection), Δ dist en km, **code couleur par colonne** — Départ/Arrivée en **jaune** sur le segment du curseur d'avance (auto-scroll centré), **Δ dist/Δ cap** colorés par le **sens** (teal/deep-purple, pas de colonne Sens), **Taux °/km** coloré **jaune → rouge** par bande (45, 75, 105, 135…). Filtre d'affichage sans régénération des keyframes |
| **Verrous de segments — mode validation** (`edition.ts`, `EditionToolbar.vue`, `EditionMap.vue`, `ProgressGraph.vue`, `CameraEditor.vue`, `ViewportFrame.vue`, `DistanceHud.vue`) | Bouton `mdi-camera-lock` : en lecture, un **clic carte** (ou la **touche Entrée**) signale un « problème » (segment déverrouillé), les segments **sans clic** sont **verrouillés** automatiquement au passage. Verrous portés par un **flag `locked` sur chaque keyframe** (verrou du segment qui en part, par ratio) ; **marques bleues persistées** dans `Keyframe.marks` (distances du curseur à chaque pose, plusieurs par segment, **affichées à la réouverture**). **Protection par parcours** : une marque protège son segment uniquement pendant le parcours où elle est posée ; un re-parcours sans Entrée → segment **verrouillé** + marques **supprimées** (aucune purge globale au retour km0) ; trait **rouge** en haut de la timeline pour les segments **non verrouillés** ; **double-clic** sur un segment → **bascule** du verrou (Verrouiller / Déverrouiller) ; keyframes bordant un verrou **non modifiables** (gardes du store + widgets désactivés dans le CameraEditor) ; **repère visuel bleu** (`#2196F3`) : cadre du ViewPort + fond de l'indicateur de distance |
| **HUD télémétrie** — Composant C (`TelemetryHud.vue`) | Cam (Zoom/Pitch/Bearing/Lng/Lat) + Traceur (altitude interpolée) + Relation (distance/cap). **Masqué par défaut** — visibilité pilotée par le paramètre `Edition.Camera.afficherTelemetrie` (bool) ; **bouton Fermer** (fermeture valable pour la session) ; **lignes Zoom/Pitch colorées** comme les ticks RdV — vert au défaut, orange (pitch ≠ défaut) / rouge (zoom ≠ défaut) |
| **Cadre ViewPort 16:9** (`ViewportFrame.vue`) | Overlay CSS, rectangle maximal, masque sombre |
| **Persistance des keyframes** (`keyframesStore`) | Sauvegarde/chargement/suppression JSON sur disque (écriture atomique) |
| **CircleLayer vs Marker DOM** | Curseur WebGL synchronisé avec le terrain 3D |
| **Déclencheur** | Bouton Éditer dans `Circuit.vue` → sélection trace → navigation `/edition-camera` |

## 🔲 Reste à faire

### 1. Frustum — Phase 2 (relief complet & paramètres dynamiques)

L'algorithme de frustum Phase 1 est fonctionnel (visibilité + occlusion relief + réorientation oblique). Améliorations envisagées ensuite :
- **Zoom / pitch dynamiques** : zoom out dans les virages serrés, zoom in dans les lignes droites (zoom/pitch fixes 16/60° en Phase 1)
- **Gestion de la vitesse** : ralentir dans les sections complexes, accélérer dans les sections simples
- **Anticipation des virages** : la caméra « regarde vers l'avant » et ajuste le cadrage aux virages à venir

### 2. Composant B — Éditions complémentaires

Le CameraEditor couvre l'édition d'un keyframe (pitch/zoom/bearing). Reste à étudier :
- **Manipulation sur la timeline** : drag des ticks du ProgressGraph, insertion par clic long, suppression par menu contextuel
- **Panneau de propriétés** latéral ou popup pour un keyframe sélectionné
- **Annulation / restauration** multi-étapes (undo/redo au-delà de l'annulation simple actuelle)

## ⚡ Améliorations possibles (non planifiées)

| Item | Description |
|------|-------------|
| **Densité adaptative des ticks** | Sur les traces > 50 km, les ticks RdV (1 km) se chevauchent → filtrage visuel ou regroupement |
| **Profil altimétrique dans le graphe** | Ajouter un mini profil d'élévation dans la zone avancement (area chart) |
| **Seek par drag** | Permettre de glisser le curseur rouge au lieu de seulement cliquer |
| **Marqueurs de bookmark** | Signets utilisateur sur la timeline (favoris de positions) |
| **Export vidéo** | Capturer le rendu en vidéo (via MediaRecorder API ou Tauri) |

---

**Dernière mise à jour** : 2026-08-16
