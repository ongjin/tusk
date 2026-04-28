use tauri::Manager;

mod commands;
pub mod db;
pub mod errors;
pub mod secrets;
pub mod ssh;

use crate::db::pool::ConnectionRegistry;
use crate::db::state::StateStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("app_data_dir unavailable");
            std::fs::create_dir_all(&app_data).ok();
            let store =
                StateStore::open(app_data.join("tusk.db")).expect("failed to open state store");
            app.manage(store);
            app.manage(ConnectionRegistry::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::meta::greet,
            commands::connections::list_connections,
            commands::connections::add_connection,
            commands::connections::delete_connection,
            commands::connections::connect,
            commands::connections::disconnect,
            commands::query::execute_query,
            commands::ssh::list_known_ssh_hosts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
