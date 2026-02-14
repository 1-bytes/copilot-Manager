use crate::models::{Account, TokenData, AuthMethod};
use crate::modules;

/// Account service - decoupled from Tauri runtime
pub struct AccountService {
    pub integration: crate::modules::integration::SystemManager,
}

impl AccountService {
    pub fn new(integration: crate::modules::integration::SystemManager) -> Self {
        Self { integration }
    }

    /// Add account via GitHub token (PAT or device flow result)
    pub async fn add_account(&self, github_token: &str) -> Result<Account, String> {
        let temp_account_id = uuid::Uuid::new_v4().to_string();

        // 1. Validate token by getting GitHub user info
        let user = modules::oauth::get_github_user(github_token, Some(&temp_account_id)).await?;

        // 2. Get primary email (user.email may be None if private)
        let email = match &user.email {
            Some(e) if !e.is_empty() => e.clone(),
            _ => modules::oauth::get_github_primary_email(github_token, Some(&temp_account_id)).await?,
        };

        // 3. Get Copilot token
        let copilot_resp = modules::oauth::get_copilot_token(github_token, Some(&temp_account_id)).await?;

        // 4. Build TokenData
        let mut token = TokenData::new(
            github_token.to_string(),
            copilot_resp.token.clone(),
            copilot_resp.expires_at,
        );
        token.sku = copilot_resp.sku.clone();
        token.chat_enabled = copilot_resp.chat_enabled;
        token.account_type = Some(modules::oauth::infer_account_type(copilot_resp.sku.as_deref()));

        // 5. Upsert account
        let mut account = modules::account::upsert_account(email.clone(), user.name.clone(), token)?;
        account.github_login = Some(user.login.clone());

        // 6. Fetch Copilot plan info
        if let Ok(copilot_user) = modules::oauth::get_copilot_user(github_token, Some(&account.id)).await {
            account.copilot_plan = copilot_user.copilot_plan;
        }

        // 7. Save updated account
        let _ = modules::account::save_account(&account);

        modules::logger::log_info(&format!(
            "[Service] Added/Updated account: {} ({})",
            account.email,
            user.login
        ));

        Ok(account)
    }

    /// Delete account
    pub fn delete_account(&self, account_id: &str) -> Result<(), String> {
        modules::delete_account(account_id)?;
        self.integration.update_tray();
        Ok(())
    }

    /// Switch account
    pub async fn switch_account(&self, account_id: &str) -> Result<(), String> {
        modules::account::switch_account(account_id, &self.integration).await
    }

    /// List accounts
    pub fn list_accounts(&self) -> Result<Vec<Account>, String> {
        modules::list_accounts()
    }

    /// Get current account ID
    pub fn get_current_id(&self) -> Result<Option<String>, String> {
        modules::get_current_account_id()
    }

    // --- Device Flow OAuth ---

    pub async fn start_device_flow(&self) -> Result<modules::oauth_server::DeviceFlowInfo, String> {
        let handle = match &self.integration {
            modules::integration::SystemManager::Desktop(h) => Some(h.clone()),
            modules::integration::SystemManager::Headless => None,
        };
        modules::oauth_server::start_device_flow(handle, None).await
    }

    pub async fn complete_device_flow(&self) -> Result<Account, String> {
        let handle = match &self.integration {
            modules::integration::SystemManager::Desktop(h) => Some(h.clone()),
            modules::integration::SystemManager::Headless => None,
        };
        let token_res = modules::oauth_server::complete_device_flow(handle).await?;
        self.process_github_token(&token_res.access_token, AuthMethod::DeviceFlow).await
    }

    pub fn cancel_device_flow(&self) -> Result<(), String> {
        modules::oauth_server::cancel_device_flow()
    }

    /// Process a GitHub token (from device flow or direct input) into an account
    async fn process_github_token(
        &self,
        github_token: &str,
        auth_method: AuthMethod,
    ) -> Result<Account, String> {
        let mut account = self.add_account(github_token).await?;
        account.auth_method = auth_method;
        let _ = modules::account::save_account(&account);
        self.integration.update_tray();
        Ok(account)
    }
}
