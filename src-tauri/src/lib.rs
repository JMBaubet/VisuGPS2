mod display;

use tauri::Manager;
use display::get_displays;

// --- Windows : détecte l'écran actif via Win32 avant que notre fenêtre prenne le focus ---

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

// --- Commandes Tauri ---

#[tauri::command]
async fn open_second_window(app: tauri::AppHandle) -> Result<(), String> {
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

    // Trouver l'écran différent de celui de main
    let other = monitors.iter().find(|m| match &main_monitor {
        Some(current) => m.position() != current.position(),
        None => true,
    });

    if let Some(monitor) = other {
        let pos = monitor.position();
        let _ = screen_bis.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
    }

    let _ = screen_bis.show();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Sur Windows : positionner la fenêtre principale sur l'écran actif
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    if let Some((x, y, w, h)) = get_active_monitor_rect() {
                        let _ = window.set_position(tauri::PhysicalPosition::new(x + w / 2 - 400, y + h / 2 - 300));
                        let _ = window.maximize();
                    }
                }
            }
            // Sur macOS : maximiser uniquement la fenêtre principale
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.maximize();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_displays, open_second_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
