`````markdown
# 📁 Features — Prototypes & Spécifications

Ce dossier contient la **conception produit** de l'application : brainstorming,
prototypes HTML/JS et PRD pour chaque fonctionnalité, avant et pendant leur
implémentation dans l'application principale.

## 🔄 Workflow

Chaque feature suit le même cycle :

1. **🧠 Brainstorming** — session avec agent IA pour explorer le besoin et les approches
2. **🟠 Prototype** — génération d'un prototype HTML/JS autonome pour valider l'UX
3. **📝 PRD** — rédaction du cahier des charges à partir du prototype validé
4. **🟡 Implémentation** — développement dans l'app principale (branche dédiée)
5. **✅ Livraison** — la feature est intégrée, le prototype est archivé

## 📂 Structure d'une feature

```
features/
└── altitude/
    ├── PRD.md                     # Product Requirements Document
    ├── specifications.md          # Spécifications fonctionnelles détaillées
    ├── algorithmes.md             # Description des algorithmes (si applicable)
    ├── documentation-technique.md # Documentation d'implémentation
    ├── integration.md             # Notes d'intégration dans l'app principale
    └── prototype/
        └── index.html             # Prototype autonome, libs via CDN
```

Tous les fichiers markdown sont **optionnels selon la complexité** : une feature
simple peut se contenter du `PRD.md`, une feature algorithmique ajoutera
`algorithmes.md`, etc.

**Conventions :**
- Nom du dossier en minuscules, sans accent ni espace (`altimetrie`, pas `Altimétrie`)
- Le prototype doit s'ouvrir par simple double-clic dans le navigateur
- Dépendances externes via CDN uniquement, jamais de `node_modules`

## 📋 Index des features

| Feature            | Statut               | Documentation                     | Prototype                                     | Livrée le   |
|--------------------|----------------------|-----------------------------------|-----------------------------------------------|-------------|
| altimetrie         | 🔵 Prototype validé  | [dossier](./altimetrie/)          | [prototype](./altimetrie/prototype/)          | —           |
| communes           | ✅ Livrée            | [dossier](./communes/)            | [prototype](./communes/prototype/)            | 14 oct 2026 |
| etiquettes         | 🔵 Prototype validé  | [dossier](./etiquettes/)          | [prototype](./etiquettes/prototype/)          | —           |
| passages-multiples | ✅ Livrée            | [dossier](./passages-multiples/)  | [prototype](./passages-multiples/prototype/)  | 17 sep 2026 |
| verification-gpx   | ✅ Livrée            | [dossier](./verification-gpx/)    | [prototype](./verification-gpx/prototype/)    | 16 sep 2026 |
| telecommande       | 💡 Idée              | [dossier](./telecommande/)        | [prototype](./telecommande/prototype/)        | —           |

### Statuts

| Emoji | Signification                        |
|-------|--------------------------------------|
| 💡    | Idée, pas encore explorée            |
| 🧠    | Brainstorming en cours               |
| 🟠    | Prototype en cours de validation     |
| 🔵    | Prototype validé — PRD à rédiger     |
| 📝    | PRD en rédaction                     |
| 🟡    | En implémentation                    |
| ✅    | Livrée dans l'application            |
| 🗄️    | Abandonnée (historique conservé)     |

## 🔗 Liens

- [Roadmap](../ROADMAP.md)
- [Notes d'architecture](../ARCHITECTURE.md)


