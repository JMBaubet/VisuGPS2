mod display;
#[allow(non_snake_case)]
mod gestionMode;

use display::{get_displays, open_second_window, close_second_window};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialisation de la logique des fenêtres et des affichages
            display::setup_display(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_displays,
            open_second_window,
            close_second_window,
            gestionMode::get_execution_env,
            gestionMode::get_modes,
            gestionMode::create_mode,
            gestionMode::update_mode,
            gestionMode::delete_mode,
            gestionMode::select_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
