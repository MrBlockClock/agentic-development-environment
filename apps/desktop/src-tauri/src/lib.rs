#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ade_desktop_crate::commands::AppState::discover())
        .invoke_handler(tauri::generate_handler![
            ade_desktop_crate::commands::get_dashboard,
            ade_desktop_crate::commands::run_audit,
            ade_desktop_crate::commands::run_plan,
            ade_desktop_crate::commands::run_execute,
            ade_desktop_crate::commands::run_verify,
            ade_desktop_crate::commands::mcp_connect,
            ade_desktop_crate::commands::mcp_list_servers,
            ade_desktop_crate::commands::mcp_list_tools,
            ade_desktop_crate::commands::mcp_disconnect,
            ade_desktop_crate::commands::mcp_call_tool,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
