mod display;
#[allow(non_snake_case)]
mod gestionMode;
mod settings;

use display::{get_displays, open_second_window, close_second_window};
use tauri::Manager;

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialisation du système de paramètres
            let settings_state = settings::init_settings_state(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            app.manage(settings_state);

            // Initialisation de la logique des fenêtres et des affichages
            display::setup_display(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            exit_app,
            get_displays,
            open_second_window,
            close_second_window,
            gestionMode::get_execution_env,
            gestionMode::get_modes,
            gestionMode::create_mode,
            gestionMode::update_mode,
            gestionMode::delete_mode,
            gestionMode::select_mode,
            settings::get_settings,
            settings::update_setting,
            settings::reset_setting,
            settings::get_setting_value
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
