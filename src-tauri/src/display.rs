use serde::Serialize;
use std::env;
use tauri::Manager;

#[derive(Serialize, Clone, Debug)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32), // coordonnées physiques (pixels)
    pub size: (u32, u32),     // taille physique (pixels)
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

#[cfg(not(target_os = "macos"))]
pub fn get_all_monitors(window: &tauri::Window) -> Vec<MonitorInfo> {
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

#[tauri::command]
pub fn get_displays(_window: tauri::Window) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }
    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(&_window)
    }
}

// ---------- Chargement de .env.local ----------
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

// ---------- Place une fenêtre en plein écran sur un moniteur donné (coordonnées physiques) ----------
fn set_window_fullscreen_on_monitor(
    window: &tauri::WebviewWindow,
    monitor: &MonitorInfo,
) -> Result<(), String> {
    // Utilisation directe des coordonnées et tailles physiques (pixels)
    window
        .set_position(tauri::PhysicalPosition::new(
            monitor.position.0 as f64,
            monitor.position.1 as f64,
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::PhysicalSize::new(
            monitor.size.0 as f64,
            monitor.size.1 as f64,
        ))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- Commandes Tauri ----------
#[tauri::command]
pub async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    load_env(&app);

    let primary_criteria =
        env::var("PRIMARY_MONITOR_CRITERIA").unwrap_or_else(|_| "origin".to_string());
    let secondary_criteria =
        env::var("SECONDARY_MONITOR_CRITERIA").unwrap_or_else(|_| "other".to_string());
    println!("[DEBUG] PRIMARY_MONITOR_CRITERIA = {}", primary_criteria);
    println!(
        "[DEBUG] SECONDARY_MONITOR_CRITERIA = {}",
        secondary_criteria
    );

    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;
    let main_window = app
        .get_webview_window("main")
        .ok_or("Fenêtre main introuvable")?;

    let monitors = {
        #[cfg(target_os = "macos")]
        {
            get_all_monitors()
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(monitors) = main_window.available_monitors() {
                monitors
                    .into_iter()
                    .map(|monitor| {
                        let name = monitor
                            .name()
                            .map_or_else(|| "(sans nom)".to_string(), |s| s.to_string());
                        let is_primary = monitor.position().x == 0 && monitor.position().y == 0;
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
    };

    if monitors.is_empty() {
        return Err("Aucun écran détecté".into());
    }

    println!("[DEBUG] Nombre d'écrans détectés : {}", monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        println!(
            "[DEBUG] Écran {} : nom={:?}, position physique=({},{}), taille physique=({}x{}), scale={}",
            i, m.name, m.position.0, m.position.1, m.size.0, m.size.1, m.scale_factor
        );
    }

    // Fenêtre principale : repositionner sur son écran cible (selon PRIMARY)
    if let Some(main_target) = find_monitor_by_criteria(&monitors, &primary_criteria) {
        println!(
            "[DEBUG] Fenêtre principale cible : position ({},{}), taille ({},{})",
            main_target.position.0, main_target.position.1, main_target.size.0, main_target.size.1
        );
        let _ = set_window_fullscreen_on_monitor(&main_window, main_target);
        let _ = main_window.maximize(); // pour s'assurer qu'elle est bien maximisée
    }

    // Fenêtre secondaire
    let sec_target = find_monitor_by_criteria(&monitors, &secondary_criteria).ok_or_else(|| {
        format!(
            "Aucun écran trouvé pour le critère '{}'",
            secondary_criteria
        )
    })?;
    println!(
        "[DEBUG] Fenêtre secondaire cible : position ({},{}), taille ({},{})",
        sec_target.position.0, sec_target.position.1, sec_target.size.0, sec_target.size.1
    );

    let _ = screen_bis.hide();
    set_window_fullscreen_on_monitor(&screen_bis, sec_target)?;

    // Pause pour que le système prenne en compte les changements
    tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_millis(150));
    })
    .await
    .map_err(|_| "Erreur lors de la pause")?;

    let _ = screen_bis.show();
    let _ = screen_bis.set_focus();

    if let Ok(pos_after) = screen_bis.outer_position() {
        println!(
            "[DEBUG] Après show, position réelle de screen-bis : {:?}",
            pos_after
        );
    }
    if let Ok(size_after) = screen_bis.outer_size() {
        println!(
            "[DEBUG] Après show, taille réelle de screen-bis : {:?}",
            size_after
        );
    }

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
    load_env(&app.handle());

    let primary_criteria =
        env::var("PRIMARY_MONITOR_CRITERIA").unwrap_or_else(|_| "origin".to_string());
    println!(
        "[DEBUG setup_display] PRIMARY_MONITOR_CRITERIA = {}",
        primary_criteria
    );

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

        if let Some(target) = find_monitor_by_criteria(&monitors, &primary_criteria) {
            println!(
                "[DEBUG setup_display] Fenêtre principale sur écran : position ({},{}), taille ({},{})",
                target.position.0, target.position.1, target.size.0, target.size.1
            );
            let _ = set_window_fullscreen_on_monitor(&window, target);
            let _ = window.maximize();
        } else {
            println!("[DEBUG setup_display] Aucun écran trouvé, fallback maximize");
            let _ = window.maximize();
        }
    }

    // Interception fermeture screen-bis
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
