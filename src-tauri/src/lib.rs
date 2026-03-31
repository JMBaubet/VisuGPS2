use serde::Serialize;
use std::ffi::CStr;

#[derive(Serialize)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub scale_factor: f64,
}

#[cfg(target_os = "macos")]
fn get_all_monitors() -> Vec<MonitorInfo> {
    use objc::{class, msg_send, sel, sel_impl};
    use objc::runtime::Object;

    unsafe {
        let screen_class = class!(NSScreen);
        let screens: *mut Object = msg_send![screen_class, screens];
        let count: usize = msg_send![screens, count];

        let mut displays = Vec::new();

        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex:i];

            // Structures pour CGRect et CGPoint/CGSize
            #[repr(C)]
            struct CGPoint {
                x: f64,
                y: f64,
            }

            #[repr(C)]
            struct CGSize {
                width: f64,
                height: f64,
            }

            #[repr(C)]
            struct CGRect {
                origin: CGPoint,
                size: CGSize,
            }

            // Obtenir le frame (position et taille)
            let frame: CGRect = msg_send![screen, frame];

            // Obtenir le facteur d'échelle
            let scale: f64 = msg_send![screen, backingScaleFactor];

            // Obtenir le nom localisé de l'écran
            let name: *mut Object = msg_send![screen, localizedName];
            let display_name: Option<String> = if !name.is_null() {
                let string_ptr: *const u8 = msg_send![name, UTF8String];
                if !string_ptr.is_null() {
                    Some(CStr::from_ptr(string_ptr as *const i8).to_string_lossy().into_owned())
                } else {
                    None
                }
            } else {
                None
            };

            displays.push(MonitorInfo {
                name: display_name,
                position: (frame.origin.x as i32, frame.origin.y as i32),
                size: (frame.size.width as u32, frame.size.height as u32),
                scale_factor: scale,
            });
        }

        displays
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
fn get_displays(window: tauri::Window) -> Vec<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        get_all_monitors()
    }

    #[cfg(not(target_os = "macos"))]
    {
        get_all_monitors(window)
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
                    let _ = window.set_maximized(true);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
