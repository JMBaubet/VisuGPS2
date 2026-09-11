# Module Audit GPX — Documentation

**Fichier** : `docs/audit/README.md`

**Rôle** : index du dossier `docs/audit/`. Ce document sert de point
d'entrée pour tout développeur (humain ou IA) qui intervient sur le
module Audit GPX de VisuGPS2.

---

## 1. Objet

Ce dossier regroupe **toute la documentation** relative au module
Audit GPX de VisuGPS2 :

- les **spécifications normatives** des algorithmes et du moteur de
  correction (documents fournis, non modifiables) ;
- l'**artefact de référence** HTML (implémentation JS gelée, consultable
  en cas de doute) ;
- les **fichiers de test** GPX des scénarios de régression ;
- le **plan d'intégration** en 11 livrables (produits par la
  conception, à suivre dans l'ordre pour l'implémentation).

Le module Audit GPX **remplace** l'ancien module Nettoyage
(`cleaning.rs` + `Cleaning.vue`). Il détecte deux familles d'anomalies :
- **AR** : aller-retours ponctuels (rebonds, aiguilles de traceur)
- **RP** : boucles de giratoire (270°, 360° ou plus)

et propose des corrections (suppression de points, routage
OpenRouteService, faux positif) avec annulation par anomalie.

---

## 2. Structure du dossier

```
docs/audit/
├── README.md                       (ce fichier)
│
├── spec/                           Spécifications NORMATIVES
│   ├── ANALYSE.md                  Détecteurs AR et RP (§1 à §17)
│   ├── CORRECTIONS.md              Moteur de correction (§1 à §12)
│   └── IHM_V2.md                   Interface (INSPIRATION uniquement)
│
├── reference/                      Artefact de référence GELÉ
│   ├── verifgpx-V3.0.html          Implémentation JS de référence
│   └── test_files/                 Fichiers GPX des scénarios
│       ├── scenario_17_1_ar_27pts.gpx
│       ├── AR_Detecté_aussi_en_RP.gpx
│       ├── rondpoints_g1.gpx
│       ├── rondpoints_g2.gpx
│       ├── rondpoints_g3.gpx
│       ├── RP_Santa_Susanna.gpx
│       ├── AR_Santa_Susanna.gpx
│       └── RP_erreur_Magny.gpx
│
└── integration/                    Plan d'intégration (11 livrables)
    ├── 01_TYPES_RS.md
    ├── 02_AUDIT_TS.md
    ├── 03_PHASE_AR.md
    ├── 04_COMMANDS_TAURI.md
    ├── 05_PLAN_MIGRATION.md
    ├── 06_SETTINGS_AUDIT.md
    ├── 07_ARBORESCENCE_CIBLE.md
    ├── 08_PHASE_RP.md
    ├── 09_CORRECTIONS.md
    ├── 10_COMPOSANTS_VUE.md
    └── 11_README.md                (copie de ce fichier)
```

---

## 3. Hiérarchie de lecture

### 3.1 Pour comprendre les algorithmes de détection

1. **`spec/ANALYSE.md`** — spécification normative. Décrit les deux
   détecteurs AR et RP de façon exhaustive : modèle de données,
   paramètres, pseudocodes phase par phase, contrat de sortie,
   invariants, cas limites, scénarios de validation.
2. **`reference/verifgpx-V3.0.html`** — implémentation JS de référence.
   **Consultable en cas de doute** sur un détail du portage Rust.
   **Gelé** : ne jamais modifier.

### 3.2 Pour comprendre le moteur de correction

1. **`spec/CORRECTIONS.md`** — spécification normative. Décrit le
   modèle d'état des anomalies, le réalignement des index par
   identifiants stables, les trois transformations (suppression,
   routage, faux positif), l'annulation, la règle d'imbrication et
   les invariants C1 à C12.

### 3.3 Pour comprendre l'IHM de référence

1. **`spec/IHM_V2.md`** — **⚠️ Inspiration uniquement**. Décrit
   l'IHM du HTML de référence. Les **comportements fonctionnels** sont
   à reproduire (6 vues du panneau d'action, prévisualisations,
   contrats réseau ORS, export GPX). L'**implémentation** doit suivre
   les conventions VisuGPS2 (Vuetify 3, Pinia setup stores,
   `docs/CONVENTIONS.md`).

### 3.4 Pour implémenter le module

Lire les 11 livrables dans l'ordre :
`integration/01_TYPES_RS.md` → `integration/11_README.md`.

Chaque livrable est autonome et contient :
- les objectifs,
- les signatures Rust/TypeScript,
- les plans d'implémentation,
- les tests à écrire,
- les points de vigilance,
- les critères de validation.

---

## 4. Décisions actées (12)

Ces décisions sont **figées**. Elles ne se remettent pas en question
sans une révision explicite documentée en avenant.

| # | Décision | Livrable de référence |
|---|---|---|
| **1** | Algorithmes en Rust, portage en 2 phases (AR puis RP) | `03_PHASE_AR.md`, `08_PHASE_RP.md` |
| **2** | Remplacement total de `Cleaning.vue` | `05_PLAN_MIGRATION.md` |
| **3** | Troisième carte Mapbox GL dédiée (`AuditMap.vue`) | `10_COMPOSANTS_VUE.md` |
| **4** | `audit_status` remplace `cleaning_status` (gate dur, détection lourde à l'import) | `05_PLAN_MIGRATION.md`, `06_SETTINGS_AUDIT.md` |
| **5** | Namespace `Audit.*` dans `settings.default.toml`, clés ORS en `secret` chiffré | `06_SETTINGS_AUDIT.md` |
| **6** | Findings volatiles (pas de persistance entre sessions) | `02_AUDIT_TS.md` |
| **7** | Import synchrone | `04_COMMANDS_TAURI.md` |
| **8** | Bouton « Appliquer » strict (`pending === 0`), ferme la vue, non annulable | `10_COMPOSANTS_VUE.md` |
| **9** | Avertissement avant de quitter `/audit` si travail en cours, reset du store à la sortie | `10_COMPOSANTS_VUE.md` |
| **10** | Base vierge (D1 : ignorer anciens registres ; D2c : supprimer `cleaning.*.json` au premier accès ; D3b : silencieux) | `05_PLAN_MIGRATION.md` |
| **11** | Module Rust modulaire : `src-tauri/src/gpx_audit/*.rs` | `07_ARBORESCENCE_CIBLE.md` |
| **12** | Ajout du type `"string"` dans le système de paramètres | `06_SETTINGS_AUDIT.md` |

---

## 5. Plan d'implémentation — Vue d'ensemble

L'implémentation se déroule en **4 phases**, chacune validée avant de
passer à la suivante.

### Phase 1 — Détecteur AR (Rust)

**Livrables** : `01_TYPES_RS.md`, `03_PHASE_AR.md`

**Objectif** : porter `detectAR` du HTML de référence en Rust, testé,
validé sur le scénario §17.1.

**Livrable** : `src-tauri/src/gpx_audit/ar.rs` + 18 tests verts.

**Validation** : validation à chaque sous-étape (5 sous-étapes).

### Phase 2 — Détecteur RP + ancres (Rust)

**Livrables** : `08_PHASE_RP.md`

**Objectif** : porter `detectRP` + `rpAnchorIndices`, avec les 12
étapes algorithmiques et les gardes anti-aiguille.

**Livrable** : `src-tauri/src/gpx_audit/rp.rs`,
`src-tauri/src/gpx_audit/anchor.rs` + 51 tests verts.

**Validation** : validation à chaque sous-étape (5 sous-étapes),
+ scénarios §17.2 (7 fichiers de test).

### Phase 3 — Moteur de correction (Rust)

**Livrables** : `09_CORRECTIONS.md`

**Objectif** : porter `sync_indexes`, `apply_delete`, `apply_route`,
`mark_fp`, `unmark_fp`, `undo_correction`, `nesting_guard`,
`absorb_fp_findings`, `migration`, `export`.

**Livrable** : `src-tauri/src/gpx_audit/corrections.rs`,
`migration.rs`, `export.rs` + 48 tests verts.

**Validation** : 12 scénarios D1..D12 (CORRECTIONS §12), invariants
C1..C12 vérifiables.

### Phase 4 — Intégration frontend (Vue + Tauri)

**Livrables** : `02_AUDIT_TS.md`, `04_COMMANDS_TAURI.md`,
`05_PLAN_MIGRATION.md`, `06_SETTINGS_AUDIT.md`,
`10_COMPOSANTS_VUE.md`

**Objectif** : store Pinia, commandes Tauri, vue `Audit.vue`,
composants `Audit/*`, redirections, migration, paramètres.

**Livrable** : intégration complète, tests de bout en bout.

**Validation** : validation à chaque sous-étape (4 sous-étapes :
squelette, carte, panneau d'action, intégration ORS).

---

## 6. Règles de maintenance

### 6.1 Fichiers à NE JAMAIS modifier

| Fichier | Raison |
|---|---|
| `spec/ANALYSE.md` | Spécification normative — tout changement casse le contrat |
| `spec/CORRECTIONS.md` | Idem |
| `reference/verifgpx-V3.0.html` | Artefact de référence gelé — il n'est plus maintenu |
| `reference/test_files/*.gpx` | Fichiers de test référencés par des verdicts documentés |

### 6.2 Fichiers modifiables avec précaution

| Fichier | Règle |
|---|---|
| `spec/IHM_V2.md` | Inspiration uniquement — un écart d'implémentation est autorisé si justifié |
| `integration/*.md` | Peut évoluer par **avenant** (ajout d'une section, pas modification du contenu) |

### 6.3 Fichiers à ajouter au besoin

| Répertoire | Règle |
|---|---|
| `reference/test_files/` | Ajouter de **nouveaux** fichiers de régression si nécessaire, sans modifier les existants |
| `integration/` | Ajouter un nouveau livrable (12, 13…) si une extension du périmètre est actée |

### 6.4 Mise à jour de la documentation projet

Quand le module est terminé, mettre à jour les documents **globaux**
de VisuGPS2 :

- `docs/COMMANDS.md` : retirer les 5 commandes cleaning, ajouter les 7 commandes audit
- `docs/ARCHITECTURE.md` : retirer le module cleaning, ajouter `gpx_audit`
- `docs/DATA_STORAGE.md` : retirer les mentions de `cleaning.*.json`
- `docs/CONTEXT.md` : remplacer la description « nettoyage » par « audit »
- `docs/SPEC_IMPORT_GPX.md` : le champ `cleaning_status` devient `audit_status`
- `CHANGELOG.md` (si existant) : mentionner la migration

---

## 7. Récapitulatif des fichiers de test

Fichiers de régression utilisés par les tests Rust :

| Fichier | Scénario | Verdict attendu |
|---|---|---|
| `scenario_17_1_ar_27pts.gpx` | AR §17.1 | 1 finding au pt 12, 3 paires (0/0/40 m), emprise 8-16 |
| `rondpoints_g1.gpx` | RP — tour + branches communes | 360°, fenêtre propre, paires 0 m |
| `rondpoints_g2.gpx` | RP — tour complet | 360° |
| `rondpoints_g3.gpx` | RP — refermeture non exacte | 330-350° bruts → 360° après quantif |
| `RP_Santa_Susanna.gpx` | RP — tour + réengagement | ~175° lissé + ~164° de coin → 360° |
| `AR_Santa_Susanna.gpx` | AR — aiguille | Rejeté par RP (anti-aiguille) + détecté par AR |
| `AR_Detecté_aussi_en_RP.gpx` | AR — aiguille ambiguë | Rejeté par RP + détecté par AR |
| `RP_erreur_Magny.gpx` | RP — A/R macroscopique + giratoire 4 tours | 1440°, fenêtre bornée |

Ces fichiers sont utilisés par les tests `ar_test.rs`, `rp_test.rs`
et les scénarios d'intégration.

---

## 8. Contact et responsabilité

| Rôle | Responsable |
|---|---|
| Conception initiale | À documenter lors de la première itération |
| Maintenance | Équipe VisuGPS2 |
| Référence algorithmique | `spec/ANALYSE.md` (normatif) |
| Référence corrections | `spec/CORRECTIONS.md` (normatif) |

---

## 9. Statut et historique

| Version | Date | Changements |
|---|---|---|
| 1.0 | 2026-01-XX | Création initiale — 12 décisions actées, 11 livrables produits |

**Statut actuel** : documentation prête, implémentation à démarrer.

**Prochaine étape** : lancer Zcode sur la Phase 1 (portage AR) avec le
prompt fourni dans le Livrable 03 (`03_PHASE_AR.md`) et valider à
chaque sous-étape.

---

**Fin du document.**