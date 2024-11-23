use std::sync::Arc;

use anyhow::Context;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{copy_client::CopyClient, errors::CopyMangaResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub password: String,
    pub token: String,
    pub limited_at: i64,
}

#[derive(Debug)]
pub struct AccountPool {
    app: AppHandle,
    accounts: Vec<Arc<RwLock<Account>>>,
}

impl AccountPool {
    pub fn new(app: &AppHandle) -> anyhow::Result<Self> {
        // 读取account.json文件，获取账号信息
        let app_data_dir = app
            .path()
            .app_data_dir()
            .context("创建AccountPool失败，获取app_data_dir失败".to_string())?;

        let account_path = app_data_dir.join("account.json");
        if !account_path.exists() {
            std::fs::write(&account_path, "[]")
                .context(format!("创建AccountPool失败，创建 {account_path:?} 失败"))?;
        }

        let accounts_string = std::fs::read_to_string(&account_path)
            .context(format!("创建AccountPool失败，读取 {account_path:?} 失败"))?;

        let accounts: Vec<Account> = serde_json::from_str(&accounts_string).context(format!(
            "创建AccountPool失败，无法将accounts_string解析为Vec<Account>: {accounts_string}"
        ))?;

        let accounts: Vec<Arc<RwLock<Account>>> = accounts
            .into_iter()
            .map(|account| Arc::new(RwLock::new(account)))
            .collect();

        let account_pool = AccountPool {
            app: app.clone(),
            accounts,
        };
        Ok(account_pool)
    }

    pub fn get_available_account(&self) -> Option<Arc<RwLock<Account>>> {
        let now = chrono::Local::now().timestamp();

        self.accounts
            .iter()
            .find(|a| now - a.read().limited_at > 60)
            .cloned()
    }

    pub async fn register(&mut self) -> CopyMangaResult<Arc<RwLock<Account>>> {
        use fake::faker::internet::en::Password;
        use fake::faker::name::en::{FirstName, LastName};
        use fake::Fake;

        let copy_client = self.copy_client();

        let first_name = FirstName().fake::<String>();
        let last_name = LastName().fake::<String>();
        let number = rand::random::<u16>();
        let username = format!("{first_name}{last_name}{number}")
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>();

        let password = Password(10..30).fake::<String>();

        copy_client.register(&username, &password).await?;

        let login_resp_data = copy_client.login(&username, &password).await?;
        let token = login_resp_data.token;

        let account = Account {
            username,
            password,
            token,
            limited_at: 0,
        };
        let account = Arc::new(RwLock::new(account));
        self.accounts.push(account.clone());

        Ok(account)
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        // 保存账号信息到文件account.json
        let app_data_dir = self
            .app
            .path()
            .app_data_dir()
            .context("保存AccountPool失败，获取app_data_dir失败".to_string())?;

        let account_path = app_data_dir.join("account.json");
        let accounts: Vec<Account> = self
            .accounts
            .iter()
            .map(|account| account.read().clone())
            .collect();

        let accounts_json = serde_json::to_string_pretty(&accounts)
            .context("保存AccountPool失败，无法将accounts解析为json")?;

        std::fs::write(&account_path, accounts_json)
            .context(format!("保存AccountPool失败，写入 {account_path:?} 失败"))?;

        Ok(())
    }

    fn copy_client(&self) -> CopyClient {
        self.app.state::<CopyClient>().inner().clone()
    }
}
