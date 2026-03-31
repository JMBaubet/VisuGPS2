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

#[cfg(not(target_os = "macos"))]
fn get_active_monitor(window: tauri::Window) -> Option<MonitorInfo> {
    if let Ok(monitors) = window.available_monitors() {
        // Obtenir la position du curseur pour déterminer le moniteur actif
        if let Ok(cursor) = window.cursor_position() {
            let x = cursor.x as i32;
            let y = cursor.y as i32;

            // Retourner le moniteur contenant la position du curseur
            monitors.into_iter().find_map(|monitor| {
                let pos = monitor.position();
                let size = monitor.size();

                if x >= pos.x && x < pos.x + size.width as i32
                    && y >= pos.y && y < pos.y + size.height as i32
                {
                    Some(MonitorInfo {
                        name: monitor.name().map(|s| s.to_string()),
                        position: (pos.x, pos.y),
                        size: (size.width, size.height),
                        scale_factor: monitor.scale_factor(),
                    })
                } else {
                    None
                }
            })
        } else {
            // Fallback sur le premier moniteur si on ne peut pas récupérer la position du curseur
            monitors.into_iter().next().map(|monitor| {
                let pos = monitor.position();
                let size = monitor.size();
                MonitorInfo {
                    name: monitor.name().map(|s| s.to_string()),
                    position: (pos.x, pos.y),
                    size: (size.width, size.height),
                    scale_factor: monitor.scale_factor(),
                }
            })
        }
    } else {
        None
    }
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

#[tauri::command]
fn get_active_display(window: tauri::Window) -> Option<MonitorInfo> {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};
        use objc::runtime::Object;

        unsafe {
            let screen_class = class!(NSScreen);
            let main_screen: *mut Object = msg_send![screen_class, mainScreen];

            if !main_screen.is_null() {
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

                let frame: CGRect = msg_send![main_screen, frame];
                let scale: f64 = msg_send![main_screen, backingScaleFactor];

                let name: *mut Object = msg_send![main_screen, localizedName];
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

                return Some(MonitorInfo {
                    name: display_name,
                    position: (frame.origin.x as i32, frame.origin.y as i32),
                    size: (frame.size.width as u32, frame.size.height as u32),
                    scale_factor: scale,
                });
            }
        }
        None
    }

    #[cfg(not(target_os = "macos"))]
    {
        get_active_monitor(window)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_displays, get_active_display])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
