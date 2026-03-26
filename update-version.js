#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import readline from 'readline';

// Couleurs pour le terminal
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  blue: '\x1b[34m',
};

// Créer une interface de lecture pour les inputs
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
});

// Fonction helper pour poser une question
const question = (prompt) => {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      resolve(answer);
    });
  });
};

// Valider le format de version (semver)
const validateVersion = (version) => {
  const semverRegex = /^\d+\.\d+\.\d+(-[a-zA-Z0-9]+)?$/;
  return semverRegex.test(version);
};

// Parser TOML simple pour Cargo.toml
const parseToml = (content) => {
  const lines = content.split('\n');
  const result = {};
  let currentSection = null;

  lines.forEach((line) => {
    line = line.trim();

    // Ignorer les commentaires et lignes vides
    if (!line || line.startsWith('#')) return;

    // Détecter les sections [section]
    const sectionMatch = line.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      if (!result[currentSection]) result[currentSection] = {};
      return;
    }

    // Parser les clés = valeurs
    const kvMatch = line.match(/^([^=]+?)=(.+)$/);
    if (kvMatch && currentSection) {
      let key = kvMatch[1].trim();
      let value = kvMatch[2].trim();

      // Supprimer les guillemets
      if ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))) {
        value = value.slice(1, -1);
      }

      result[currentSection][key] = value;
    }
  });

  return result;
};

// Reconstruire le TOML à partir de l'objet parsé
const stringifyToml = (obj, originalContent) => {
  const lines = originalContent.split('\n');
  let result = [];
  let currentSection = null;
  const processed = new Set();

  lines.forEach((line) => {
    const trimmed = line.trim();

    // Détecter les sections
    const sectionMatch = trimmed.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      result.push(line);
      return;
    }

    // Détecter les clés à mettre à jour
    const kvMatch = trimmed.match(/^([^=]+?)=/);
    if (kvMatch && currentSection && obj[currentSection] !== undefined) {
      const key = kvMatch[1].trim();
      if (obj[currentSection][key] !== undefined) {
        const newValue = obj[currentSection][key];
        
        // N'ajouter des guillemets que si la valeur n'est pas déjà formatée (tableau, objet ou déjà citée)
        if (typeof newValue === 'string' && 
            !newValue.startsWith('"') && 
            !newValue.startsWith("'") && 
            !newValue.startsWith('[') && 
            !newValue.startsWith('{')) {
          result.push(`${key} = "${newValue}"`);
        } else {
          result.push(`${key} = ${newValue}`);
        }
        
        processed.add(`${currentSection}.${key}`);
        return;
      }
    }

    result.push(line);
  });

  return result.join('\n');
};

// Fonction principale
async function main() {
  try {
    console.log(`\n${colors.bright}${colors.blue}🚀 Mise à jour de la version${colors.reset}\n`);

    // Vérifier que les fichiers existent
    const files = ['package.json', 'src-tauri/Cargo.toml', 'src-tauri/tauri.conf.json'];
    const missingFiles = files.filter(f => !fs.existsSync(f));

    if (missingFiles.length > 0) {
      console.log(`${colors.red}❌ Fichiers manquants: ${missingFiles.join(', ')}${colors.reset}`);
      process.exit(1);
    }

    // Lire les fichiers actuels
    const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf-8'));
    const cargoToml = fs.readFileSync('src-tauri/Cargo.toml', 'utf-8');
    const parsedCargo = parseToml(cargoToml);
    const tauriConf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf-8'));

    // Afficher la version actuelle
    console.log(`${colors.yellow}Version actuelle: ${colors.bright}${packageJson.version}${colors.reset}\n`);

    // Demander la nouvelle version
    let version = await question(`${colors.blue}Nouvelle version${colors.reset} (format X.Y.Z) [${packageJson.version}]: `);
    version = version.trim() || packageJson.version;
    
    if (!validateVersion(version)) {
      throw new Error(`Format de version invalide. Utilisez X.Y.Z (ex: 1.2.3)`);
    }

    // Mettre à jour package.json
    packageJson.version = version;
    fs.writeFileSync('package.json', JSON.stringify(packageJson, null, 2) + '\n');

    // Mettre à jour Cargo.toml
    if (!parsedCargo.package) parsedCargo.package = {};
    parsedCargo.package.version = version;
    const updatedCargo = stringifyToml(parsedCargo, cargoToml);
    fs.writeFileSync('src-tauri/Cargo.toml', updatedCargo);

    // Mettre à jour tauri.conf.json
    tauriConf.version = version;
    fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(tauriConf, null, 2) + '\n');

    // Afficher les résultats
    console.log(`\n${colors.green}✅ Version mise à jour à ${version}${colors.reset}\n`);

    rl.close();
    process.exit(0);

  } catch (error) {
    console.error(`\n${colors.red}❌ Erreur: ${error.message}${colors.reset}\n`);
    rl.close();
    process.exit(1);
  }
}

main();
