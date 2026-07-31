# Guide pour utiliser un assistant IA (ZCode / Claude Code) avec VisuGPS2

> Comment tirer le meilleur parti d'un assistant IA dans ce projet

## Introduction

VisuGPS2 est documenté en profondeur pour faciliter le travail avec ZCode (et Claude Code ou tout autre assistant IA). Ce guide explique comment utiliser efficacement un assistant IA dans ce contexte.

## 🚀 Démarrage rapide

### Première interaction avec l'assistant IA

Lorsque vous démarrez une session de développement, donnez ce contexte à l'IA :

```
Je travaille sur VisuGPS2 (Tauri v2 + Vue 3 + Vuetify + Pinia).
Merci de lire docs/CONTEXT.md pour comprendre le projet avant de commencer.
```

Ou plus simplement :

```
Lis docs/CONTEXT.md
```

L'IA va lire le fichier de contexte et comprendre :
- L'architecture du projet
- Les conventions utilisées
- Les patterns à suivre
- La structure des fichiers

## 📋 Fichiers de contexte disponibles

### Pour différentes tâches

| Tâche | Fichier à lire | Commande |
|-------|----------------|----------|
| Découvrir le projet | `docs/CONTEXT.md` | "Lis docs/CONTEXT.md" |
| Comprendre l'architecture | `docs/ARCHITECTURE.md` | "Lis docs/ARCHITECTURE.md" |
| Commandes backend | `docs/COMMANDS.md` | "Consulte docs/COMMANDS.md pour les commandes Tauri" |
| Stockage des données | `docs/DATA_STORAGE.md` | "Lis docs/DATA_STORAGE.md pour comprendre la persistance" |
| Écrire du code | `docs/CONVENTIONS.md` | "Respecte docs/CONVENTIONS.md" |
| Ajouter une fonctionnalité | `docs/EXTENDING.md` | "Consulte docs/EXTENDING.md pour ajouter [fonctionnalité]" |
| Module GPX | `docs/SPEC_IMPORT_GPX.md` | "Lis docs/SPEC_IMPORT_GPX.md" |
| Affichage traces/favoris sur carte | `docs/SPEC_AFFICHAGE_TRACES.md` | "Lis docs/SPEC_AFFICHAGE_TRACES.md (spécification à implémenter)" |

## 💬 Exemples de prompts efficaces

### Comprendre le projet

```
Lis docs/CONTEXT.md et explique-moi l'architecture du projet.
```

```
Quelle est la structure des stores Pinia selon docs/CONVENTIONS.md ?
```

### Ajouter des fonctionnalités

```
En respectant docs/CONVENTIONS.md, crée un nouveau store pour gérer
les produits avec les actions fetch, add et remove.
```

```
D'après docs/EXTENDING.md, aide-moi à ajouter une page de profil
utilisateur avec routing.
```

### Déboguer

```
J'ai une erreur dans mon store. Vérifie que je respecte bien
les conventions de docs/CONVENTIONS.md.
```

```
Mon composant ne s'affiche pas. Consulte docs/ARCHITECTURE.md
et vérifie la configuration de mon router.
```

### Refactoring

```
Refactorise ce composant en respectant les conventions de
docs/CONVENTIONS.md et l'ordre défini pour les script setup.
```

```
Optimise ce code selon les bonnes pratiques de docs/CONVENTIONS.md.
```

## 🎯 Bonnes pratiques

### 1. Toujours fournir le contexte au début

**Mauvais** ❌
```
Crée un store pour gérer les utilisateurs.
```

**Bon** ✅
```
En suivant docs/CONVENTIONS.md (setup stores pattern),
crée un store users avec state, getters et actions.
```

### 2. Référencer la documentation pour les patterns

**Mauvais** ❌
```
Comment je fais pour ajouter une route ?
```

**Bon** ✅
```
Selon docs/EXTENDING.md section "Ajouter une nouvelle page",
aide-moi à créer une route pour la page Settings.
```

### 3. Demander des vérifications de conformité

```
Vérifie que ce composant respecte :
- L'ordre de docs/CONVENTIONS.md pour script setup
- Les conventions de nommage
- Le typage TypeScript strict
```

### 4. Utiliser la documentation pour l'apprentissage

```
Explique-moi la différence entre computed et function
selon docs/CONVENTIONS.md.
```

## 🔄 Workflow recommandé

### Pour une nouvelle fonctionnalité

1. **Comprendre** : "Lis docs/EXTENDING.md section [fonctionnalité]"
2. **Planifier** : "Selon la doc, quelles sont les étapes ?"
3. **Implémenter** : "Crée [X] en respectant docs/CONVENTIONS.md"
4. **Vérifier** : "Vérifie la conformité avec la documentation"

### Pour déboguer

1. **Analyser** : "Lis docs/ARCHITECTURE.md pour comprendre le flux"
2. **Identifier** : "Qu'est-ce qui ne suit pas les conventions ?"
3. **Corriger** : "Corrige en respectant docs/CONVENTIONS.md"

### Pour refactorer

1. **Auditer** : "Compare ce code avec docs/CONVENTIONS.md"
2. **Lister** : "Quelles conventions ne sont pas respectées ?"
3. **Refactoriser** : "Refactorise selon les conventions"

## 📝 Templates de prompts

### Créer un composant

```
Crée un composant [NomComposant] qui [description].

Respecte :
- docs/CONVENTIONS.md pour la structure du fichier
- L'ordre : imports → props → emits → state → computed → fonctions
- Typage TypeScript strict
- Utilisation de Vuetify
```

### Créer un store

```
Crée un store [nomStore] avec :
- State : [liste des états]
- Getters : [liste des getters]
- Actions : [liste des actions]

Utilise le setup pattern de docs/CONVENTIONS.md.
```

### Ajouter une page

```
D'après docs/EXTENDING.md, crée une nouvelle page [NomPage] :
1. Créer src/views/[NomPage].vue
2. Ajouter la route dans router
3. Ajouter au menu dans App.vue

La page doit afficher [description].
```

### Déboguer un problème

```
J'ai ce problème : [description]

Consulte docs/ARCHITECTURE.md pour comprendre le flux
et identifie le problème.
```

## 🎓 Cas d'usage avancés

### Génération de documentation

```
Génère une documentation pour ce nouveau store en suivant
le format de docs/EXTENDING.md.
```

### Code review

```
Fais une review de ce fichier selon :
- docs/CONVENTIONS.md (conventions)
- docs/ARCHITECTURE.md (patterns architecturaux)

Liste les points à améliorer.
```

### Migration de code

```
Migre ce composant vers notre stack en respectant :
- docs/CONTEXT.md (technologies utilisées)
- docs/CONVENTIONS.md (conventions de code)
```

## ⚙️ Configuration Claude Code

### Workspace context

Si vous utilisez Claude Code en IDE, vous pouvez configurer le contexte du workspace :

`.claude/context.md` ou fichier `AGENTS.md` (si supporté)
```markdown
Ce projet est VisuGPS2 (Tauri v2 + Vue 3 + Vuetify + Pinia + Router).

Documentation de référence :
- docs/CONTEXT.md : Vue d'ensemble
- docs/ARCHITECTURE.md : Architecture technique
- docs/COMMANDS.md : Référence des 20 commandes Tauri
- docs/DATA_STORAGE.md : Schéma de stockage des données
- docs/CONVENTIONS.md : Conventions de code
- docs/EXTENDING.md : Guide d'extension
- docs/SPEC_IMPORT_GPX.md : Spécification du module GPX
- docs/SPEC_AFFICHAGE_TRACES.md : Spécification favoris & affichage carte (à implémenter)

Principes :
- Composition API uniquement
- Setup stores pattern pour Pinia
- TypeScript strict
- Commandes Tauri comme seule passerelle d'E/S disque
- Pas de linting (environnement minimal)
```

## 🚫 À éviter

### Ne pas faire

❌ Demander à Claude d'ajouter ESLint/Prettier (choix délibéré)
❌ Ignorer les conventions documentées
❌ Utiliser Options API au lieu de Composition API
❌ Créer des stores sans setup pattern
❌ Ne pas typer en TypeScript

### Faire à la place

✅ Suivre l'environnement minimal défini
✅ Référencer docs/CONVENTIONS.md
✅ Utiliser Composition API + script setup
✅ Utiliser setup stores pattern
✅ Typer strictement tout

## 📊 Mesurer l'efficacité

### Claude Code est efficace quand :

✅ Il référence la documentation sans qu'on le demande
✅ Le code généré suit les conventions
✅ Les patterns correspondent à docs/ARCHITECTURE.md
✅ Les noms suivent docs/CONVENTIONS.md
✅ Il suggère de consulter la doc en cas de doute

### Claude Code a besoin d'aide quand :

⚠️ Il ne suit pas les conventions → Rappeler docs/CONVENTIONS.md
⚠️ Il utilise des patterns différents → Rappeler docs/ARCHITECTURE.md
⚠️ Il ne respecte pas l'ordre → Montrer docs/CONVENTIONS.md
⚠️ Il oublie le typage → Rappeler le TypeScript strict

## 🔄 Mise à jour de la documentation

Quand vous ajoutez une nouvelle convention ou pattern :

1. Mettre à jour le fichier approprié dans `docs/`
2. Informer Claude : "La convention X a changé, lis docs/CONVENTIONS.md"
3. Vérifier que les futurs codes respectent la nouvelle convention

## 💡 Tips et astuces

### Tip 1 : Contexte persistant

Au début de la session :
```
Pour toute la session, respecte docs/CONVENTIONS.md et
docs/ARCHITECTURE.md. Je ne le répéterai pas à chaque fois.
```

### Tip 2 : Vérification automatique

Après chaque génération de code :
```
Vérifie automatiquement la conformité avec docs/CONVENTIONS.md.
```

### Tip 3 : Apprentissage itératif

```
Lis docs/EXTENDING.md section [X] et crée un exemple similaire
pour [Y].
```

### Tip 4 : Comparaison

```
Compare ce code avec l'exemple de docs/EXTENDING.md et
dis-moi les différences.
```

## 📚 Ressources complémentaires

- Documentation complète : `docs/README.md`
- Exemples de code : `docs/EXTENDING.md`
- Patterns : `docs/ARCHITECTURE.md`
- Conventions : `docs/CONVENTIONS.md`

## ❓ FAQ

**Q : Claude ne suit pas les conventions, que faire ?**
R : Rappeler explicitement le fichier : "Respecte docs/CONVENTIONS.md"

**Q : Claude utilise un pattern différent, pourquoi ?**
R : Il n'a peut-être pas lu la doc. Demander : "Lis docs/ARCHITECTURE.md d'abord"

**Q : Comment éviter de répéter le contexte ?**
R : Le donner une fois au début : "Pour toute la session, suis docs/..."

**Q : Claude peut-il mettre à jour la documentation ?**
R : Oui ! "Ajoute cette convention à docs/CONVENTIONS.md"

---

**Note** : Cette documentation évoluera avec le projet. N'hésitez pas à l'améliorer en fonction de votre expérience.

**Dernière mise à jour** : 2026-07-30
