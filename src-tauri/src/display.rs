use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
}

// --- macOS : liste tous les écrans via NSScreen (objc2) ---

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
                let name = screen.localizedName();
                MonitorInfo {
                    name: Some(name.to_string()),
                    position: (frame.origin.x as i32, frame.origin.y as i32),
                    size: (frame.size.width as u32, frame.size.height as u32),
                    scale_factor: scale,
                }
            })
            .collect()
    }
}

// --- Windows et autres : liste tous les écrans via Tauri ---

#[cfg(not(target_os = "macos"))]
pub fn get_all_monitors(window: &tauri::Window) -> Vec<MonitorInfo> {
    if let Ok(monitors) = window.available_monitors() {
        monitors
            .into_iter()
            .map(|monitor| MonitorInfo {
                name: monitor.name().map(|s| s.to_string()),
                position: (monitor.position().x, monitor.position().y),
                size: (monitor.size().width, monitor.size().height),
                scale_factor: monitor.scale_factor(),
            })
            .collect()
    } else {
        Vec::new()
    }
}

// --- Commande Tauri pour obtenir les informations de displays ---

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

// --- Lecture de la configuration d'écran depuis .env.local ---

/// Lit les variables PRIMARY_DISPLAY et SECONDARY_DISPLAY depuis .env.local.
/// Retourne (primary_index, secondary_index) avec des valeurs par défaut (0, 1).
fn read_display_config() -> (usize, usize) {
    // Charger le fichier .env.local (situé à la racine du projet, un niveau au-dessus de src-tauri)
    // dotenvy::from_filename_override permet de charger sans erreur si le fichier n'existe pas
    let _ = dotenvy::from_filename_override(".env.local");

    let primary = std::env::var("PRIMARY_DISPLAY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let secondary = std::env::var("SECONDARY_DISPLAY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);

    println!("[display] Configuration lue : PRIMARY_DISPLAY={}, SECONDARY_DISPLAY={}", primary, secondary);
    (primary, secondary)
}

// --- Commandes d'ouverture de fenêtre ---

#[tauri::command]
pub async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    // screen-bis est déclarée dans tauri.conf.json avec visible: false
    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;

    let (_, secondary_index) = read_display_config();

    let monitors = screen_bis.available_monitors().map_err(|e| e.to_string())?;
    println!("[display] Nombre de moniteurs disponibles : {}", monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        let name = m.name().map(|n| n.as_str()).unwrap_or("<sans_nom>");
        println!("[display]   Moniteur {}: nom={}, pos=({},{}) taille=({}x{})",
            i, name, m.position().x, m.position().y, m.size().width, m.size().height);
    }

    // Sélectionner le moniteur par index configuré, ou le dernier disponible
    let target_index = if secondary_index < monitors.len() {
        secondary_index
    } else if monitors.len() > 1 {
        // Si l'index configuré dépasse le nombre de moniteurs, prendre le dernier
        println!("[display] SECONDARY_DISPLAY={} hors limites, utilisation du moniteur {}",
            secondary_index, monitors.len() - 1);
        monitors.len() - 1
    } else {
        println!("[display] Un seul moniteur disponible, screen-bis sur le moniteur 0");
        0
    };

    let target_monitor = &monitors[target_index];
    let pos = target_monitor.position();
    println!("[display] Positionnement de screen-bis sur moniteur {} à ({}, {})", target_index, pos.x, pos.y);

    // Hide the window before repositioning
    let _ = screen_bis.hide();
    let _ = screen_bis.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));

    // Show the secondary window, maximize it on the target monitor, and give it focus
    let _ = screen_bis.show();
    let _ = screen_bis.maximize();
    let _ = screen_bis.set_focus();
    println!("[display] screen-bis affichée et focus appliqué sur moniteur {}", target_index);
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

// --- Configuration des affichages au démarrage de l'app ---

pub fn setup_display(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (primary_index, _) = read_display_config();

    // 1. Positionnement/Maximisation de la fenêtre principale sur le moniteur configuré
    if let Some(window) = app.get_webview_window("main") {
        let monitors = window.available_monitors()?;
        println!("[display] setup_display – {} moniteur(s) détecté(s)", monitors.len());
        for (i, m) in monitors.iter().enumerate() {
            let name = m.name().map(|n| n.to_string()).unwrap_or_else(|| "<sans_nom>".to_string());
            println!("[display]   Moniteur {}: nom={}, pos=({},{}) taille=({}x{})",
                i, name, m.position().x, m.position().y, m.size().width, m.size().height);
        }

        let target_index = if primary_index < monitors.len() {
            primary_index
        } else {
            println!("[display] PRIMARY_DISPLAY={} hors limites, utilisation du moniteur 0", primary_index);
            0
        };

        let target = &monitors[target_index];
        let pos = target.position();
        let size = target.size();
        println!("[display] Fenêtre principale → moniteur {} à ({}, {}), taille {}x{}",
            target_index, pos.x, pos.y, size.width, size.height);

        // Positionner la fenêtre au centre du moniteur cible, puis maximiser
        let _ = window.set_position(tauri::PhysicalPosition::new(
            pos.x + (size.width as i32) / 2 - 400,
            pos.y + (size.height as i32) / 2 - 300,
        ));
        let _ = window.maximize();
    }

    // 2. Interception de l'événement de fermeture sur screen-bis pour la masquer au lieu de la détruire
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
