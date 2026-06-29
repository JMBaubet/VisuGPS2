use serde::Serialize;
use tauri::Manager;

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

// -----------------------------------------------------------------------------
// macOS : récupération des écrans via NSScreen
// -----------------------------------------------------------------------------
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
                let is_builtin =
                    name_string.contains("Built-in") || name_string.contains("Interne");
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

// -----------------------------------------------------------------------------
// Windows et autres : récupération via Tauri
// -----------------------------------------------------------------------------
#[cfg(not(target_os = "macos"))]
pub fn get_all_monitors(window: &tauri::WebviewWindow) -> Vec<MonitorInfo> {
    if let Ok(monitors) = window.available_monitors() {
        monitors
            .into_iter()
            .map(|monitor| {
                let is_primary = monitor.position().x == 0 && monitor.position().y == 0;
                let name = monitor
                    .name()
                    .map_or_else(|| "(sans nom)".to_string(), |s| s.to_string());
                MonitorInfo {
                    name: Some(name),
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

// Commande Tauri pour obtenir les infos écrans
#[tauri::command]
pub fn get_displays(_window: tauri::WebviewWindow) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }
    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(&_window)
    }
}

// -----------------------------------------------------------------------------
// Chargement de .env.local (ressource externe)
// -----------------------------------------------------------------------------
fn load_env(app_handle: &tauri::AppHandle) {
    let path = if cfg!(debug_assertions) {
        std::env::current_dir().ok().map(|p| p.join(".env.local"))
    } else {
        app_handle
            .path()
            .resolve(".env.local", tauri::path::BaseDirectory::Resource)
            .ok()
    };

    if let Some(path) = path {
        if path.exists() {
            let _ = dotenvy::from_path(&path);
            println!("[DEBUG] .env.local chargé depuis {:?}", path);
        } else {
            println!("[DEBUG] .env.local non trouvé à {:?}", path);
        }
    } else {
        println!("[DEBUG] Impossible de déterminer le chemin pour .env.local");
    }
}

// -----------------------------------------------------------------------------
// Sélection d'un écran par critère textuel
// -----------------------------------------------------------------------------
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

fn find_monitor_by_name<'a>(monitors: &'a [MonitorInfo], name: &str) -> Option<&'a MonitorInfo> {
    monitors.iter().find(|m| {
        m.name.as_deref()
            .map(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
}

pub fn resolve_monitor_selection<'a>(selection: &str, monitors: &'a [MonitorInfo]) -> &'a MonitorInfo {
    let known_criteria = ["origin", "other", "builtin", "external"];
    let found = if known_criteria.contains(&selection) {
        find_monitor_by_criteria(monitors, selection)
    } else {
        find_monitor_by_name(monitors, selection)
    };
    found.unwrap_or_else(|| {
        eprintln!("[warning] Écran '{}' introuvable, fallback sur origin", selection);
        monitors.iter().find(|m| m.position == (0, 0)).unwrap_or_else(|| &monitors[0])
    })
}

// -----------------------------------------------------------------------------
// Placement d'une fenêtre (standard) : position + maximize
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn place_window_on_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    selection: &str,
) -> Result<(), String> {
    let target = resolve_monitor_selection(selection, monitors);
    let pos = target.position;
    window
        .set_position(tauri::LogicalPosition::new(pos.0 as f64, pos.1 as f64))
        .map_err(|e| e.to_string())?;
    window.maximize().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn place_window_on_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    selection: &str,
) -> Result<(), String> {
    let target = resolve_monitor_selection(selection, monitors);
    window
        .set_position(tauri::PhysicalPosition::new(
            target.position.0 as f64,
            target.position.1 as f64,
        ))
        .map_err(|e| e.to_string())?;
    window.maximize().map_err(|e| e.to_string())?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Placement d'une fenêtre en plein écran (spécifique pour screen-bis)
// -----------------------------------------------------------------------------
#[cfg(target_os = "macos")]
fn place_window_on_monitor_fullscreen(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    selection: &str,
) -> Result<(), String> {
    let target = resolve_monitor_selection(selection, monitors);
    let pos = target.position;
    // Déplacer sur l'écran cible
    window
        .set_position(tauri::LogicalPosition::new(pos.0 as f64, pos.1 as f64))
        .map_err(|e| e.to_string())?;
    // Passer en plein écran (masque la barre de menu)
    window.set_fullscreen(true).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn place_window_on_monitor_fullscreen(
    window: &tauri::WebviewWindow,
    monitors: &[MonitorInfo],
    selection: &str,
) -> Result<(), String> {
    // Sur Windows, on utilise maximize (comportement équivalent sans masquer la barre des tâches)
    let target = resolve_monitor_selection(selection, monitors);
    window
        .set_position(tauri::PhysicalPosition::new(
            target.position.0 as f64,
            target.position.1 as f64,
        ))
        .map_err(|e| e.to_string())?;
    window.maximize().map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Commandes Tauri
// -----------------------------------------------------------------------------
#[tauri::command]
pub async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    load_env(&app);

    let secondary_criteria = if let Some(state) = app.try_state::<std::sync::Arc<tokio::sync::RwLock<crate::settings::SettingsState>>>() {
        let state_read = state.read().await;
        crate::settings::get_toml_value_by_path(&state_read.user_overrides, "Affichage.moniteurs.secondaire")
            .or_else(|| crate::settings::get_toml_value_by_path(&state_read.default_toml, "Affichage.moniteurs.secondaire"))
            .and_then(|v| v.as_str())
            .unwrap_or("other")
            .to_string()
    } else {
        "other".to_string()
    };

    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;

    // Récupération de la liste des écrans
    let monitors = {
        #[cfg(target_os = "macos")]
        {
            get_all_monitors()
        }
        #[cfg(not(target_os = "macos"))]
        {
            let main_window = app
                .get_webview_window("main")
                .ok_or("Fenêtre main introuvable")?;
            get_all_monitors(&main_window)
        }
    };

    if monitors.is_empty() {
        return Err("Aucun écran détecté".into());
    }

    // Sur macOS, on ne touche PAS à la fenêtre principale
    // Sur Windows, on la repositionne selon PRIMARY_MONITOR_CRITERIA (commenté)
    #[cfg(target_os = "macos")]
    {
        // rien
    }
    #[cfg(not(target_os = "macos"))]
    {
        // (code commenté conservé)
    }

    // Placement de la fenêtre secondaire en plein écran (fullscreen sur macOS, maximize sur Windows)
    let _ = screen_bis.hide();
    if let Err(e) = place_window_on_monitor_fullscreen(&screen_bis, &monitors, &secondary_criteria)
    {
        eprintln!("[warning] Placement secondaire: {}", e);
        if let Err(e2) = place_window_on_monitor_fullscreen(&screen_bis, &monitors, "other") {
            return Err(format!("Échec du placement de screen-bis: {}", e2));
        }
    }

    tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_millis(100));
    })
    .await
    .map_err(|_| "Erreur lors de la pause")?;

    let _ = screen_bis.show();
    let _ = screen_bis.set_focus();

    Ok(())
}

#[tauri::command]
pub async fn close_second_window(app: tauri::AppHandle) -> Result<(), String> {
    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;

    // Quitter le plein écran pour détruire l'espace dédié
    screen_bis
        .set_fullscreen(false)
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        // Attendre que macOS ait terminé la transition (1500 ms sont nécessaires sous macOS)
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    }

    screen_bis.hide().map_err(|e| e.to_string())?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Configuration au démarrage de l'application
// -----------------------------------------------------------------------------
pub fn setup_display(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    load_env(&app.handle());

    let primary_criteria = if let Some(state) = app.handle().try_state::<std::sync::Arc<tokio::sync::RwLock<crate::settings::SettingsState>>>() {
        if let Ok(state_read) = state.try_read() {
            crate::settings::get_toml_value_by_path(&state_read.user_overrides, "Affichage.moniteurs.principal")
                .or_else(|| crate::settings::get_toml_value_by_path(&state_read.default_toml, "Affichage.moniteurs.principal"))
                .and_then(|v| v.as_str())
                .unwrap_or("origin")
                .to_string()
        } else {
            "origin".to_string()
        }
    } else {
        "origin".to_string()
    };

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
                        let name = mon
                            .name()
                            .map_or_else(|| "(sans nom)".to_string(), |s| s.to_string());
                        let is_primary = mon.position().x == 0 && mon.position().y == 0;
                        vec.push(MonitorInfo {
                            name: Some(name),
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
            let _ = window.maximize();
        }
    }

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
