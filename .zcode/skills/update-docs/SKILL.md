---
name: update-docs
description: Met à jour la documentation du projet VisuGPS2 après des modifications de code. Déclencher quand l'utilisateur demande de mettre à jour les docs, la documentation, ou quand un commit/PR contient des changements qui rendent les docs obsolètes. Fonctionne aussi quand on ajoute une commande Tauri, un paramètre, un composant, un store, une route, un utilitaire, un composable, ou qu'on modifie la structure des fichiers.
---

# Skill : Mise à jour de la documentation

Mise à jour coordonnée des 10 fichiers de documentation de `docs/` suite à des modifications de code.

## Objectif

Après un changement de code, identifier quels fichiers docs sont impactés, les lire, les mettre à jour en respectant les conventions, et vérifier la cohérence transversale (compteurs, chemins, références croisées).

## Étapes

### 1. Identifier les changements

Lancer `git diff --name-only` (ou `git diff main --name-only` si sur une branche) pour obtenir la liste des fichiers modifiés.

Regarder aussi les fichiers ajoutés (`git diff --name-only --diff-filter=A`) et supprimés (`--diff-filter=D`) si pertinents.

### 2. Déterminer quels docs sont impactés

Utiliser cette table de correspondance. Plusieurs fichiers source peuvent impacter le même doc, et inversement.

| Fichier source modifié | Doc(s) à vérifier |
|---|---|
| `src-tauri/src/*.rs` (nouvelle commande) | COMMANDS.md, ARCHITECTURE.md, CONTEXT.md |
| `src-tauri/src/lib.rs` (nouvel invoke_handler) | COMMANDS.md |
| `src-tauri/src/settings.rs` | COMMANDS.md, DATA_STORAGE.md, ARCHITECTURE.md, EXTENDING.md |
| `src-tauri/src/import_gpx.rs` | COMMANDS.md, SPEC_IMPORT_GPX.md, ARCHITECTURE.md |
| `src-tauri/src/display.rs` | COMMANDS.md, ARCHITECTURE.md |
| `src-tauri/src/gestionMode.rs` | COMMANDS.md, DATA_STORAGE.md, ARCHITECTURE.md |
| `src-tauri/settings.default.toml` | DATA_STORAGE.md, EXTENDING.md |
| `src-tauri/tauri.conf.json` | ARCHITECTURE.md, DATA_STORAGE.md, CONTEXT.md |
| `src/views/*.vue` (nouvelle vue) | ARCHITECTURE.md, EXTENDING.md, CONTEXT.md |
| `src/stores/*.ts` (nouveau store) | ARCHITECTURE.md, EXTENDING.md, CONVENTIONS.md |
| `src/components/**/*.vue` (nouveau composant) | ARCHITECTURE.md, CONVENTIONS.md |
| `src/composables/*.ts` (nouveau composable) | ARCHITECTURE.md, CONVENTIONS.md |
| `src/utils/*.ts` (nouveau utilitaire) | CONVENTIONS.md, EXTENDING.md |
| `src/router/*.ts` | ARCHITECTURE.md, EXTENDING.md |
| `package.json` (nouvelle dépendance) | CONTEXT.md, ARCHITECTURE.md |
| `Cargo.toml` (nouvelle dépendance) | CONTEXT.md |

### 3. Lire chaque doc impacté

Lire intégralement chaque fichier doc identifié à l'étape 2. Comprendre la structure et le style existants avant de modifier.

### 4. Appliquer les mises à jour

Pour chaque doc impacté :

- **Ajouter** les nouvelles entrées (commandes, paramètres, composants, etc.) au bon endroit, dans le style existant
- **Modifier** les sections obsolètes pour refléter le nouveau code
- **Supprimer** les références à du code supprimé
- **Maintenir la date** en bas du fichier (`**Dernière mise à jour** : YYYY-MM-DD`) — la mettre à jour uniquement si le contenu change

### 5. Vérifier la cohérence transversale

Après les modifications, vérifier systématiquement ces points de cohérence entre docs :

1. **Nombre de commandes Tauri** : vérifier que `COMMANDS.md`, `CLAUDE-CODE-GUIDE.md` et `README.md` mentionnent le même nombre (actuellement 20). Si une commande a été ajoutée ou supprimée, mettre à jour les trois fichiers.

2. **Arborescence des fichiers** : vérifier que `ARCHITECTURE.md`, `CONTEXT.md` et `README.md` mentionnent les mêmes dossiers et fichiers clés. Si un nouveau dossier ou fichier important a été créé (ex: `src/composables/`), l'ajouter aux arborescences.

3. **Table des matières / index** : vérifier que `README.md` et `CLAUDE-CODE-GUIDE.md` listent correctement tous les fichiers docs existants et leur description.

4. **Références croisées** : vérifier que les chemins de fichiers mentionnés dans les docs existent réellement (ex: si `EXTENDING.md` cite `src/composables/useSettingsTree.ts`, vérifier que ce fichier existe).

5. **Convention doc de maintenance** : `README.md` contient une section "Quand mettre à jour" — vérifier qu'elle couvre le type de changement effectué.

### 6. Résumer les modifications

Présenter à l'utilisateur un résumé clair avant et après chaque modification :

```
docs/COMMANDS.md :
  - Ajout de la commande get_settings_meta
  - Mise à jour du compteur : 18 → 20
  - Date mise à jour

docs/ARCHITECTURE.md :
  - Ajout du dossier composables/ dans l'arborescence
  - Mise à jour de la section Settings (champ icon, _meta)
```

## Style et conventions des docs

- Langue : **français** pour tout le contenu
- Titres : `##` pour les sections principales, `###` pour les sous-sections
- Listes : utiliser `-` pour les listes à puces
- Code : blocs clôturés avec le bon langage (`rust`, `typescript`, `toml`, `bash`)
- Emphase modérée : gras pour les termes clés au premier usage uniquement
- Pas de jargon inutile : préférer "paramètre" à "setting", "commande" à "endpoint"
- Maintenir l'indentation existante du fichier

## Fichiers de documentation

| Fichier | Rôle |
|---|---|
| `docs/README.md` | Index / point d'entrée, liste les autres docs, guide de maintenance |
| `docs/CONTEXT.md` | Vue d'ensemble, stack technique, objectifs, structure des fichiers |
| `docs/ARCHITECTURE.md` | Architecture en couches, flux de données, modules détaillés |
| `docs/COMMANDS.md` | Référence exhaustive des commandes Tauri (signature, comportement, types) |
| `docs/DATA_STORAGE.md` | Schéma de stockage disque (chemins, fichiers, sécurité, cascade) |
| `docs/CONVENTIONS.md` | Conventions de code, nommage, structure, checklist pre-commit |
| `docs/EXTENDING.md` | Tutoriels pas-à-pas pour ajouter des fonctionnalités |
| `docs/SPEC_IMPORT_GPX.md` | Spécification du module d'import GPX |
| `docs/SPEC_AFFICHAGE_TRACES.md` | Spécification affichage des traces sur la carte |
| `docs/CLAUDE-CODE-GUIDE.md` | Guide d'utilisation de l'IA avec le projet |
