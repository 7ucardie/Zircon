mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::scan_client_dir,
            commands::build_patches,
            commands::read_patch_config,
            commands::save_patch_config,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Zircon Admin");
}
