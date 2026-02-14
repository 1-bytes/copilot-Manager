use serde::{Deserialize, Serialize};
use crate::modules::logger;

// GitHub OAuth Device Flow configuration
// Uses the same Client ID as VS Code Copilot
const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// Get the GitHub OAuth client ID
pub fn client_id() -> &'static str {
    GITHUB_CLIENT_ID
}

// ============================================================================
// Response types
// ============================================================================

/// Device code response from GitHub
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
    pub interval: i64,
}

/// Token response from GitHub device flow polling
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

/// GitHub user info
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// GitHub email entry (from /user/emails)
#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

/// Copilot token response
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CopilotTokenResponse {
    pub token: Option<String>,
    pub expires_at: i64,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub chat_enabled: Option<bool>,
}

/// Copilot user info
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CopilotUser {
    #[serde(default)]
    pub copilot_plan: Option<String>,
}

/// Poll result enum
pub enum PollResult {
    Success(DeviceTokenResponse),
    Pending,
    SlowDown,
    Expired,
    Denied,
    Error(String),
}

// ============================================================================
// OAuth Device Flow
// ============================================================================

/// Step 1: Request a device code from GitHub
pub async fn request_device_code(client_id_override: Option<&str>) -> Result<DeviceCodeResponse, String> {
    let cid = client_id_override.unwrap_or(GITHUB_CLIENT_ID);

    let client = crate::utils::http::get_client();
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", cid), ("scope", "read:user user:email")])
        .send()
        .await
        .map_err(|e| format!("request_device_code failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("request_device_code HTTP {}: {}", status, body));
    }

    resp.json::<DeviceCodeResponse>()
        .await
        .map_err(|e| format!("parse_device_code_response: {}", e))
}

/// Step 2: Poll for token (single attempt)
pub async fn poll_for_token(
    device_code: &str,
    client_id_override: Option<&str>,
) -> Result<PollResult, String> {
    let cid = client_id_override.unwrap_or(GITHUB_CLIENT_ID);

    let client = crate::utils::http::get_client();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", cid),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("poll_for_token failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("poll_for_token HTTP {}: {}", status, body));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse_poll_response: {}", e))?;

    // Check for error response (pending, slow_down, etc.)
    if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
        return Ok(match error {
            "authorization_pending" => PollResult::Pending,
            "slow_down" => PollResult::SlowDown,
            "expired_token" => PollResult::Expired,
            "access_denied" => PollResult::Denied,
            _ => PollResult::Error(format!("Unknown error: {}", error)),
        });
    }

    // Success - parse token response
    let token_resp: DeviceTokenResponse = serde_json::from_value(body)
        .map_err(|e| format!("parse_token_response: {}", e))?;
    Ok(PollResult::Success(token_resp))
}

// ============================================================================
// GitHub API
// ============================================================================

/// Get GitHub user info using an access token
pub async fn get_github_user(
    github_token: &str,
    account_id: Option<&str>,
) -> Result<GitHubUser, String> {
    let _id = account_id.unwrap_or("unknown");
    let client = if let Some(pool) = crate::proxy::proxy_pool::get_global_proxy_pool() {
        pool.get_effective_client(account_id, 15).await
    } else {
        crate::utils::http::get_client()
    };

    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", github_token))
        .header("User-Agent", "copilot-manager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("get_github_user failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("get_github_user HTTP {}: {}", status, body));
    }

    resp.json::<GitHubUser>()
        .await
        .map_err(|e| format!("parse_github_user: {}", e))
}

/// Get GitHub user primary email
pub async fn get_github_primary_email(
    github_token: &str,
    account_id: Option<&str>,
) -> Result<String, String> {
    let client = if let Some(pool) = crate::proxy::proxy_pool::get_global_proxy_pool() {
        pool.get_effective_client(account_id, 15).await
    } else {
        crate::utils::http::get_client()
    };

    let resp = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", github_token))
        .header("User-Agent", "copilot-manager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("get_github_primary_email failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("get_github_primary_email HTTP {}: {}", status, body));
    }

    let emails: Vec<GitHubEmail> = resp
        .json()
        .await
        .map_err(|e| format!("parse_github_emails: {}", e))?;

    emails
        .iter()
        .find(|e| e.primary && e.verified)
        .or_else(|| emails.iter().find(|e| e.primary))
        .or_else(|| emails.first())
        .map(|e| e.email.clone())
        .ok_or_else(|| "no_email_found".to_string())
}

// ============================================================================
// Copilot API
// ============================================================================

/// Get Copilot token (short-lived, ~30 min)
pub async fn get_copilot_token(
    github_token: &str,
    account_id: Option<&str>,
) -> Result<CopilotTokenResponse, String> {
    let client = if let Some(pool) = crate::proxy::proxy_pool::get_global_proxy_pool() {
        pool.get_effective_client(account_id, 15).await
    } else {
        crate::utils::http::get_client()
    };

    let resp = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", github_token))
        .header("User-Agent", "copilot-manager")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("get_copilot_token failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("get_copilot_token HTTP {}: {}", status, body));
    }

    resp.json::<CopilotTokenResponse>()
        .await
        .map_err(|e| format!("parse_copilot_token: {}", e))
}

/// Get Copilot user plan info
pub async fn get_copilot_user(
    github_token: &str,
    account_id: Option<&str>,
) -> Result<CopilotUser, String> {
    let client = if let Some(pool) = crate::proxy::proxy_pool::get_global_proxy_pool() {
        pool.get_effective_client(account_id, 15).await
    } else {
        crate::utils::http::get_client()
    };

    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", github_token))
        .header("User-Agent", "copilot-manager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("get_copilot_user failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("get_copilot_user HTTP {}: {}", status, body));
    }

    // Try to parse copilot-related fields from the user response
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse_copilot_user: {}", e))?;

    let copilot_plan = body
        .get("plan")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    Ok(CopilotUser { copilot_plan })
}

/// Ensure fresh copilot token - refresh if expired
pub async fn ensure_fresh_copilot_token(
    account: &mut crate::models::Account,
) -> Result<String, String> {
    if !account.token.is_copilot_token_expired() {
        return Ok(account.token.copilot_token.clone().unwrap());
    }

    logger::log_info(&format!(
        "[OAuth] Refreshing Copilot token for {}",
        account.email
    ));

    let copilot_resp = get_copilot_token(&account.token.github_token, Some(&account.id)).await?;

    account.token.copilot_token = copilot_resp.token.clone();
    account.token.copilot_token_expires_at = copilot_resp.expires_at;
    account.token.sku = copilot_resp.sku.clone();
    account.token.chat_enabled = copilot_resp.chat_enabled;
    account.token.account_type = Some(infer_account_type(copilot_resp.sku.as_deref()));

    // Save updated token
    let _ = crate::modules::account::save_account(account);

    copilot_resp
        .token
        .ok_or_else(|| "copilot_token_is_none".to_string())
}

/// Infer account type from SKU
pub fn infer_account_type(sku: Option<&str>) -> String {
    match sku {
        Some(s) if s.contains("business") => "business".to_string(),
        Some(s) if s.contains("enterprise") => "enterprise".to_string(),
        _ => "individual".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_account_type() {
        assert_eq!(infer_account_type(None), "individual");
        assert_eq!(infer_account_type(Some("copilot_for_business_seat")), "business");
        assert_eq!(infer_account_type(Some("copilot_enterprise")), "enterprise");
        assert_eq!(infer_account_type(Some("copilot_individual")), "individual");
    }
}
