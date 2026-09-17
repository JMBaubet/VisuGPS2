mod display;
#[allow(non_snake_case)]
mod gestionMode;
mod settings;
mod import_gpx;
mod gpx_audit;
mod gpx_multiride;

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
            gpx_audit::commands::audit_run_detection,
            gpx_audit::commands::audit_map_overlay,
            gpx_audit::commands::audit_delete_preview,
            gpx_audit::commands::audit_routes_identical,
            gpx_audit::commands::audit_apply_delete,
            gpx_audit::commands::audit_apply_route,
            gpx_audit::commands::audit_mark_fp,
            gpx_audit::commands::audit_unmark_fp,
            gpx_audit::commands::audit_undo_correction,
            gpx_audit::commands::audit_validate,
            gpx_audit::commands::audit_save_state,
            gpx_audit::commands::audit_load_archive,
            gpx_multiride::commands::multiride_detect,
            gpx_multiride::commands::multiride_load,
            gpx_multiride::commands::multiride_validate,
            gpx_multiride::commands::multiride_merge_segment,
            gpx_multiride::commands::multiride_toggle_fp,
            gpx_multiride::commands::multiride_validate_segment,
            gpx_multiride::commands::multiride_undo_segment,
            gpx_multiride::commands::multiride_reset
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    /// Le catalogue des commandes documenté est celui réellement enregistré.
    ///
    /// `COMMANDS.md` se déclare sourcé sur ce fichier (« source de vérité :
    /// `src-tauri/src/lib.rs` ») : ce test l'y confronte, dans l'esprit du
    /// contrôle de cohérence du drawer (`settings::tests`). Sans lui, une
    /// commande ajoutée, renommée ou retirée peut passer inaperçue dans la
    /// documentation — or c'est la documentation que lit un nouvel arrivant, et
    /// la source de vérité déclarée est le code.
    #[test]
    fn the_command_catalogue_matches_the_invoke_handler() {
        // Commandes réellement enregistrées : dernier segment de chaque entrée
        // de l'`invoke_handler`.
        let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
            .expect("lib.rs doit être lisible");
        let handler = source
            .split("tauri::generate_handler![")
            .nth(1)
            .expect("invoke_handler déclaré")
            .split("])")
            .next()
            .expect("invoke_handler refermé");
        let mut registered: Vec<String> = handler
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.rsplit("::").next().unwrap_or(entry).to_string())
            .collect();
        registered.sort();

        // Commandes documentées : première colonne des tableaux de la section
        // « Catalogue », la seule qui prétende à l'exhaustivité.
        let doc = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/COMMANDS.md"
        ))
        .expect("docs/COMMANDS.md doit être lisible");
        let catalogue = doc
            .split("## Catalogue")
            .nth(1)
            .expect("section « Catalogue » déclarée")
            .split("\n## ")
            .next()
            .expect("section « Catalogue » refermée");
        let mut documented: Vec<String> = catalogue
            .lines()
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|rest| rest.split('`').next())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
        documented.sort();

        assert_eq!(
            documented, registered,
            "le catalogue de docs/COMMANDS.md ne correspond plus aux commandes enregistrées"
        );
    }
}
