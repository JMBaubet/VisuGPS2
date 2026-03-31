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
fn get_all_monitors() -> Vec<MonitorInfo> {
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
fn get_all_monitors(window: &tauri::Window) -> Vec<MonitorInfo> {
    if let Ok(monitors) = window.available_monitors() {
        monitors.into_iter().map(|monitor| MonitorInfo {
            name: monitor.name().map(|s| s.to_string()),
            position: (monitor.position().x, monitor.position().y),
            size: (monitor.size().width, monitor.size().height),
            scale_factor: monitor.scale_factor(),
        }).collect()
    } else {
        Vec::new()
    }
}

// --- Windows : détecte l'écran actif via Win32 avant que notre fenêtre prenne le focus ---
//
// GetForegroundWindow() retourne la fenêtre active au moment du lancement
// (ex: PowerShell). MonitorFromWindow() donne le moniteur de cette fenêtre.
// Doit être appelé dans le setup Tauri, AVANT que notre fenêtre soit visible.

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
            // rcWork exclut la barre des tâches
            let r = info.rcWork;
            Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
        } else {
            None
        }
    }
}

// --- Commandes Tauri ---

#[tauri::command]
fn get_displays(_window: tauri::Window) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }

    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(&_window)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Sur Windows : positionner la fenêtre sur l'écran actif AVANT qu'elle s'affiche.
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    if let Some((x, y, w, h)) = get_active_monitor_rect() {
                        let _ = window.set_position(tauri::PhysicalPosition::new(x + w / 2 - 400, y + h / 2 - 300));
                        let _ = window.maximize();
                    }
                }
            }
            // Sur macOS : maximiser la fenêtre après création
            #[cfg(target_os = "macos")]
            {
                for (_, window) in app.webview_windows().iter() {
                    let _ = window.maximize();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_displays])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
