use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub github_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copilot_token: Option<String>,
    #[serde(default)]
    pub copilot_token_expires_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,  // "individual" | "business" | "enterprise"
}

impl TokenData {
    pub fn new(
        github_token: String,
        copilot_token: Option<String>,
        copilot_token_expires_at: i64,
    ) -> Self {
        Self {
            github_token,
            copilot_token,
            copilot_token_expires_at,
            sku: None,
            chat_enabled: None,
            account_type: None,
        }
    }

    /// Check if the copilot token is expired or about to expire (within 60 seconds)
    pub fn is_copilot_token_expired(&self) -> bool {
        if self.copilot_token.is_none() {
            return true;
        }
        let now = chrono::Utc::now().timestamp();
        now >= self.copilot_token_expires_at - 60
    }
}
