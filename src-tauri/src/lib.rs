mod account_pool;
mod commands;
mod config;
mod copy_client;
mod downloader;
mod errors;
mod events;
mod export;
mod extensions;
mod logger;
mod responses;
mod types;
mod utils;

use eyre::WrapErr;
use parking_lot::RwLock;
use tauri::{Manager, Wry};

use crate::{
    account_pool::AccountPool,
    commands::*,
    config::Config,
    copy_client::CopyClient,
    downloader::download_manager::DownloadManager,
    errors::install_custom_eyre_handler,
    events::{
        DownloadEvent, ExportCbzEvent, ExportPdfEvent, LogEvent, UpdateDownloadedComicsEvent,
    },
    export::ComicExportLock,
    types::LogMetadata,
};

fn generate_context() -> tauri::Context<Wry> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_custom_eyre_handler().unwrap();

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
            get_favorite,
            create_download_tasks,
            pause_download_task,
            resume_download_task,
            delete_download_task,
            save_metadata,
            get_downloaded_comics,
            export_cbz,
            export_pdf,
            export_cbz_chapters,
            export_pdf_chapters,
            update_downloaded_comics,
            get_logs_dir_size,
            show_path_in_file_manager,
            get_synced_comic,
            get_synced_comic_in_favorite,
            get_synced_comic_in_search,
        ])
        .events(tauri_specta::collect_events![
            DownloadEvent,
            ExportCbzEvent,
            ExportPdfEvent,
            UpdateDownloadedComicsEvent,
            LogEvent,
        ])
        .typ::<LogMetadata>();

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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_data_dir = app.path().app_data_dir().wrap_err("获取app_data_dir失败")?;

            std::fs::create_dir_all(&app_data_dir)
                .wrap_err(format!("创建`{}`失败", app_data_dir.display()))?;

            let config = RwLock::new(Config::new(app.handle()).wrap_err("创建Config失败")?);
            app.manage(config);

            let copy_client = CopyClient::new(app.handle().clone());
            app.manage(copy_client);

            let download_manager = DownloadManager::new(app.handle());
            app.manage(download_manager);

            let account_pool = AccountPool::new(app.handle()).wrap_err("创建AccountPool失败")?;
            app.manage(account_pool);

            let export_lock = ComicExportLock::new();
            app.manage(export_lock);

            logger::init(app.handle())?;

            Ok(())
        })
        .run(generate_context())
        .expect("error while running tauri application");
}
