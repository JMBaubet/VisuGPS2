use serde::Serialize;

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
