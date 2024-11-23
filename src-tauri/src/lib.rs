mod account_pool;
mod commands;
mod config;
mod copy_client;
mod download_manager;
mod errors;
mod events;
mod extensions;
mod responses;
mod types;
mod utils;

use account_pool::AccountPool;
use anyhow::Context;
use copy_client::CopyClient;
use download_manager::DownloadManager;
use events::DownloadEvent;
use parking_lot::RwLock;
use tauri::{Manager, Wry};
use types::AsyncRwLock;

use crate::commands::*;
use crate::config::Config;

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri_specta::Builder::<Wry>::new()
        .commands(tauri_specta::collect_commands![
            greet,
            get_config,
            save_config,
            register,
            login,
            get_user_profile,
            search,
            get_comic,
            get_group_chapters,
            get_chapter,
            download_chapters,
            show_path_in_file_manager,
        ])
        .events(tauri_specta::collect_events![DownloadEvent]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .formatter(specta_typescript::formatter::prettier)
                .header("// @ts-nocheck"), // 跳过检查
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_data_dir = app
                .path()
                .app_data_dir()
                .context("failed to get app data dir")?;

            std::fs::create_dir_all(&app_data_dir)
                .context(format!("failed to create app data dir: {app_data_dir:?}"))?;
            println!("app data dir: {app_data_dir:?}");

            let config = RwLock::new(Config::new(app.handle())?);
            app.manage(config);

            let copy_client = CopyClient::new(app.handle().clone());
            app.manage(copy_client);

            let download_manager = DownloadManager::new(app.handle());
            app.manage(download_manager);

            let account_pool = AsyncRwLock::new(AccountPool::new(app.handle())?);
            app.manage(account_pool);

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}
