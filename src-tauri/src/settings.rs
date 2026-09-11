use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Manager};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SettingDefinition {
    pub path: String,
    pub description: String,
    #[serde(rename = "documentation")] // anciennement "doc"
    pub documentation: String,
    #[serde(rename = "type")]
    pub setting_type: String,
    pub default: serde_json::Value,
    pub value: serde_json::Value,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    #[serde(rename = "critical")] // anciennement "critique"
    pub critical: Option<bool>,
    pub unit: Option<String>,
    pub choices: Option<Vec<serde_json::Value>>,
    /// Icône MDI optionnelle pour le drawer (défaut = icône par type côté frontend).
    pub icon: Option<String>,
    pub is_overridden: bool,
}

// --- Métadonnées d'organisation du drawer (table `_meta` du TOML) ----------

/// Métadonnées d'une entrée d'action (non-paramètre) en section système.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActionEntry {
    pub label: String,
    pub icon: String,
    /// Nom du handler frontend à invoquer (ex: "openModes").
    pub action: String,
}

/// Paramètres communs à toutes les vues (section système du drawer).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemMeta {
    /// Groupes racine toujours visibles (ex: "Systeme", "Affichage").
    pub groups: Vec<String>,
    /// Entrées d'action affichées en tête de la section système.
    pub actions: BTreeMap<String, ActionEntry>,
    /// Groupes utilisant un handler spécial au lieu de ParameterCard individuel
    /// (ex: "Affichage.moniteurs" -> "monitors").
    pub handlers: BTreeMap<String, String>,
}

/// Métadonnées d'une vue applicative.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ViewMeta {
    pub label: String,
    pub icon: String,
    /// Catégories (groupes) exposées par cette vue.
    pub groups: Vec<String>,
}

/// Métadonnées d'affichage d'une catégorie (libellé / icône optionnels).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupMeta {
    pub label: Option<String>,
    pub icon: Option<String>,
}

/// Organisation complète du drawer, lue depuis la table `_meta` du TOML.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SettingsMeta {
    pub system: SystemMeta,
    /// Clé = nom de la vue (ex: "accueil", "carte"), aligné sur les noms de route.
    pub views: BTreeMap<String, ViewMeta>,
    /// Clé = identifiant de groupe (ex: "Carte.Traces").
    pub groups: BTreeMap<String, GroupMeta>,
}

pub struct SettingsState {
    pub default_toml: toml::Table,
    pub user_overrides: toml::Table,
    pub config_path: PathBuf,
    pub master_key: [u8; 32],
}

// Récupère ou génère la clé maîtresse de chiffrement.
// En mode dev, pour éviter les popups intempestifs du keyring OS sur macOS non-signé,
// on utilise une clé statique de développement. En prod, on utilise le trousseau système.
fn get_or_create_master_key() -> Result<[u8; 32], String> {
    if cfg!(debug_assertions) {
        let key = b"dev_master_key_32_bytes_long_!!!";
        return Ok(*key);
    }

    let entry = keyring::Entry::new("visugps2_master_key", "system")
        .map_err(|e| format!("Erreur keyring : {}", e))?;
    
    match entry.get_password() {
        Ok(password_b64) => {
            let bytes = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, &password_b64)
                .map_err(|e| format!("Erreur décodage clé maîtresse : {}", e))?;
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Ok(key)
            } else {
                Err("Clé maîtresse corrompue dans le keyring".to_string())
            }
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            let password_b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &key);
            entry.set_password(&password_b64)
                .map_err(|e| format!("Erreur écriture keyring : {}", e))?;
            Ok(key)
        }
        Err(e) => Err(format!("Erreur lecture keyring : {}", e)),
    }
}

// Chiffre un secret avec AES-256-GCM et renvoie une chaîne Base64 combinant le nonce et le texte chiffré
fn encrypt_secret(plaintext: &str, master_key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(master_key.into());
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Erreur de chiffrement : {}", e))?;
        
    let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    
    Ok(base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &combined))
}

// Déchiffre un secret à partir de sa représentation Base64
fn decrypt_secret(ciphertext_b64: &str, master_key: &[u8; 32]) -> Result<String, String> {
    let combined = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, ciphertext_b64)
        .map_err(|e| format!("Erreur décodage base64 : {}", e))?;
        
    if combined.len() < 12 {
        return Err("Données chiffrées invalides".to_string());
    }
    
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new(master_key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Erreur de déchiffrement : {}", e))?;
        
    String::from_utf8(plaintext_bytes)
        .map_err(|e| format!("Erreur encodage UTF-8 : {}", e))
}

// Convertit une valeur TOML en valeur JSON équivalente
fn toml_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(*f) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::Null
            }
        }
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(toml_to_json).collect())
        }
        toml::Value::Table(tbl) => {
            let mut map = serde_json::Map::new();
            for (k, v) in tbl {
                map.insert(k.clone(), toml_to_json(v));
            }
            serde_json::Value::Object(map)
        }
    }
}

// Convertit une valeur JSON en valeur TOML équivalente
fn json_to_toml(val: &serde_json::Value) -> Option<toml::Value> {
    match val {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Some(toml::Value::Float(f))
            } else {
                None
            }
        }
        serde_json::Value::String(s) => Some(toml::Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let toml_arr: Vec<toml::Value> = arr.iter().filter_map(json_to_toml).collect();
            Some(toml::Value::Array(toml_arr))
        }
        serde_json::Value::Object(obj) => {
            let mut table = toml::Table::new();
            for (k, v) in obj {
                if let Some(toml_v) = json_to_toml(v) {
                    table.insert(k.clone(), toml_v);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

// Lit une valeur TOML comme f64, en acceptant indifféremment un entier ou un flottant.
// Nécessaire car `min = 0.5` est un Float et `min = 1` un Integer en TOML, et que l'on
// doit supporter à la fois `int` et `float` côté paramètres.
fn toml_as_f64(val: &toml::Value) -> Option<f64> {
    match val {
        toml::Value::Integer(i) => Some(*i as f64),
        toml::Value::Float(f) => Some(*f),
        _ => None,
    }
}

// Valide qu'une chaîne est une couleur hexadécimale avec canal alpha : #RRGGBBAA.
fn is_hex_alpha(s: &str) -> bool {
    let bytes = s.as_bytes();
    s.len() == 9 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

// Parcourt récursivement le TOML par défaut pour extraire les définitions de paramètres
fn flatten_settings(table: &toml::Table, prefix: &str, acc: &mut Vec<SettingDefinition>) {
    if table.contains_key("type") {
        if let Some(setting_type) = table.get("type").and_then(|v| v.as_str()) {
            let description = table.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let documentation = table.get("documentation")
                .or_else(|| table.get("doc")) // rétro-compatibilité temporaire
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let default_val = if let Some(d) = table.get("default") {
                toml_to_json(d)
            } else {
                serde_json::Value::Null
            };

            let min = table.get("min").and_then(toml_as_f64);
            let max = table.get("max").and_then(toml_as_f64);
            let step = table.get("step").and_then(toml_as_f64);
            let critical = table.get("critical")
                .or_else(|| table.get("critique")) // rétro-compatibilité temporaire
                .and_then(|v| v.as_bool());
            let unit = table.get("unit").and_then(|v| v.as_str()).map(|s| s.to_string());
            let icon = table.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string());
            let choices = table.get("choices").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().map(toml_to_json).collect::<Vec<_>>()
            });

            acc.push(SettingDefinition {
                path: prefix.to_string(),
                description,
                documentation,
                setting_type: setting_type.to_string(),
                default: default_val.clone(),
                value: default_val,
                min,
                max,
                step,
                critical,
                unit,
                choices,
                icon,
                is_overridden: false,
            });
        }
    } else {
        for (k, v) in table {
            // La table `_meta` décrit l'organisation du drawer : ce n'est pas
            // un paramètre, on l'ignore lors du flatten (lue par extract_meta).
            if prefix.is_empty() && k == "_meta" {
                continue;
            }
            if let Some(sub_table) = v.as_table() {
                let next_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_settings(sub_table, &next_prefix, acc);
            }
        }
    }
}

// Extrait les métadonnées d'organisation (_meta) du TOML par défaut.
// Renvoie une structure vide si la table `_meta` est absente ou mal formée,
// de façon à ne jamais bloquer le démarrage de l'application.
fn extract_meta(default_table: &toml::Table) -> SettingsMeta {
    let empty = SettingsMeta {
        system: SystemMeta {
            groups: Vec::new(),
            actions: BTreeMap::new(),
            handlers: BTreeMap::new(),
        },
        views: BTreeMap::new(),
        groups: BTreeMap::new(),
    };

    let meta = match default_table.get("_meta").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return empty,
    };

    // --- system ---
    let system = meta.get("system").and_then(|v| v.as_table());
    let system_meta = if let Some(sys) = system {
        let groups = sys.get("groups")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut actions = BTreeMap::new();
        if let Some(acts) = sys.get("actions").and_then(|v| v.as_table()) {
            for (key, val) in acts {
                if let Some(entry) = val.as_table() {
                    let label = entry.get("label").and_then(|v| v.as_str()).unwrap_or(key).to_string();
                    let icon = entry.get("icon").and_then(|v| v.as_str()).unwrap_or("mdi-cog").to_string();
                    let action = entry.get("action").and_then(|v| v.as_str()).unwrap_or(key).to_string();
                    actions.insert(key.clone(), ActionEntry { label, icon, action });
                }
            }
        }

        let mut handlers = BTreeMap::new();
        if let Some(hs) = sys.get("handlers").and_then(|v| v.as_table()) {
            for (key, val) in hs {
                if let Some(s) = val.as_str() {
                    handlers.insert(key.clone(), s.to_string());
                }
            }
        }

        SystemMeta { groups, actions, handlers }
    } else {
        empty.system.clone()
    };

    // --- views ---
    let mut views = BTreeMap::new();
    if let Some(views_tbl) = meta.get("views").and_then(|v| v.as_table()) {
        for (name, val) in views_tbl {
            if let Some(v) = val.as_table() {
                let label = v.get("label").and_then(|x| x.as_str()).unwrap_or(name).to_string();
                let icon = v.get("icon").and_then(|x| x.as_str()).unwrap_or("mdi-eye").to_string();
                let groups = v.get("groups")
                    .and_then(|x| x.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                views.insert(name.clone(), ViewMeta { label, icon, groups });
            }
        }
    }

    // --- groups (métadonnées d'affichage des catégories) ---
    let mut groups = BTreeMap::new();
    if let Some(groups_tbl) = meta.get("groups").and_then(|v| v.as_table()) {
        for (id, val) in groups_tbl {
            if let Some(g) = val.as_table() {
                let label = g.get("label").and_then(|x| x.as_str()).map(|s| s.to_string());
                let icon = g.get("icon").and_then(|x| x.as_str()).map(|s| s.to_string());
                groups.insert(id.clone(), GroupMeta { label, icon });
            }
        }
    }

    SettingsMeta { system: system_meta, views, groups }
}

// Lit une valeur TOML par son chemin pointé (ex: "Accueil.nbrCircuits.list")
pub(crate) fn get_toml_value_by_path<'a>(table: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = table;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return current.get(*part);
        } else {
            current = current.get(*part)?.as_table()?;
        }
    }
    None
}

// Définit une valeur TOML à un chemin pointé, en créant les tables parentes si nécessaire
fn set_toml_value_by_path(table: &mut toml::Table, path: &str, value: toml::Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = table;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current.insert(part.to_string(), value);
            return;
        } else {
            if !current.contains_key(*part) {
                current.insert(part.to_string(), toml::Value::Table(toml::Table::new()));
            }
            // Temporairement, on doit s'assurer que c'est bien une table.
            // Si c'était autre chose, on l'écrase avec une table vide.
            if !current.get(*part).unwrap().is_table() {
                current.insert(part.to_string(), toml::Value::Table(toml::Table::new()));
            }
            current = current.get_mut(*part).unwrap().as_table_mut().unwrap();
        }
    }
}

// Supprime une valeur TOML à un chemin pointé
fn remove_toml_value_by_path(table: &mut toml::Table, path: &str) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = table;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return current.remove(*part).is_some();
        } else {
            if let Some(sub_table) = current.get_mut(*part).and_then(|v| v.as_table_mut()) {
                current = sub_table;
            } else {
                return false;
            }
        }
    }
    false
}

// Génère la liste des paramètres fusionnés (défaut + surcharges)
fn get_merged_settings(
    default_table: &toml::Table,
    overrides_table: &toml::Table,
) -> Vec<SettingDefinition> {
    let mut definitions = Vec::new();
    flatten_settings(default_table, "", &mut definitions);
    
    for def in &mut definitions {
        if let Some(user_val) = get_toml_value_by_path(overrides_table, &def.path) {
            def.is_overridden = true;
            if def.setting_type == "secret" {
                if let Some(ciphertext_b64) = user_val.as_str() {
                    if !ciphertext_b64.is_empty() {
                        def.value = serde_json::Value::String("********".to_string());
                    } else {
                        def.value = serde_json::Value::String("".to_string());
                    }
                }
            } else {
                def.value = toml_to_json(user_val);
            }
        } else {
            def.is_overridden = false;
            if def.setting_type == "secret" {
                if let Some(default_str) = def.default.as_str() {
                    if !default_str.is_empty() {
                        def.value = serde_json::Value::String("********".to_string());
                    } else {
                        def.value = serde_json::Value::String("".to_string());
                    }
                }
            } else {
                def.value = def.default.clone();
            }
        }
    }
    
    definitions
}

// Initialise le state de configuration
pub fn init_settings_state(app_handle: &AppHandle) -> Result<Arc<RwLock<SettingsState>>, String> {
    let default_toml_path = app_handle.path().resource_dir()
        .map_err(|e| format!("Dossier ressources introuvable : {}", e))?
        .join("settings.default.toml");
        
    let default_toml_str = std::fs::read_to_string(&default_toml_path)
        .map_err(|e| format!("Impossible de lire settings.default.toml dans {:?} : {}", default_toml_path, e))?;
        
    let default_toml: toml::Table = toml::from_str(&default_toml_str)
        .map_err(|e| format!("Erreur de parsing dans settings.default.toml : {}", e))?;

    let is_dev = cfg!(debug_assertions);
    let app_data_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Dossier app_data introuvable : {}", e))?;
        
    let active_mode = crate::gestionMode::read_active_mode(&app_data_dir, is_dev);
    let mode_dir = app_data_dir.join(&active_mode);
    
    // Création du répertoire du mode si inexistant
    if !mode_dir.exists() {
        std::fs::create_dir_all(&mode_dir)
            .map_err(|e| format!("Impossible de créer le dossier du mode {:?} : {}", mode_dir, e))?;
    }

    let config_filename = if is_dev { "config-dev.toml" } else { "config.toml" };
    let config_path = mode_dir.join(config_filename);
    
    let user_overrides = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Impossible de lire {:?} : {}", config_path, e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Erreur de parsing dans {:?} : {}", config_path, e))?
    } else {
        toml::Table::new()
    };

    let master_key = get_or_create_master_key()?;

    Ok(Arc::new(RwLock::new(SettingsState {
        default_toml,
        user_overrides,
        config_path,
        master_key,
    })))
}

// Commande Tauri : Récupère tous les paramètres (avec les secrets masqués)
#[tauri::command]
pub async fn get_settings(
    state: tauri::State<'_, Arc<RwLock<SettingsState>>>,
) -> Result<Vec<SettingDefinition>, String> {
    let state_read = state.read().await;
    Ok(get_merged_settings(&state_read.default_toml, &state_read.user_overrides))
}

// Commande Tauri : Récupère les métadonnées d'organisation du drawer (table `_meta`).
#[tauri::command]
pub async fn get_settings_meta(
    state: tauri::State<'_, Arc<RwLock<SettingsState>>>,
) -> Result<SettingsMeta, String> {
    let state_read = state.read().await;
    Ok(extract_meta(&state_read.default_toml))
}

// Commande Tauri : Met à jour un paramètre
#[tauri::command]
pub async fn update_setting(
    state: tauri::State<'_, Arc<RwLock<SettingsState>>>,
    path: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut state_write = state.write().await;
    
    let mut definitions = Vec::new();
    flatten_settings(&state_write.default_toml, "", &mut definitions);
    let def = definitions.iter().find(|d| d.path == path)
        .ok_or_else(|| format!("Paramètre inconnu : {}", path))?;
        
    let toml_val = match def.setting_type.as_str() {
        "int" => {
            let val_i64 = value.as_i64()
                .ok_or_else(|| format!("Valeur invalide pour un entier : {:?}", value))?;

            if let Some(min) = def.min {
                if (val_i64 as f64) < min {
                    return Err(format!("Valeur inférieure au minimum autorisé ({})", min));
                }
            }
            if let Some(max) = def.max {
                if (val_i64 as f64) > max {
                    return Err(format!("Valeur supérieure au maximum autorisé ({})", max));
                }
            }
            toml::Value::Integer(val_i64)
        }
        "float" => {
            let val_f64 = value.as_f64()
                .ok_or_else(|| format!("Valeur invalide pour un décimal : {:?}", value))?;

            // On conserve les bornes et le pas tels quels ; l'arrondi au pas est appliqué
            // côté UI. Ici on valide seulement l'intervalle.
            if let Some(min) = def.min {
                if val_f64 < min {
                    return Err(format!("Valeur inférieure au minimum autorisé ({})", min));
                }
            }
            if let Some(max) = def.max {
                if val_f64 > max {
                    return Err(format!("Valeur supérieure au maximum autorisé ({})", max));
                }
            }
            toml::Value::Float(val_f64)
        }
        "secret" => {
            let val_str = value.as_str()
                .ok_or_else(|| format!("Valeur invalide pour un secret : {:?}", value))?;

            if val_str.is_empty() {
                toml::Value::String("".to_string())
            } else {
                let encrypted = encrypt_secret(val_str, &state_write.master_key)?;
                toml::Value::String(encrypted)
            }
        }
        "bool" => {
            let val_bool = value.as_bool()
                .ok_or_else(|| format!("Valeur invalide pour un booléen : {:?}", value))?;
            toml::Value::Boolean(val_bool)
        }
        // Type `string` : texte libre, non chiffré, sans borne (décision 12).
        "string" => {
            let val_str = value.as_str()
                .ok_or_else(|| format!("Valeur invalide pour un texte : {:?}", value))?;
            toml::Value::String(val_str.to_string())
        }
        "list" => {
            let val_str = value.as_str()
                .ok_or_else(|| format!("Valeur invalide pour une liste (chaîne attendue) : {:?}", value))?
                .to_string();
            // Si des choix sont définis, on restreint la valeur à cet ensemble.
            if let Some(choices) = &def.choices {
                let allowed = choices.iter().any(|c| c.as_str() == Some(val_str.as_str()));
                if !allowed {
                    return Err(format!("Valeur '{}' non autorisée pour cette liste", val_str));
                }
            }
            toml::Value::String(val_str)
        }
        "rgba" | "material_primary" | "material_extended" => {
            // Toutes les couleurs sont stockées au format hexadécimal avec alpha : #RRGGBBAA.
            let val_str = value.as_str()
                .ok_or_else(|| format!("Valeur couleur invalide (chaîne hex attendue) : {:?}", value))?;
            if !is_hex_alpha(val_str) {
                return Err("Couleur invalide : format attendu #RRGGBBAA".to_string());
            }
            toml::Value::String(val_str.to_string())
        }
        _ => {
            // Fall-through générique pour les types non strictement validés (ex: "monitor").
            if let Some(toml_v) = json_to_toml(&value) {
                toml_v
            } else {
                return Err(format!("Type de valeur non supporté : {:?}", value));
            }
        }
    };
    
    set_toml_value_by_path(&mut state_write.user_overrides, &path, toml_val);
    
    let toml_str = toml::to_string(&state_write.user_overrides)
        .map_err(|e| format!("Erreur sérialisation TOML : {}", e))?;
    std::fs::write(&state_write.config_path, toml_str)
        .map_err(|e| format!("Erreur écriture fichier configuration : {}", e))?;
        
    Ok(())
}

// Commande Tauri : Réinitialise un paramètre à sa valeur par défaut (suppression de la surcharge)
#[tauri::command]
pub async fn reset_setting(
    state: tauri::State<'_, Arc<RwLock<SettingsState>>>,
    path: String,
) -> Result<(), String> {
    let mut state_write = state.write().await;
    
    remove_toml_value_by_path(&mut state_write.user_overrides, &path);
    
    let toml_str = toml::to_string(&state_write.user_overrides)
        .map_err(|e| format!("Erreur sérialisation TOML : {}", e))?;
    std::fs::write(&state_write.config_path, toml_str)
        .map_err(|e| format!("Erreur écriture fichier configuration : {}", e))?;
        
    Ok(())
}

// Commande Tauri : Récupère la valeur effective d'un paramètre (déchiffrée si c'est un secret) pour l'usage interne de l'app
#[tauri::command]
pub async fn get_setting_value(
    state: tauri::State<'_, Arc<RwLock<SettingsState>>>,
    path: String,
) -> Result<serde_json::Value, String> {
    let state_read = state.read().await;
    
    if let Some(user_val) = get_toml_value_by_path(&state_read.user_overrides, &path) {
        let mut definitions = Vec::new();
        flatten_settings(&state_read.default_toml, "", &mut definitions);
        if let Some(def) = definitions.iter().find(|d| d.path == path) {
            if def.setting_type == "secret" {
                if let Some(ciphertext_b64) = user_val.as_str() {
                    if ciphertext_b64.is_empty() {
                        return Ok(serde_json::Value::String("".to_string()));
                    }
                    let decrypted = decrypt_secret(ciphertext_b64, &state_read.master_key)?;
                    return Ok(serde_json::Value::String(decrypted));
                }
            }
        }
        return Ok(toml_to_json(user_val));
    }
    
    let mut definitions = Vec::new();
    flatten_settings(&state_read.default_toml, "", &mut definitions);
    if let Some(def) = definitions.iter().find(|d| d.path == path) {
        return Ok(def.default.clone());
    }

    Err(format!("Paramètre inconnu : {}", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lit le vrai `settings.default.toml` livré avec l'application et valide
    /// que la table `_meta` est correctement extraite et correctement exclue
    /// du flatten des paramètres.
    #[test]
    fn meta_is_extracted_and_excluded_from_flatten() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let toml_str = std::fs::read_to_string(format!("{}/settings.default.toml", manifest_dir))
            .expect("settings.default.toml doit être lisible");
        let table: toml::Table = toml::from_str(&toml_str)
            .expect("settings.default.toml doit être du TOML valide");

        // 1. `_meta` ne doit pas générer de paramètres fantômes.
        let mut defs = Vec::new();
        flatten_settings(&table, "", &mut defs);
        assert!(
            !defs.iter().any(|d| d.path.starts_with("_meta")),
            "la table `_meta` ne doit pas produire de paramètres"
        );
        assert!(
            defs.iter().any(|d| d.path == "Systeme.Key.mapBox"),
            "les paramètres réels doivent toujours être présents"
        );
        // La clé MapBox doit être marquée critique (drapeau `critical = true`
        // du TOML), pour que le drawer l'affiche en orange.
        let mapbox = defs.iter().find(|d| d.path == "Systeme.Key.mapBox").unwrap();
        assert_eq!(mapbox.critical, Some(true), "Systeme.Key.mapBox doit être critique");

        // 2. Les métadonnées sont extraites.
        let meta = extract_meta(&table);
        // Les groupes système sont les catégories réellement affichées
        // (sous-groupes porteurs de paramètres, pas les racines nues).
        assert!(meta.system.groups.contains(&"Systeme.Key".to_string()));
        assert!(
            meta.system.groups.contains(&"Affichage.moniteurs".to_string()),
            "groups système = {:?}",
            meta.system.groups
        );
        // Action système « Modes d'exécution ».
        let modes = meta.system.actions.get("modes").expect("action 'modes' déclarée");
        assert_eq!(modes.action, "openModes");
        // Handler spécial pour les moniteurs (clé quotée contenant un point).
        assert_eq!(
            meta.system.handlers.get("Affichage.moniteurs"),
            Some(&"monitors".to_string())
        );
        // Métadonnée de catégorie pour une clé quotée.
        assert_eq!(
            meta.groups.get("Affichage.moniteurs").and_then(|g| g.label.as_deref()),
            Some("Configuration des fenêtres")
        );
        // Au moins la vue `accueil` est déclarée.
        assert!(meta.views.contains_key("accueil"));
    }
}
