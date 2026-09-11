# Livrable 6 — Modifications de `settings.default.toml`

**Fichier cible** : `src-tauri/settings.default.toml`

**Rôle** : déclarer les paramètres du module Audit GPX dans le système
existant de VisuGPS2 (TOML + `_meta`), et retirer les paramètres du
module Nettoyage désormais obsolète.

**Décisions appliquées** :
- Décision 5 : namespace `Audit.*` + clés ORS en `secret` chiffré
- Décision 12 : ajout du type `"string"` au système de paramètres
- Décision 4 : suppression du namespace `Nettoyage.*`

---

## 1. Vue d'ensemble

| Opération | Éléments |
|---|---|
| **Ajoutés** | 10 paramètres (`Audit.*`) + 1 vue `_meta` + 5 groupes `_meta` |
| **Supprimés** | ~7 paramètres (`Nettoyage.*`) + 1 vue + 2 groupes |
| **Nouveau type** | `"string"` (à implémenter dans `settings.rs`) |
| **Mapbox** | Réutilise `Systeme.Key.mapBox` existant — **aucun ajout** |

---

## 2. Namespace `Audit.*` — Contenu complet

À insérer dans `settings.default.toml`, à la fin du fichier (après le
dernier paramètre existant, avant la table `[_meta]`).

```toml
# ═══════════════════════════════════════════════════════════════════════
# AUDIT GPX — Paramètres d'analyse et de routage
# ═══════════════════════════════════════════════════════════════════════
#
# Ce namespace pilote le module Audit GPX (remplace l'ancien module
# Nettoyage). Il est consommé par :
#   - le store Pinia `src/stores/audit.ts`
#   - les commandes Tauri `gpx_audit::commands::*`
#   - le SettingsDrawer de la vue /audit
#
# Le paramètre `Systeme.Key.mapBox` (existant) est réutilisé pour la
# carte ; aucun paramètre Mapbox spécifique n'est ajouté ici.
# ═══════════════════════════════════════════════════════════════════════

# ─── Consolidation ────────────────────────────────────────────────────

[Audit.Consolidation.seuil]
description = "Seuil de consolidation des points (m)"
documentation = """
Points consécutifs à moins de ce seuil sont fusionnés avant analyse.
0 désactive la consolidation (trace brute).

Un seuil plus élevé produit une trace plus lissée, éliminant les
micro-segments. Un seuil trop élevé peut masquer certaines anomalies
fines (notamment les paires miroir à faible distance).
"""
type = "float"
default = 0.5
min = 0.0
max = 5.0
step = 0.1
unit = "m"

# ─── Détecteur AR (aller-retours) ─────────────────────────────────────

[Audit.AR.toleranceDeg]
description = "Tolérance angulaire pour un retournement AR (°)"
documentation = """
Écart maximal au demi-tour parfait pour valider un retournement.
20° accepte les retournements approximatifs (bruit GPS courant),
0° n'accepte que les demi-tours quasi exacts (déconseillé en production,
sensible aux arrondis flottants).
"""
type = "float"
default = 20.0
min = 0.0
max = 90.0
step = 1.0
unit = "°"
icon = "mdi-angle-acute"

[Audit.AR.seuilPaireM]
description = "Seuil de distance pour une paire miroir (m)"
documentation = """
Distance maximale entre deux points symétriques autour d'un sommet
pour former une paire miroir.

Un seuil plus élevé détecte des paires plus larges (emprises AR plus
étendues). Un seuil trop faible ne retient que les superpositions
exactes. La valeur de 50 m couvre la majorité des cas réels.
"""
type = "float"
default = 50.0
min = 0.0
max = 500.0
step = 5.0
unit = "m"
icon = "mdi-arrow-expand-horizontal"

[Audit.AR.maxPaires]
description = "Nombre max de paires par sommet AR"
documentation = """
Budget maximal de paires symétriques testées autour d'un sommet.

Une valeur plus élevée étend l'emprise du finding (plus de points
inclus dans la zone d'anomalie). Une valeur plus faible réduit
l'emprise au voisinage immédiat du sommet.
"""
type = "int"
default = 5
min = 1
max = 10
step = 1
icon = "mdi-counter"

[Audit.AR.branchesMaxM]
description = "Longueur max des branches du sommet AR (m)"
documentation = """
Longueur maximale des deux branches (amont/aval) encadrant un sommet
AR. Sert de garde anti-« vrai virage en épingle » : un virage routier
normal a des branches > 200 m et ne doit pas être confondu avec un
retournement de traceur.

Une valeur plus élevée tolère des branches longues (risque de faux
positifs sur virages serrés). Une valeur plus faible exclut les
branches longues (risque de faux négatifs sur traces de traceur).
"""
type = "float"
default = 200.0
min = 10.0
max = 1000.0
step = 10.0
unit = "m"
icon = "mdi-source-branch"

# ─── Détecteur RP (boucles giratoires) ────────────────────────────────

[Audit.RP.seuilFermetureM]
description = "Seuil de refermeture des boucles RP (m)"
documentation = """
Distance maximale entre deux points non consécutifs pour créer une
paire candidate. Aussi utilisé par l'anti-aiguille (test miroir) et
pour la zone de superposition des ancres.

Attention : un seuil trop grand réhabilite les aiguilles larges (le
test miroir de l'anti-aiguille est calé sur ce seuil). Un seuil trop
petit empêche la détection des mini-giratoires (rayon < seuil).
"""
type = "float"
default = 15.0
min = 5.0
max = 60.0
step = 1.0
unit = "m"
icon = "mdi-vector-circle-variant"

[Audit.RP.angleMinDeg]
description = "Angle cumulé minimal pour valider une boucle RP (°)"
documentation = """
Angle cumulé minimal (après quantification) pour publier une fenêtre.
- 270° = « trois-quarts de tour et plus » (détecte les 3/4 de tour)
- 360° = tours complets et multiples uniquement (filtre strict)

La quantification aligne un tour complet sur exactement 360° quand la
mesure brute est dans la plage 320–400°, ce qui correspond à la
physique de la refermeture à l'angle vif.
"""
type = "int"
default = 270
min = 270
max = 360
step = 5
unit = "°"
icon = "mdi-rotate-360"

# ─── OpenRouteService (secrets chiffrés) ──────────────────────────────

[Audit.OpenRouteService.clePrimaire]
description = "Clé API OpenRouteService n°1 (principale)"
documentation = """
Clé utilisée pour les demandes de routage (profil voiture et vélo de
route, deux requêtes parallèles). En cas de quota épuisé (HTTP 403 ou
429), bascule automatique sur la clé secondaire.

Format : chaîne de 40+ caractères fournie par OpenRouteService.
Chiffrée en AES-256-GCM avant écriture disque (clé maîtresse gérée
par le keyring OS en production).
"""
type = "secret"
default = ""

[Audit.OpenRouteService.cleSecondaire]
description = "Clé API OpenRouteService n°2 (secours)"
documentation = """
Clé utilisée en repli si la clé principale est refusée (403/429).
Si les deux clés sont refusées, le routage est impossible jusqu'à
la prochaine remise à zéro du quota (minuit UTC pour le plan gratuit).

Chiffrée en AES-256-GCM avant écriture disque.
"""
type = "secret"
default = ""

# ─── Application (nom d'export) ───────────────────────────────────────

[Audit.Application.nom]
description = "Nom d'application pour l'export GPX"
documentation = """
Nom inséré dans les métadonnées du GPX exporté (bloc audit) :
- élément <tool> du bloc <extensions><audit>
- élément <desc> du <metadata>
- attribut creator de la balise <gpx> (uniquement en cas de source
  sans attribut, cas pathologique)

Si vide, la valeur par défaut « VérificationGPX » est utilisée.
"""
type = "string"
default = "VérificationGPX"
icon = "mdi-application"
```

---

## 3. Table `[_meta]` — Ajout de la vue `audit`

À insérer dans la section `[_meta]` existante, à côté des vues
existantes (`accueil`, `editionCamera`, etc.).

```toml
# ─── Vue : Audit GPX ──────────────────────────────────────────────────

[_meta.views.audit]
label = "Audit GPX"
icon = "mdi-map-marker-path"
groups = [
  "Audit.Consolidation",
  "Audit.AR",
  "Audit.RP",
  "Audit.OpenRouteService",
  "Audit.Application",
]

# ─── Libellés et icônes des catégories ────────────────────────────────

[_meta.groups."Audit.Consolidation"]
label = "Consolidation"
icon = "mdi-vector-polyline"

[_meta.groups."Audit.AR"]
label = "Détecteur aller-retours"
icon = "mdi-swap-horizontal"

[_meta.groups."Audit.RP"]
label = "Détecteur boucles giratoires"
icon = "mdi-rotate-360"

[_meta.groups."Audit.OpenRouteService"]
label = "OpenRouteService"
icon = "mdi-routes"

[_meta.groups."Audit.Application"]
label = "Application"
icon = "mdi-application"
```

**Note** : le libellé et l'icône du groupe reflètent ce que
`useSettingsTree.ts` utilisera pour construire l'arbre de catégories
du drawer sur la vue `/audit`. Aucun composant Vue à modifier.

---

## 4. Retrait du namespace `Nettoyage.*`

### 4.1 Paramètres à supprimer

Dans `settings.default.toml`, supprimer **toutes** les entrées suivantes
(et leurs commentaires associés) :

```toml
# À SUPPRIMER INTÉGRALEMENT
[Nettoyage.Cap.toleranceDeg]
[Nettoyage.RondPoints.angleMinDeg]
[Nettoyage.RondPoints.pointsMin]
[Nettoyage.RondPoints.pointsMax]
[Nettoyage.RondPoints.angleSeuilDeg]
[Nettoyage.RondPoints.margePoints]
```

### 4.2 Références `[_meta]` à supprimer

Dans la section `[_meta]`, supprimer :

```toml
# À SUPPRIMER
[_meta.views.nettoyage]
label = "Nettoyage"
icon = "mdi-broom"
groups = ["Nettoyage.Cap", "Nettoyage.RondPoints"]

[_meta.groups."Nettoyage.Cap"]
label = "Nettoyage — Détection"
icon = "mdi-angle-acute"

[_meta.groups."Nettoyage.RondPoints"]
label = "Nettoyage — Ronds-points"
icon = "mdi-rotate-360"
```

### 4.3 Effet

Après cette modification, le SettingsDrawer ne propose plus la vue
`nettoyage`. Les valeurs éventuellement surchargées dans
`config.toml` / `config-dev.toml` deviennent orphelines — elles sont
ignorées silencieusement au chargement (aucune erreur, aucune action).

**Recommandation** : documenter dans `docs/DATA_STORAGE.md` que ces
surcharges résiduelles peuvent être supprimées manuellement si
souhaité, mais qu'elles sont sans effet.

---

## 5. Ajout du type `"string"` au système de paramètres

### 5.1 Contexte

Le type `"string"` n'existe pas encore dans VisuGPS2. Les types
supportés sont :
`int | float | bool | secret | list | rgba | material_primary |
material_extended | monitor`.

`Audit.Application.nom` a besoin d'un type texte simple, non chiffré.

### 5.2 Modifications côté Rust (`settings.rs`)

Trois points à modifier dans `settings.rs` :

**a) Parse du TOML** — reconnaissance du type `"string"` :

```rust
// Dans la fonction qui parse les types (ex. parse_setting_type)
match type_str.as_str() {
    "int" => SettingType::Int,
    "float" => SettingType::Float,
    "bool" => SettingType::Bool,
    "string" => SettingType::String,   // NOUVEAU
    "secret" => SettingType::Secret,
    "list" => SettingType::List,
    "rgba" => SettingType::Rgba,
    "material_primary" => SettingType::MaterialPrimary,
    "material_extended" => SettingType::MaterialExtended,
    "monitor" => SettingType::Monitor,
    other => return Err(format!("Type inconnu : {}", other)),
}
```

**b) Enum `SettingType`** — ajout de la variante :

```rust
pub enum SettingType {
    Int,
    Float,
    Bool,
    String,   // NOUVEAU
    Secret,
    List,
    Rgba,
    MaterialPrimary,
    MaterialExtended,
    Monitor,
}
```

**c) Validation et sérialisation** :

```rust
// Validation (pas de contrainte min/max sur string, juste type check)
SettingType::String => {
    if !value.is_string() {
        return Err(format!(
            "Paramètre {} : valeur attendue de type string, reçu {}",
            path, value_type_name(&value)
        ));
    }
    // Pas de chiffrement (contrairement à Secret)
    // Pas de contrainte min/max
}
```

### 5.3 Composant Vue `InputString.vue`

**Fichier** : `src/components/parameters/InputString.vue`

```vue
<template>
  <v-text-field
    :model-value="modelValue"
    :label="label"
    :hint="hint"
    :persistent-hint="!!hint"
    variant="outlined"
    density="comfortable"
    @update:model-value="onInput"
  />
</template>

<script setup lang="ts">
const props = defineProps<{
  modelValue: string
  label: string
  hint?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

function onInput(value: string) {
  emit('update:modelValue', value)
}
</script>
```

### 5.4 Intégration dans `ParameterCard.vue`

Ajouter la branche `"string"` dans la sélection adaptative :

```vue
<template>
  <div>
    <InputBool
      v-if="setting.type === 'bool'"
      v-model="localValue"
      :label="setting.description"
      :hint="setting.documentation"
    />
    <InputInt
      v-else-if="setting.type === 'int'"
      ...
    />
    <InputString
      v-else-if="setting.type === 'string'"
      v-model="localValue"
      :label="setting.description"
      :hint="setting.documentation"
    />
    <InputSecret
      v-else-if="setting.type === 'secret'"
      ...
    />
    <!-- autres types existants -->
  </div>
</template>
```

---

## 6. Vérifications post-modification

### 6.1 Cohérence des fichiers

Après modification, s'assurer que :

- `settings.default.toml` est un TOML valide
  (`cargo check` + démarrage de l'app suffisent)
- Chaque groupe déclaré dans `[_meta.views.audit].groups` a son entrée
  correspondante dans `[_meta.groups."..."]`
- Chaque paramètre est référencé par un groupe (`Audit.Consolidation`,
  `Audit.AR`, `Audit.RP`, `Audit.OpenRouteService`, `Audit.Application`)
- Aucune référence à `Nettoyage.*` ne subsiste

### 6.2 Commandes de vérification

```bash
# Vérifier qu'aucun paramètre Nettoyage ne subsiste
grep -i "nettoyage" src-tauri/settings.default.toml

# Vérifier que tous les paramètres Audit sont bien dans _meta
grep -E "^\[Audit\." src-tauri/settings.default.toml | sort
# Doit afficher 10 lignes (1 seuil + 4 AR + 2 RP + 2 ORS + 1 app)

# Vérifier les groupes _meta
grep -E "^\[_meta\.groups\.\"Audit" src-tauri/settings.default.toml
# Doit afficher 5 lignes

# Vérifier la vue _meta
grep -A3 "^\[_meta\.views\.audit\]" src-tauri/settings.default.toml
```

### 6.3 Test visuel

Lancer l'application, ouvrir une trace, cliquer sur « Éditer » pour
arriver sur `/audit`, ouvrir le drawer Paramètres :

- Les 5 catégories doivent apparaître :
  - **Consolidation** (1 paramètre)
  - **Détecteur aller-retours** (4 paramètres)
  - **Détecteur boucles giratoires** (2 paramètres)
  - **OpenRouteService** (2 clés masquées)
  - **Application** (1 champ texte)

- La vue `Nettoyage` **ne doit plus apparaître** dans le sélecteur de
  vue du drawer.

---

## 7. Ordre d'exécution recommandé

| Étape | Action | Vérification |
|---|---|---|
| 1 | Ajouter la variante `String` à `SettingType` dans `settings.rs` | `cargo check` |
| 2 | Ajouter la branche `"string"` dans le parseur TOML | `cargo check` |
| 3 | Ajouter la validation (type check) | `cargo check` |
| 4 | Créer `InputString.vue` | `npx vue-tsc --noEmit` |
| 5 | Ajouter la branche dans `ParameterCard.vue` | `npx vue-tsc --noEmit` |
| 6 | Retirer les paramètres `[Nettoyage.*]` du TOML | `cargo check` |
| 7 | Retirer les références `[_meta.views.nettoyage]` et `[_meta.groups."Nettoyage.*"]` | `cargo check` |
| 8 | Ajouter les paramètres `[Audit.*]` | `cargo check` |
| 9 | Ajouter `[_meta.views.audit]` et `[_meta.groups."Audit.*"]` | `cargo check` |
| 10 | Lancer l'app, ouvrir le drawer sur `/audit` | Test visuel |

**Point d'attention** : les étapes 6-7 (suppression `Nettoyage`) et 8-9
(ajout `Audit`) peuvent être faites dans le même commit. Séparer les
commits facilite le rollback en cas de problème.

---

## 8. Points de vigilance

| # | Piège | Mitigation |
|---|---|---|
| 1 | Le type `"string"` n'existe pas → erreur au démarrage de l'app | Ajouter le type AVANT d'ajouter le paramètre |
| 2 | Paramètre `[Audit.Application.nom]` non listé dans `[_meta.views.audit].groups` | Vérifier la cohérence `_meta` ↔ paramètres |
| 3 | `[_meta.groups."Audit.AR"]` et `[_meta.groups."Audit.RP"]` mal nommés (point vs tiret) | Les noms de groupes doivent matcher exactement le préfixe du paramètre |
| 4 | Surcout de l'AES sur la clé ORS vide (`""`) | Le chiffrement d'une chaîne vide est un cas particulier — soit autoriser, soit ne pas chiffrer si vide |
| 5 | Le drawer affiche `[Audit.OpenRouteService.clePrimaire]` sous forme « *** » sans distinction | Les deux clés ont des descriptions distinctes — pas de confusion possible |
| 6 | `Audit.Application.nom` : si l'utilisateur efface le champ, la valeur par défaut doit s'appliquer | `settings.rs` retourne `""` ; c'est `export.rs` qui applique `"VérificationGPX"` si vide |
| 7 | Le paramètre `Audit.AR.toleranceDeg` avec `min = 0.0` : valeur 0 possible (déconseillée) | Documenter dans la `documentation` (fait ci-dessus) |
| 8 | Les surcharges `Nettoyage.*` dans `config.toml` restent orphelines | Documenter dans `DATA_STORAGE.md` — sans effet, peuvent être supprimées manuellement |

---

## 9. Critères de validation

- [ ] `settings.default.toml` est un TOML valide (démarrage de l'app OK)
- [ ] Le type `"string"` est reconnu par `settings.rs`
- [ ] Les 10 paramètres `Audit.*` sont présents
- [ ] Les 5 groupes `_meta` sont présents
- [ ] La vue `Audit GPX` apparaît dans le drawer de `/audit`
- [ ] Aucune trace de `Nettoyage.*` dans le TOML
- [ ] Le champ `Audit.Application.nom` est éditable et sauvegardable
- [ ] Les deux clés ORS sont chiffrées (vérifiable dans `config.toml` après saisie)
- [ ] `npx vue-tsc --noEmit` passe sans erreur