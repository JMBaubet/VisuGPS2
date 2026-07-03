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
    pub is_overridden: bool,
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

// Parcourt récursivement le TOML par défaut pour extraire les définitions de paramètres
fn flatten_settings(table: &toml::Table, prefix: &str, acc: &mut Vec<SettingDefinition>) {
    if table.contains_key("type") {
        if let Some(setting_type) = table.get("type").and_then(|v| v.as_str()) {
            let description = table.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let doc = table.get("doc").and_then(|v| v.as_str()).unwrap_or("").to_string();
            
            let default_val = if let Some(d) = table.get("default") {
                toml_to_json(d)
            } else {
                serde_json::Value::Null
            };

            let min = table.get("min").and_then(|v| v.as_integer());
            let max = table.get("max").and_then(|v| v.as_integer());
            let step = table.get("step").and_then(|v| v.as_integer());
            let critique = table.get("critique").and_then(|v| v.as_bool());

            acc.push(SettingDefinition {
                path: prefix.to_string(),
                description,
                doc,
                setting_type: setting_type.to_string(),
                default: default_val.clone(),
                value: default_val,
                min,
                max,
                step,
                critique,
                is_overridden: false,
            });
        }
    } else {
        for (k, v) in table {
            if let Some(sub_table) = v.as_table() {
                let next_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_settings(sub_table, &next_prefix, acc);
            }
        }
    }
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
            if def.setting_type == "Secret" {
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
            if def.setting_type == "Secret" {
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
        "Entier" => {
            let val_i64 = value.as_i64()
                .ok_or_else(|| format!("Valeur invalide pour un entier : {:?}", value))?;
            
            if let Some(min) = def.min {
                if val_i64 < min {
                    return Err(format!("Valeur inférieure au minimum autorisé ({})", min));
                }
            }
            if let Some(max) = def.max {
                if val_i64 > max {
                    return Err(format!("Valeur supérieure au maximum autorisé ({})", max));
                }
            }
            toml::Value::Integer(val_i64)
        }
        "Secret" => {
            let val_str = value.as_str()
                .ok_or_else(|| format!("Valeur invalide pour un secret : {:?}", value))?;
            
            if val_str.is_empty() {
                toml::Value::String("".to_string())
            } else {
                let encrypted = encrypt_secret(val_str, &state_write.master_key)?;
                toml::Value::String(encrypted)
            }
        }
        _ => {
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
            if def.setting_type == "Secret" {
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
