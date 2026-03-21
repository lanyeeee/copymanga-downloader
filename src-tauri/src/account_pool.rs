use std::sync::Arc;

use anyhow::Context;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    errors::{GetUserProfileError, RiskControlResult},
    extensions::{AnyhowErrorToStringChain, AppHandleExt},
    types::AsyncMutex,
};

#[derive(Debug)]
pub struct AccountPool {
    app: AppHandle,
    accounts: papaya::HashMap<String, Arc<RwLock<Account>>>,
    register_lock: AsyncMutex<()>,
}

impl AccountPool {
    pub fn new(app: &AppHandle) -> anyhow::Result<Self> {
        // 读取account.json文件，获取账号信息
        let app_data_dir = app.path().app_data_dir().context("获取app_data_dir失败")?;

        let account_path = app_data_dir.join("account.json");
        if !account_path.exists() {
            std::fs::write(&account_path, "[]")
                .context(format!("创建`{}`失败", account_path.display()))?;
        }

        let accounts_string = std::fs::read_to_string(&account_path)
            .context(format!("读取`{}`失败", account_path.display()))?;

        let accounts: Vec<Account> = serde_json::from_str(&accounts_string).context(format!(
            "无法将accounts_string解析为Vec<Account>: {accounts_string}"
        ))?;

        let accounts = accounts
            .into_iter()
            .map(|a| (a.username.clone(), Arc::new(RwLock::new(a))))
            .collect();

        let account_pool = AccountPool {
            app: app.clone(),
            accounts,
            register_lock: AsyncMutex::new(()),
        };

        Ok(account_pool)
    }

    // 如果有可用的账号，则直接返回可用的账号
    // 否则注册一个新的账号，并返回这个新的账号
    pub async fn acquire_account(&self) -> RiskControlResult<Account> {
        use fake::faker::internet::en::Password;
        use fake::faker::name::en::{FirstName, LastName};
        use fake::Fake;

        // 如果有可用的账号，则直接返回可用的账号
        if let Some(account) = self.find_available_account().await {
            return Ok(account);
        }
        // 否则准备注册一个新的
        // 先拿register_lock，防止并发注册
        let _guard = self.register_lock.lock().await;
        // 拿到register_lock后再次检查是否有可用的账号
        // 如果有就用，否则才真的注册一个新账号
        // 再次检查是因为可能在拿到register_lock之前，其他线程已经注册了一个新账号，避免重复注册
        if let Some(account) = self.find_available_account().await {
            return Ok(account);
        }
        // 再次检查后，确认没有可用的账号，则注册一个新的
        let first_name = FirstName().fake::<String>();
        let last_name = LastName().fake::<String>();
        let number = rand::random::<u16>();
        let username = format!("{first_name}{last_name}{number}")
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>();

        let password = Password(10..30).fake::<String>();

        let copy_client = self.app.get_copy_client();
        copy_client.register(&username, &password).await?;
        let login_resp_data = copy_client.login(&username, &password).await?;

        let account = Account {
            username: username.clone(),
            password,
            token: login_resp_data.token,
            limited_at: 0,
            last_check_token_at: chrono::Utc::now().timestamp(),
            prepare_lock: Arc::new(AsyncMutex::new(())),
        };

        self.accounts
            .pin()
            .insert(username.clone(), Arc::new(RwLock::new(account.clone())));

        self.save().context("保存AccountPool失败")?;

        Ok(account)
    }

    async fn find_available_account(&self) -> Option<Account> {
        // papaya提供了pin_owned支持在异步任务期间持有Guard
        // 这里不用pin_owned而选择clone的原因是：
        // 1. 避免长期持有Guard
        // 2. clone开销极低，因为account包裹在Arc里，且username是短字符串，相比长期持有Guard，clone的开销完全可以接受
        let accounts: Vec<(String, Arc<RwLock<Account>>)> = self
            .accounts
            .pin()
            .iter()
            .map(|(username, account)| (username.clone(), account.clone()))
            .collect();

        for (username, account) in accounts {
            if account.read().is_limited() {
                continue;
            }

            match Account::prepare(&account, &self.app)
                .await
                .context(format!("用户名为`{username}`的账号准备失败"))
            {
                Ok(PrepareResult::Limited) => {}
                Ok(PrepareResult::Available(account)) => return Some(account.clone()),
                Ok(PrepareResult::Modified(account)) => {
                    if let Err(err) = self.save().context("保存AccountPool失败") {
                        let err_title = "从AccountPool中获取可用账号时遇到错误";
                        let string_chain = err.to_string_chain();
                        tracing::error!(err_title, message = string_chain);
                    }
                    return Some(account.clone());
                }
                Err(err) => {
                    let err_title = "从AccountPool中获取可用账号时遇到错误";
                    let string_chain = err.to_string_chain();
                    tracing::error!(err_title, message = string_chain);
                }
            }
        }

        None
    }

    pub fn mark_account_limited(&self, account: &Account) -> anyhow::Result<()> {
        if let Some(account) = self.accounts.pin().get(&account.username) {
            account.write().limited_at = chrono::Utc::now().timestamp();
            self.save()?;
        }

        Ok(())
    }

    fn save(&self) -> anyhow::Result<()> {
        // 保存账号信息到文件account.json
        let app_data_dir = self
            .app
            .path()
            .app_data_dir()
            .context("获取app_data_dir失败")?;

        let account_path = app_data_dir.join("account.json");

        let accounts: Vec<Account> = self
            .accounts
            .pin()
            .values()
            .map(|a| a.read().clone())
            .collect();

        let accounts_json =
            serde_json::to_string_pretty(&accounts).context("无法将accounts解析为json")?;

        std::fs::write(&account_path, accounts_json)
            .context(format!("写入`{}`失败", account_path.display()))?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub username: String,
    pub password: String,
    pub token: String,
    pub limited_at: i64,
    pub last_check_token_at: i64,
    #[serde(skip)]
    prepare_lock: Arc<AsyncMutex<()>>,
}

impl Account {
    fn is_limited(&self) -> bool {
        let now = chrono::Utc::now().timestamp();

        now - self.limited_at <= 60
    }

    fn should_check_token(&self) -> bool {
        let now = chrono::Utc::now().timestamp();

        now - self.last_check_token_at > 24 * 60 * 60
    }

    /// 返回值表示account的内容是否被修改了
    async fn prepare(
        account: &Arc<RwLock<Account>>,
        app: &AppHandle,
    ) -> anyhow::Result<PrepareResult> {
        // 保证一个账号在同一时间只有一个prepare在运行
        // 以防止多个任务同时使用同一个账号时，重复登录
        let prepare_lock = account.read().prepare_lock.clone();
        let _guard = prepare_lock.lock().await;

        let (token, username, password) = {
            let a = account.read();

            if a.is_limited() {
                return Ok(PrepareResult::Limited);
            }

            if !a.should_check_token() {
                return Ok(PrepareResult::Available(a.clone()));
            }

            (a.token.clone(), a.username.clone(), a.password.clone())
        };
        // 开始检查token是否过期
        let copy_client = app.get_copy_client();

        match copy_client.get_user_profile(&token).await {
            // token没过期，更新检查时间然后返回
            Ok(_) => {
                let mut a = account.write();
                a.last_check_token_at = chrono::Utc::now().timestamp();
                Ok(PrepareResult::Modified(a.clone()))
            }
            // token过期了，重新登录，更新token和检查时间，然后返回
            Err(GetUserProfileError::TokenErrorOrExpired) => {
                let login_resp = copy_client.login(&username, &password).await?;
                let mut a = account.write();
                a.token = login_resp.token;
                a.last_check_token_at = chrono::Utc::now().timestamp();
                Ok(PrepareResult::Modified(a.clone()))
            }
            Err(GetUserProfileError::Anyhow(err)) => Err(err),
        }
    }
}

enum PrepareResult {
    Available(Account),
    Modified(Account),
    Limited,
}
