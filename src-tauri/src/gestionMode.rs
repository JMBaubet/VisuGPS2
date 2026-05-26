use std::path::Path;
use tauri::Manager;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeInfo {
    pub nom: String,
    pub descrition: String,
    #[serde(rename = "création")]
    pub creation: String,
    #[serde(rename = "révision")]
    pub revision: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeConfig {
    #[serde(rename = "Modes")]
    pub modes: Vec<ModeInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionEnv {
    pub is_dev: bool,
    pub active_mode_dev: String,
    pub active_mode_prod: String,
}

// Lit le mode actif depuis le fichier .env
fn read_active_mode(app_data_dir: &Path, is_dev: bool) -> String {
    let env_path = app_data_dir.join(".env");
    if !env_path.exists() {
        let _ = std::fs::create_dir_all(app_data_dir);
        let _ = std::fs::write(&env_path, "APP_ENV_DEV=OPE\nAPP_ENV_PROD=OPE\n");
        return "OPE".to_string();
    }

    if let Ok(content) = std::fs::read_to_string(&env_path) {
        let key = if is_dev { "APP_ENV_DEV" } else { "APP_ENV_PROD" };
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(&format!("{}=", key)) {
                let value = line.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
                // Enlever les guillemets éventuels
                let cleaned_value = value.trim_matches(|c| c == '"' || c == '\'');
                if !cleaned_value.is_empty() {
                    return cleaned_value.to_string();
                }
            }
        }
    }
    "OPE".to_string()
}

// Écrit le mode actif dans le fichier .env
fn write_active_mode(app_data_dir: &Path, is_dev: bool, new_mode: &str) -> Result<(), String> {
    let env_path = app_data_dir.join(".env");
    let mut lines = Vec::new();

    if env_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&env_path) {
            lines = content.lines().map(|s| s.to_string()).collect();
        }
    }

    let key = if is_dev { "APP_ENV_DEV" } else { "APP_ENV_PROD" };
    let new_line = format!("{}={}", key, new_mode);
    let mut found = false;

    for line in &mut lines {
        if line.trim().starts_with(&format!("{}=", key)) {
            *line = new_line.clone();
            found = true;
            break;
        }
    }

    if !found {
        lines.push(new_line);
    }

    std::fs::write(&env_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok(())
}

// Charge les modes depuis le fichier ModeExe.toml
fn load_modes_from_file(app_data_dir: &Path) -> Result<ModeConfig, String> {
    let toml_path = app_data_dir.join("ModeExe.toml");
    if !toml_path.exists() {
        // Créer les dossiers parents s'ils n'existent pas
        std::fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;

        // S'assurer que le dossier OPE existe
        let ope_dir = app_data_dir.join("OPE");
        if !ope_dir.exists() {
            std::fs::create_dir_all(&ope_dir).map_err(|e| e.to_string())?;
        }

        let initial_config = ModeConfig {
            modes: vec![ModeInfo {
                nom: "OPE".to_string(),
                descrition: "Mode opérationnel par défaut".to_string(),
                creation: "2026-05-15 15:12:28".to_string(),
                revision: None,
            }],
        };

        let toml_str = toml::to_string(&initial_config).map_err(|e| e.to_string())?;
        std::fs::write(&toml_path, toml_str).map_err(|e| e.to_string())?;
        return Ok(initial_config);
    }

    let content = std::fs::read_to_string(&toml_path).map_err(|e| e.to_string())?;
    let config: ModeConfig = toml::from_str(&content).map_err(|e| e.to_string())?;
    Ok(config)
}

// Sauvegarde les modes dans le fichier ModeExe.toml
fn save_modes_to_file(app_data_dir: &Path, config: &ModeConfig) -> Result<(), String> {
    let toml_path = app_data_dir.join("ModeExe.toml");
    let toml_str = toml::to_string(config).map_err(|e| e.to_string())?;
    std::fs::write(&toml_path, toml_str).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_execution_env(app: tauri::AppHandle) -> Result<ExecutionEnv, String> {
    let is_dev = cfg!(debug_assertions);
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let active_mode_dev = read_active_mode(&app_data_dir, true);
    let active_mode_prod = read_active_mode(&app_data_dir, false);
    Ok(ExecutionEnv { is_dev, active_mode_dev, active_mode_prod })
}

#[tauri::command]
pub async fn get_modes(app: tauri::AppHandle) -> Result<Vec<ModeInfo>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config = load_modes_from_file(&app_data_dir)?;
    Ok(config.modes)
}

#[tauri::command]
pub async fn create_mode(app: tauri::AppHandle, nom: String, descrition: String) -> Result<(), String> {
    println!("create_mode: {}", nom);
    if !nom.starts_with("EVAL_") {
        return Err("Le nom du mode doit commencer par 'EVAL_'".to_string());
    }

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut config = load_modes_from_file(&app_data_dir)?;

    if config.modes.iter().any(|m| m.nom == nom) {
        return Err("Un mode d'exécution avec ce nom existe déjà".to_string());
    }

    // Création du répertoire privé
    let mode_dir = app_data_dir.join(&nom);
    if !mode_dir.exists() {
        std::fs::create_dir_all(&mode_dir).map_err(|e| e.to_string())?;
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    config.modes.push(ModeInfo {
        nom,
        descrition,
        creation: now,
        revision: None,
    });

    save_modes_to_file(&app_data_dir, &config)?;
    Ok(())
}

#[tauri::command]
pub async fn update_mode(
    app: tauri::AppHandle,
    old_nom: String,
    new_nom: String,
    descrition: String,
) -> Result<(), String> {
    if old_nom == "OPE" {
        return Err("Le mode OPE ne peut pas être modifié".to_string());
    }
    if !new_nom.starts_with("EVAL_") {
        return Err("Le nom du mode doit commencer par 'EVAL_'".to_string());
    }

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut config = load_modes_from_file(&app_data_dir)?;

    let idx = config
        .modes
        .iter()
        .position(|m| m.nom == old_nom)
        .ok_or_else(|| "Mode d'exécution introuvable".to_string())?;

    if old_nom != new_nom {
        if config.modes.iter().any(|m| m.nom == new_nom) {
            return Err("Un mode d'exécution avec ce nouveau nom existe déjà".to_string());
        }

        // Renommer le dossier physique
        let old_dir = app_data_dir.join(&old_nom);
        let new_dir = app_data_dir.join(&new_nom);
        if old_dir.exists() {
            std::fs::rename(&old_dir, &new_dir).map_err(|e| e.to_string())?;
        } else {
            std::fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
        }

        // Si le mode modifié était le mode actif, mettre à jour .env
        let is_dev = cfg!(debug_assertions);
        let active_mode = read_active_mode(&app_data_dir, is_dev);
        if active_mode == old_nom {
            write_active_mode(&app_data_dir, is_dev, &new_nom)?;
        }
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    config.modes[idx].nom = new_nom;
    config.modes[idx].descrition = descrition;
    config.modes[idx].revision = Some(now);

    save_modes_to_file(&app_data_dir, &config)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_mode(app: tauri::AppHandle, nom: String) -> Result<(), String> {
    println!("delete_mode: {}", nom);
    if nom == "OPE" {
        return Err("Le mode OPE ne peut pas être supprimé".to_string());
    }

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // Interdire la suppression du mode actif
    let is_dev = cfg!(debug_assertions);
    let active_mode = read_active_mode(&app_data_dir, is_dev);
    if active_mode == nom {
        return Err("Impossible de supprimer le mode d'exécution actif".to_string());
    }

    let mut config = load_modes_from_file(&app_data_dir)?;
    let idx = config
        .modes
        .iter()
        .position(|m| m.nom == nom)
        .ok_or_else(|| "Mode d'exécution introuvable".to_string())?;

    config.modes.remove(idx);

    // Supprimer le dossier physique
    let mode_dir = app_data_dir.join(&nom);
    if mode_dir.exists() {
        std::fs::remove_dir_all(&mode_dir).map_err(|e| e.to_string())?;
    }

    save_modes_to_file(&app_data_dir, &config)?;
    Ok(())
}

#[tauri::command]
#[allow(unreachable_code)]
pub async fn select_mode(app: tauri::AppHandle, nom: String) -> Result<(), String> {
    println!("select_mode: {}", nom);
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config = load_modes_from_file(&app_data_dir)?;
    if !config.modes.iter().any(|m| m.nom == nom) {
        return Err("Mode d'exécution inconnu".to_string());
    }

    let is_dev = cfg!(debug_assertions);
    write_active_mode(&app_data_dir, is_dev, &nom)?;

    // Redémarrage de l'application en mode BUILD, sinon arret
    //app.restart();
    //Ok(())
    if is_dev {
        // En DEV : on arrête juste le processus
        std::process::exit(0);
    } else {
        // En PROD (build) : on utilise le restart propre
        app.restart();
        Ok(())
}
}
