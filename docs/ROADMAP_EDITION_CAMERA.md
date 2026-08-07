# Roadmap — Vue d'édition caméra

> État au 2026-08-07 (branche `EditonCamera2`)

## ✅ Déjà réalisé

| Feature | Détail |
|---------|--------|
| **Génération de keyframes** (`keyframeGenerator.ts`) | Polyligne indexée par distance, échantillonnage régulier (1 km), types JSON figés, helpers purs d'interpolation |
| **Altitude dans les keyframes** | Propagation depuis les `tracePoints` backend → polyligne → keyframes → interpolation linéaire → HUD + tooltip |
| **Polyligne réelle** | Le curseur avance le long de la trace (et non entre les keyframes), suit les virages |
| **Carte satellite + terrain** (`EditionMap.vue`) | Style `standard-satellite`, DEM, pitch 60°, CircleLayer WebGL (synchronisé terrain) |
| **Boucle de lecture rAF** | `requestAnimationFrame`, delta réel, `tick()`, pause auto en fin de course |
| **Contrôles de lecture** — Composant A (`PlaybackControls.vue`) | Play/Pause, vitesse (0.5×/1×/2×/4×), distance parcourue |
| **Graphe SVG d'avancement** — §4.6 (`ProgressGraph.vue`) | Timeline proportionnelle (3px/100m), 3 zones (RdV / avancement / graduation), curseur rouge, repères 10km, clic→seek, tooltip distance+altitude, auto-scroll fluide (scroll DOM + détection par valeur) |
| **HUD télémétrie** — Composant C (`TelemetryHud.vue`) | Cam (Zoom/Pitch/Bearing/Lng/Lat) + Traceur (altitude interpolée) + Relation (distance/cap) |
| **Cadre ViewPort 16:9** (`ViewportFrame.vue`) | Overlay CSS, rectangle maximal, masque sombre |
| **Persistance des keyframes** (`keyframesStore`) | Sauvegarde/chargement/suppression JSON sur disque (écriture atomique) |
| **CircleLayer vs Marker DOM** | Curseur WebGL synchronisé avec le terrain 3D |
| **Déclencheur** | Bouton Éditer dans `Circuit.vue` → sélection trace → navigation `/edition-camera` |

## 🔲 Reste à faire

### 1. Composant B — Édition fine des keyframes

**Aucune spec détaillée dans le repo** (la spec externe §4.4 n'a pas été enregistrée).

D'après les références dans le code et la doc :
- Ajouter/supprimer/déplacer des keyframes sur la timeline
- Édition des propriétés d'un keyframe individuel (position caméra, zoom, pitch, bearing, temps/distance)
- Interaction avec le `ProgressGraph` (drag de ticks, insertion par clic long, suppression par menu contextuel ?)
- Panneau de propriétés latéral ou popup pour éditer un keyframe sélectionné
- Synchronisation avec le store edition et la persistance JSON

**Questions à clarifier avant de commencer :**
- Quel UX exact ? (drag sur la timeline, panneau dédié, les deux ?)
- Quelle granularité d'édition ? (position only, ou cam state complet ?)
- Annulation/restauration possible ? (undo/redo)

### 2. Algorithme de frustum — Phase 1 (spec §3.3)

**Aucune spec détaillée dans le repo non plus.** L'algorithme MVP actuel est un suivi fidèle de la trace. L'algorithme de frustum remplacerait :
- Anticipation des virages : la caméra « regarde vers l'avant » et ajuste le zoom/pitch pour prendre en compte les virages à venir
- Ajustement dynamique zoom/pitch : zoom out dans les virages serrés, zoom in dans les lignes droites
- Gestion de la vitesse : ralentir dans les sections complexes, accélérer dans les sections simples

Le code est déjà conçu pour être **remplaçable sans impacter le reste** :
- `keyframeGenerator.ts` est un module isolé (pas de dépendance UI)
- Les stores, la carte et le HUD consomment le `KeyframeSet` généré — ils n'ont pas à changer
- `EXTENDING.md` et `CONVENTIONS.md` documentent cette remplaçabilité

## ⚡ Améliorations possibles (non planifiées)

| Item | Description |
|------|-------------|
| **Densité adaptative des ticks** | Sur les traces > 50 km, les ticks RdV (1 km) se chevauchent → filtrage visuel ou regroupement |
| **Profil altimétrique dans le graphe** | Ajouter un mini profil d'élévation dans la zone avancement (area chart) |
| **Seek par drag** | Permettre de glisser le curseur rouge au lieu de seulement cliquer |
| **Marqueurs de bookmark** | Signets utilisateur sur la timeline (favoris de positions) |
| **Export vidéo** | Capturer le rendu en vidéo (via MediaRecorder API ou Tauri) |

---

**Dernière mise à jour** : 2026-08-07
