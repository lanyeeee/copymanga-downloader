use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::types::DownloadFormat;

const DEFAULT_API_DOMAIN: &str = "api.2025copy.com";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub token: String,
    pub download_dir: PathBuf,
    pub export_dir: PathBuf,
    pub api_domain_mode: ApiDomainMode,
    pub custom_api_domain: String,
    pub download_format: DownloadFormat,
    pub enable_file_logger: bool,
    pub chapter_concurrency: usize,
    pub chapter_download_interval_sec: u64,
    pub img_concurrency: usize,
    pub img_download_interval_sec: u64,
    pub update_downloaded_comics_interval_sec: u64,
    pub comic_dir_fmt: String,
    pub chapter_dir_fmt: String,
    pub create_pdf_concurrency: usize,
    pub enable_merge_pdf: bool,
}

impl Config {
    pub fn new(app: &AppHandle) -> anyhow::Result<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");

        let config = if config_path.exists() {
            let config_string = std::fs::read_to_string(config_path)?;
            match serde_json::from_str(&config_string) {
                // 如果能够直接解析为Config，则直接返回
                Ok(config) => config,
                // 否则，将默认配置与文件中已有的配置合并
                // 以免新版本添加了新的配置项，用户升级到新版本后，所有配置项都被重置
                Err(_) => Config::merge_config(&config_string, &app_data_dir),
            }
        } else {
            Config::default(&app_data_dir)
        };
        config.save(app)?;
        Ok(config)
    }

    pub fn save(&self, app: &AppHandle) -> anyhow::Result<()> {
        let app_data_dir = app.path().app_data_dir()?;
        let config_path = app_data_dir.join("config.json");
        let config_string = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, config_string)?;
        Ok(())
    }

    fn merge_config(config_string: &str, app_data_dir: &Path) -> Config {
        let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(config_string) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(ref mut map) = json_value else {
            return Config::default(app_data_dir);
        };
        let Ok(default_config_value) = serde_json::to_value(Config::default(app_data_dir)) else {
            return Config::default(app_data_dir);
        };
        let serde_json::Value::Object(default_map) = default_config_value else {
            return Config::default(app_data_dir);
        };
        for (key, value) in default_map {
            map.entry(key).or_insert(value);
        }
        let Ok(config) = serde_json::from_value(json_value) else {
            return Config::default(app_data_dir);
        };
        config
    }

    fn default(app_data_dir: &Path) -> Config {
        let cpu_core_num = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);

        Config {
            token: String::new(),
            download_dir: app_data_dir.join("漫画下载"),
            export_dir: app_data_dir.join("漫画导出"),
            api_domain_mode: ApiDomainMode::default(),
            custom_api_domain: DEFAULT_API_DOMAIN.to_string(),
            download_format: DownloadFormat::Webp,
            enable_file_logger: true,
            chapter_concurrency: 3,
            chapter_download_interval_sec: 0,
            img_concurrency: 30,
            img_download_interval_sec: 0,
            update_downloaded_comics_interval_sec: 0,
            comic_dir_fmt: "{comic_title}".to_string(),
            chapter_dir_fmt: "{group_title}/{order} {chapter_title}".to_string(),
            create_pdf_concurrency: cpu_core_num,
            enable_merge_pdf: true,
        }
    }

    pub fn get_authorization(&self) -> String {
        format!("Token {}", self.token)
    }

    pub fn get_api_domain(&self) -> String {
        if self.api_domain_mode == ApiDomainMode::Custom {
            self.custom_api_domain.clone()
        } else {
            DEFAULT_API_DOMAIN.to_string()
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum ApiDomainMode {
    #[default]
    Default,
    Custom,
}
