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

// --- Commandes d'ouverture de fenêtre déplacée ici ---

#[tauri::command]
pub async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
    // screen-bis est déclarée dans tauri.conf.json avec visible: false
    // On la place sur l'écran OPPOSÉ à celui où se trouve actuellement main
    let screen_bis = app
        .get_webview_window("screen-bis")
        .ok_or("Fenêtre screen-bis introuvable")?;

    let main_window = app
        .get_webview_window("main")
        .ok_or("Fenêtre main introuvable")?;

    // Écran actuel de la fenêtre principale (pas forcément le primaire OS)
    let main_monitor = main_window.current_monitor().ok().flatten();

    let monitors = screen_bis.available_monitors().map_err(|e| e.to_string())?;
    println!("[debug] Nombre de moniteurs disponibles : {}", monitors.len());
    for (i, m) in monitors.iter().enumerate() {
        let name = m.name().map(|n| n.as_str()).unwrap_or("<sans_nom>");
        println!("[debug] Moniteur {}: nom={}, pos=({},{}) taille=({}x{})", i, name, m.position().x, m.position().y, m.size().width, m.size().height);
    }
    // Trouver l'écran différent de celui de main
    let other = monitors.iter().find(|m| match &main_monitor {
        Some(current) => m.position() != current.position(),
        None => true,
    });
    println!("[debug] Main monitor position: {:?}", main_monitor.as_ref().map(|m| (m.position().x, m.position().y)));
    // Hide the window before repositioning to ensure macOS respects the new location
    let _ = screen_bis.hide();
    if let Some(monitor) = other {
        // Les positions retournées par macOS sont en coordonnées logiques (points),
        // donc on utilise LogicalPosition au lieu de PhysicalPosition
        let pos = monitor.position();
        println!("[debug] Positionnement en LogicalPosition({}, {})", pos.x, pos.y);
        let _ = screen_bis.set_position(tauri::LogicalPosition::new(pos.x as f64, pos.y as f64));
    }
    // Show the secondary window, maximize it on the target monitor, and give it focus
    let _ = screen_bis.show();
    if other.is_some() {
        let _ = screen_bis.maximize();
    }
    let _ = screen_bis.set_focus();
    println!("[debug] ScreenBis affichée et focus appliqué");
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

// --- Détecter l'écran actif sous Windows ---

#[cfg(target_os = "windows")]
fn get_active_monitor_rect() -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        let hwnd = GetForegroundWindow();
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let r = info.rcWork;
            Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
        } else {
            None
        }
    }
}

// --- Configuration des affichages au démarrage de l'app ---

pub fn setup_display(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Positionnement/Maximisation de la fenêtre principale
    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window("main") {
            if let Some((x, y, w, h)) = get_active_monitor_rect() {
                let _ = window.set_position(tauri::PhysicalPosition::new(x + w / 2 - 400, y + h / 2 - 300));
                let _ = window.maximize();
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.maximize();
        }
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
