use crate::modules::db;
use crate::models::Account;
use std::fs;

pub trait SystemIntegration: Send + Sync {
    /// 当切换账号时执行的系统层操作（如杀进程、写入文件、注入数据库）
    async fn on_account_switch(&self, account: &crate::models::Account) -> Result<(), String>;
    
    /// 更新系统托盘（如果适用）
    fn update_tray(&self);
    
    /// 发送系统通知
    fn show_notification(&self, title: &str, body: &str);
}

/// 桌面版实现：包含完整的进程控制和 UI 同步
pub struct DesktopIntegration {
    pub app_handle: tauri::AppHandle,
}

impl SystemIntegration for DesktopIntegration {
    async fn on_account_switch(&self, account: &crate::models::Account) -> Result<(), String> {
        crate::modules::logger::log_info(&format!("[Desktop] Executing system switch for: {}", account.email));
        
        // For Copilot mode, we no longer need to manipulate storage.json or device profiles.
        // The account switch is primarily an in-memory and persistence operation.

        // 1. 数据库处理 (if db path exists) - currently a no-op in Copilot mode
        if let Ok(db_path) = db::get_db_path() {
            if db_path.exists() {
                let backup_path = db_path.with_extension("vscdb.backup");
                let _ = fs::copy(&db_path, &backup_path);
            }

            // inject_token is a no-op stub in Copilot mode, but kept for interface compatibility
            let _ = db::inject_token(
                &db_path,
                &account.token.github_token,
                "",  // no refresh_token in Copilot mode
                account.token.copilot_token_expires_at,
                &account.email,
            );
        }

        // 2. 更新托盘
        let _ = crate::modules::tray::update_tray_menus(&self.app_handle);
        
        Ok(())
    }

    fn update_tray(&self) {
        let _ = crate::modules::tray::update_tray_menus(&self.app_handle);
    }

    fn show_notification(&self, title: &str, body: &str) {
        crate::modules::logger::log_info(&format!("[Notification] {}: {}", title, body));
    }
}

/// Headless/Docker 实现：仅执行数据层操作，忽略 UI 和进程控制
pub struct HeadlessIntegration;

impl SystemIntegration for HeadlessIntegration {
    async fn on_account_switch(&self, account: &crate::models::Account) -> Result<(), String> {
        crate::modules::logger::log_info(&format!("[Headless] Account switched in memory: {}", account.email));
        Ok(())
    }

    fn update_tray(&self) {
        // No-op
    }

    fn show_notification(&self, title: &str, body: &str) {
        crate::modules::logger::log_info(&format!("[Log Notification] {}: {}", title, body));
    }
}

/// 系统集成管理器：替代 Arc<dyn SystemIntegration> 以解决 async trait 的 dyn 兼容性问题
#[derive(Clone)]
pub enum SystemManager {
    Desktop(tauri::AppHandle),
    Headless,
}

impl SystemManager {
    pub async fn on_account_switch(&self, account: &Account) -> Result<(), String> {
        match self {
            SystemManager::Desktop(handle) => {
                let integration = DesktopIntegration { app_handle: handle.clone() };
                integration.on_account_switch(account).await
            },
            SystemManager::Headless => {
                let integration = HeadlessIntegration;
                integration.on_account_switch(account).await
            }
        }
    }

    pub fn update_tray(&self) {
        if let SystemManager::Desktop(handle) = self {
            let integration = DesktopIntegration { app_handle: handle.clone() };
            integration.update_tray();
        }
    }

    pub fn show_notification(&self, title: &str, body: &str) {
        match self {
            SystemManager::Desktop(handle) => {
                let integration = DesktopIntegration { app_handle: handle.clone() };
                integration.show_notification(title, body);
            },
            SystemManager::Headless => {
                let integration = HeadlessIntegration;
                integration.show_notification(title, body);
            }
        }
    }
}

impl SystemIntegration for SystemManager {
    async fn on_account_switch(&self, account: &crate::models::Account) -> Result<(), String> {
        match self {
            SystemManager::Desktop(handle) => {
                let integration = DesktopIntegration { app_handle: handle.clone() };
                integration.on_account_switch(account).await
            },
            SystemManager::Headless => {
                let integration = HeadlessIntegration;
                integration.on_account_switch(account).await
            }
        }
    }

    fn update_tray(&self) {
        match self {
            SystemManager::Desktop(handle) => {
                let integration = DesktopIntegration { app_handle: handle.clone() };
                integration.update_tray();
            },
            SystemManager::Headless => {
                let integration = HeadlessIntegration;
                integration.update_tray();
            }
        }
    }

    fn show_notification(&self, title: &str, body: &str) {
        match self {
            SystemManager::Desktop(handle) => {
                let integration = DesktopIntegration { app_handle: handle.clone() };
                integration.show_notification(title, body);
            },
            SystemManager::Headless => {
                let integration = HeadlessIntegration;
                integration.show_notification(title, body);
            }
        }
    }
}
