mod app_update;
mod commands;
mod domain;
mod push;
mod push_command;

const APP_ICON_BYTES: &[u8] = include_bytes!("../icons/icon.ico");

fn build_context() -> tauri::Context<tauri::Wry> {
    let mut context = tauri::generate_context!();
    let icon = tauri::image::Image::from_bytes(APP_ICON_BYTES)
        .expect("failed to load bundled icon");
    context.set_default_window_icon(Some(icon));
    context
}

pub fn run() {
    tauri::Builder::default()
        .manage(app_update::PendingUpdate::default())
        .manage(push_command::PushCancellation::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            app_update::check_app_update,
            app_update::install_app_update,
            commands::conversion::preview_cpa_source,
            commands::conversion::export_cpa_preview_accounts,
            push_command::check_sub2api_connection,
            push_command::cancel_cpa_push,
            push_command::push_cpa_source_to_sub2api
        ])
        .run(build_context())
        .expect("error while running cpa-bridge");
}

