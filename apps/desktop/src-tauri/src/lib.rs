#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ade_desktop_crate::commands::AppState::discover())
        .invoke_handler(tauri::generate_handler![
            ade_desktop_crate::commands::get_dashboard,
            ade_desktop_crate::commands::key_status,
            ade_desktop_crate::commands::key_set,
            ade_desktop_crate::commands::key_delete,
            ade_desktop_crate::commands::key_smoke,
            ade_desktop_crate::commands::key_live_smoke,
            ade_desktop_crate::commands::run_audit,
            ade_desktop_crate::commands::run_plan,
            ade_desktop_crate::commands::run_execute,
            ade_desktop_crate::commands::run_verify,
            ade_desktop_crate::commands::mcp_connect,
            ade_desktop_crate::commands::mcp_list_servers,
            ade_desktop_crate::commands::mcp_list_tools,
            ade_desktop_crate::commands::mcp_disconnect,
            ade_desktop_crate::commands::mcp_call_tool,
            ade_desktop_crate::commands::run_agent_turn,
            ade_desktop_crate::commands::list_recipes,
            ade_desktop_crate::commands::initialize_recipe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
