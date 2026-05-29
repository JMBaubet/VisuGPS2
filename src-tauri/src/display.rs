use serde::Serialize;
use tauri::Manager;
use std::env;

#[derive(Serialize, Clone, Debug)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
    #[cfg(target_os = "macos")]
    pub is_builtin: bool,
    #[cfg(not(target_os = "macos"))]
    pub is_primary: bool,
}

// ---------- Récupération de la liste des écrans ----------

#[cfg(target_os = "macos")]
pub fn get_all_monitors() -> Vec<MonitorInfo> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    unsafe {
        let mtm = MainThreadMarker::new_unchecked();
        let screens = NSScreen::screens(mtm);
        screens
            .iter()
            .map(|screen| {
                let frame = screen.frame();
                let scale = screen.backingScaleFactor();
                let name_ns = screen.localizedName();
                let name_string = name_ns.to_string();
                let is_builtin = name_string.contains("Built-in") || name_string.contains("Interne");
                MonitorInfo {
                    name: Some(name_string),
                    position: (frame.origin.x as i32, frame.origin.y as i32),
                    size: (frame.size.width as u32, frame.size.height as u32),
                    scale_factor: scale,
                    is_builtin,
                }
            })
            .collect()
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_all_monitors(window: &tauri::Window) -> Vec<MonitorInfo> {
    if let Ok(monitors) = window.available_monitors() {
        monitors
            .into_iter()
            .map(|monitor| {
                let is_primary = monitor.position().x == 0 && monitor.position().y == 0;
                MonitorInfo {
                    name: monitor.name().map(|s| s.to_string()),
                    position: (monitor.position().x, monitor.position().y),
                    size: (monitor.size().width, monitor.size().height),
                    scale_factor: monitor.scale_factor(),
                    is_primary,
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub fn get_displays(_window: tauri::Window) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }
    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(&window)
    }
}

// ---------- Chargement de .env.local (ressource externe) ----------

/// Charge le fichier .env.local depuis le dossier des ressources (production)
/// ou depuis le dossier courant (développement).
fn load_env(app_handle: &tauri::AppHandle) {
    let path = if cfg!(debug_assertions) {
        // En développement : on cherche à la racine du projet
        std::env::current_dir().ok().map(|p| p.join(".env.local"))
    } else {
        // En production : on utilise le dossier des ressources
        app_handle
            .path()
            .resolve(".env.local", tauri::path::BaseDirectory::Resource)
            .ok()
    };

    if let Some(path) = path {
        if path.exists() {
            let _ = dotenvy::from_path(&path);
            eprintln!("[debug] .env.local chargé depuis {:?}", path);
        } else {
            eprintln!("[debug] .env.local non trouvé à {:?}", path);
        }
    } else {
        eprintln!("[debug] Impossible de déterminer le chemin pour .env.local");
    }
}

// ---------- Sélection d'un écran par critère ----------

fn find_monitor_by_criteria<'a>(
    monitors: &'a [MonitorInfo],
    criteria: &str,
) -> Option<&'a MonitorInfo> {
    match criteria {
        "origin" => monitors.iter().find(|m| m.position == (0, 0)),
        "other" => monitors.iter().find(|m| m.position != (0, 0)),
        "builtin" => {
            #[cfg(target_os = "macos")]
            {
                monitors.iter().find(|m| m.is_builtin)
            }
            #[cfg(not(target_os = "macos"))]
            {
                monitors.iter().find(|m| m.position == (0, 0))
            }
        }
        "external" => {
            #[cfg(target_os = "macos")]
            {
                monitors.iter().find(|m| !m.is_builtin)
            }
            #[cfg(not(target_os = "macos"))]
            {
                monitors.iter().find(|m| m.position != (0, 0))
            }
        }
        _ => None,
    }
}

// Positionne une fenêtre sur l'écran cible (sans maximise)
fn set_window_position(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    criteria: &str,
) -> Result<(), String> {
    let target = find_monitor_by_criteria(monitors, criteria)
        .ok_or_else(|| format!("Aucun écran trouvé pour le critère '{}'", criteria))?;
    let pos = target.position;
    window
        .set_position(tauri::LogicalPosition::new(pos.0 as f64, pos.1 as f64))
        .map_err(|e| e.to_string())
}

// Place et maximise une fenêtre (utilisé pour la fenêtre principale au démarrage)
fn place_window_on_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    criteria: &str,
) -> Result<(), String> {
    set_window_position(window, monitors, criteria)?;
    window.maximize().map_err(|e| e.to_string())
}

// ---------- Commandes Tauri ----------

#[tauri::command]
pub async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    load_env(&app); // on recharge les variables à chaque ouverture

    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;
    let _main_window = app
        .get_webview_window("main")
        .ok_or("Fenêtre main introuvable")?;

    // Récupération de la liste des écrans
    let monitors = {
        #[cfg(target_os = "macos")]
        {
            get_all_monitors()
        }
        #[cfg(not(target_os = "macos"))]
        {
            get_all_monitors(&main_window.clone().into())
        }
    };

    if monitors.is_empty() {
        return Err("Aucun écran détecté".into());
    }

    // On ne repositionne PAS la fenêtre principale ici (seulement au démarrage)
    // Lecture du critère pour la seconde fenêtre
    let secondary_criteria = env::var("SECONDARY_MONITOR_CRITERIA")
        .unwrap_or_else(|_| "other".to_string());

    // Masquer avant déplacement
    let _ = screen_bis.hide();

    // Positionner sans maximiser
    if let Err(e) = set_window_position(&screen_bis, &monitors, &secondary_criteria) {
        eprintln!("[warning] Placement secondaire: {}", e);
        if let Err(e2) = set_window_position(&screen_bis, &monitors, "other") {
            return Err(format!("Échec du placement: {}", e2));
        }
    }

    // Pause nécessaire sur macOS pour que la nouvelle position soit prise en compte
    // On utilise un sleep non-bloquant via spawn_blocking
    tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_millis(100));
    })
    .await
    .map_err(|_| "Erreur lors de la pause")?;

    // Afficher, maximiser et focus
    let _ = screen_bis.show();
    let _ = screen_bis.maximize();
    let _ = screen_bis.set_focus();

    Ok(())
}

#[tauri::command]
pub async fn close_second_window(app: tauri::AppHandle) -> Result<(), String> {
    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;
    let _ = screen_bis.hide();
    Ok(())
}

// ---------- Configuration au démarrage ----------

pub fn setup_display(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Charger .env.local au démarrage
    load_env(&app.handle());

    let primary_criteria = env::var("PRIMARY_MONITOR_CRITERIA")
        .unwrap_or_else(|_| "origin".to_string());

    if let Some(window) = app.get_webview_window("main") {
        let monitors = {
            #[cfg(target_os = "macos")]
            {
                get_all_monitors()
            }
            #[cfg(not(target_os = "macos"))]
            {
                let mut vec = Vec::new();
                if let Ok(monitors) = window.available_monitors() {
                    for mon in monitors {
                        let is_primary = mon.position().x == 0 && mon.position().y == 0;
                        vec.push(MonitorInfo {
                            name: mon.name().map(|s| s.to_string()),
                            position: (mon.position().x, mon.position().y),
                            size: (mon.size().width, mon.size().height),
                            scale_factor: mon.scale_factor(),
                            is_primary,
                        });
                    }
                }
                vec
            }
        };

        if let Err(e) = place_window_on_monitor(&window, &monitors, &primary_criteria) {
            eprintln!("[warning] Erreur placement fenêtre principale: {}", e);
            let _ = window.maximize(); // fallback
        }
    }

    // Interception de la fermeture de screen-bis pour la masquer
    if let Some(screen_bis) = app.get_webview_window("screen-bis") {
        let screen_bis_clone = screen_bis.clone();
        screen_bis.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = screen_bis_clone.hide();
                api.prevent_close();
            }
        });
    }

    Ok(())
}