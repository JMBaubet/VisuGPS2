mod display;
#[allow(non_snake_case)]
mod gestionMode;
mod settings;
mod import_gpx;
mod cleaning;
mod gpx_audit;

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
        .plugin(tauri_plugin_dialog::init())
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
            settings::get_settings_meta,
            settings::update_setting,
            settings::reset_setting,
            settings::get_setting_value,
            import_gpx::import_gpx_file,
            import_gpx::get_traces,
            import_gpx::delete_trace,
            import_gpx::update_trace,
            import_gpx::get_trace_geometry,
            import_gpx::get_trace_points,
            import_gpx::save_keyframes,
            import_gpx::get_keyframes,
            import_gpx::delete_keyframes,
            cleaning::detect_trace_anomalies,
            cleaning::get_cleaning_state,
            cleaning::save_cleaning_state,
            cleaning::reset_cleaning,
            cleaning::validate_phase,
            gpx_audit::commands::audit_run_detection,
            gpx_audit::commands::audit_map_overlay,
            gpx_audit::commands::audit_delete_preview,
            gpx_audit::commands::audit_routes_identical,
            gpx_audit::commands::audit_apply_delete,
            gpx_audit::commands::audit_apply_route,
            gpx_audit::commands::audit_mark_fp,
            gpx_audit::commands::audit_unmark_fp,
            gpx_audit::commands::audit_undo_correction,
            gpx_audit::commands::audit_validate
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
