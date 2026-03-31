use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
}

#[cfg(target_os = "macos")]
fn get_all_monitors() -> Vec<MonitorInfo> {
    use objc2_foundation::MainThreadMarker;
    use objc2_app_kit::NSScreen;

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

#[cfg(not(target_os = "macos"))]
fn get_all_monitors(window: tauri::Window) -> Vec<MonitorInfo> {
    let mut displays = Vec::new();

    if let Ok(Some(monitor)) = window.primary_monitor() {
        displays.push(MonitorInfo {
            name: monitor.name().map(|s| s.to_string()),
            position: (monitor.position().x, monitor.position().y),
            size: (monitor.size().width, monitor.size().height),
            scale_factor: monitor.scale_factor(),
        });
    }

    displays
}

#[tauri::command]
fn get_displays(_window: tauri::Window) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }

    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(_window)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_displays])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                for (_, window) in app.webview_windows().iter() {
                    let _ = window.maximize();
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
