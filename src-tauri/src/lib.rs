use tauri::Manager;

pub mod commands;
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("app_data_dir unavailable");
            std::fs::create_dir_all(&app_data).ok();
            let store =
                StateStore::open(app_data.join("tusk.db")).expect("failed to open state store");
            app.manage(store);
            app.manage(ConnectionRegistry::new());
            app.manage(crate::db::pg_meta::MetaCache::new());
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
            commands::schema::list_databases,
            commands::schema::list_schemas,
            commands::schema::list_tables,
            commands::schema::list_columns,
            commands::history::list_history,
            commands::history::list_history_statements,
            commands::transactions::tx_begin,
            commands::transactions::tx_commit,
            commands::transactions::tx_rollback,
            commands::fk_lookup::fk_lookup,
            commands::editing::preview_pending_changes,
            commands::editing::submit_pending_changes,
            commands::cancel::cancel_query,
            commands::export::export_result,
            commands::ai_secrets::ai_secret_set,
            commands::ai_secrets::ai_secret_get,
            commands::ai_secrets::ai_secret_delete,
            commands::ai_secrets::ai_secret_list_present,
            commands::destructive::classify_destructive_sql,
            commands::schema_index::sync_schema_index,
            commands::schema_index::schema_index_clear,
            commands::schema_index::schema_index_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
