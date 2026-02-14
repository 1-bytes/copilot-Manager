// Upstream client implementation for GitHub Copilot API
// High-performance HTTP client with per-account proxy support

use dashmap::DashMap;
use reqwest::{Client, Response};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

/// Record of a fallback attempt (kept for API compatibility)
#[derive(Debug, Clone)]
pub struct FallbackAttemptLog {
    /// The endpoint URL attempted
    pub endpoint_url: String,
    /// HTTP status code (None for network errors)
    pub status: Option<u16>,
    /// Error description
    pub error: String,
}

/// Upstream call result containing the response and any fallback attempt records
pub struct UpstreamCallResult {
    /// The final HTTP response
    pub response: Response,
    /// Fallback attempt records (empty on first-try success)
    pub fallback_attempts: Vec<FallbackAttemptLog>,
}

/// Mask email for logging: show first 3 chars + *** + @ + first 2 domain chars + ***
/// Example: "userexample@gmail.com" -> "use***@gm***"
pub fn mask_email(email: &str) -> String {
    if let Some(at_pos) = email.find('@') {
        let local = &email[..at_pos];
        let domain = &email[at_pos + 1..];
        let local_prefix: String = local.chars().take(3).collect();
        let domain_prefix: String = domain.chars().take(2).collect();
        format!("{}***@{}***", local_prefix, domain_prefix)
    } else {
        let prefix: String = email.chars().take(5).collect();
        format!("{}***", prefix)
    }
}

// GitHub Copilot API base URLs by account type
const COPILOT_API_BASE_INDIVIDUAL: &str = "https://api.githubcopilot.com";
const COPILOT_API_BASE_BUSINESS: &str = "https://api.business.githubcopilot.com";
const COPILOT_API_BASE_ENTERPRISE: &str = "https://api.enterprise.githubcopilot.com";

/// Get the Copilot API base URL for a given account type
pub fn copilot_base_url(account_type: Option<&str>) -> &'static str {
    match account_type {
        Some("business") => COPILOT_API_BASE_BUSINESS,
        Some("enterprise") => COPILOT_API_BASE_ENTERPRISE,
        _ => COPILOT_API_BASE_INDIVIDUAL,
    }
}

pub struct UpstreamClient {
    default_client: Client,
    proxy_pool: Option<Arc<crate::proxy::proxy_pool::ProxyPoolManager>>,
    client_cache: DashMap<String, Client>,
    user_agent_override: RwLock<Option<String>>,
}

impl UpstreamClient {
    pub fn new(
        proxy_config: Option<crate::proxy::config::UpstreamProxyConfig>,
        proxy_pool: Option<Arc<crate::proxy::proxy_pool::ProxyPoolManager>>,
    ) -> Self {
        let default_client = Self::build_client_internal(proxy_config)
            .expect("Failed to create default HTTP client");

        Self {
            default_client,
            proxy_pool,
            client_cache: DashMap::new(),
            user_agent_override: RwLock::new(None),
        }
    }

    /// Internal helper to build a client with optional upstream proxy config
    fn build_client_internal(
        proxy_config: Option<crate::proxy::config::UpstreamProxyConfig>,
    ) -> Result<Client, reqwest::Error> {
        let mut builder = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(600))
            .user_agent(crate::constants::USER_AGENT.as_str());

        if let Some(config) = proxy_config {
            if config.enabled && !config.url.is_empty() {
                let url = crate::proxy::config::normalize_proxy_url(&config.url);
                if let Ok(proxy) = reqwest::Proxy::all(&url) {
                    builder = builder.proxy(proxy);
                    tracing::info!("UpstreamClient enabled proxy: {}", url);
                }
            }
        }

        builder.build()
    }

    /// Build a client with a specific PoolProxyConfig (from ProxyPool)
    fn build_client_with_proxy(
        &self,
        proxy_config: crate::proxy::proxy_pool::PoolProxyConfig,
    ) -> Result<Client, reqwest::Error> {
        Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(600))
            .user_agent(crate::constants::USER_AGENT.as_str())
            .proxy(proxy_config.proxy)
            .build()
    }

    /// Set dynamic User-Agent override
    pub async fn set_user_agent_override(&self, ua: Option<String>) {
        let mut lock = self.user_agent_override.write().await;
        *lock = ua;
        tracing::debug!("UpstreamClient User-Agent override updated: {:?}", lock);
    }

    /// Get current User-Agent
    pub async fn get_user_agent(&self) -> String {
        let ua_override = self.user_agent_override.read().await;
        ua_override
            .as_ref()
            .cloned()
            .unwrap_or_else(|| crate::constants::USER_AGENT.clone())
    }

    /// Get client for a specific account (or default if no proxy bound)
    pub async fn get_client(&self, account_id: Option<&str>) -> Client {
        if let Some(pool) = &self.proxy_pool {
            if let Some(acc_id) = account_id {
                match pool.get_proxy_for_account(acc_id).await {
                    Ok(Some(proxy_cfg)) => {
                        if let Some(client) = self.client_cache.get(&proxy_cfg.entry_id) {
                            return client.clone();
                        }
                        match self.build_client_with_proxy(proxy_cfg.clone()) {
                            Ok(client) => {
                                self.client_cache
                                    .insert(proxy_cfg.entry_id.clone(), client.clone());
                                tracing::info!(
                                    "Using ProxyPool proxy ID: {} for account: {}",
                                    proxy_cfg.entry_id,
                                    acc_id
                                );
                                return client;
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build client for proxy {}: {}, falling back to default",
                                    proxy_cfg.entry_id,
                                    e
                                );
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::error!(
                            "Error getting proxy for account {}: {}, falling back to default",
                            acc_id,
                            e
                        );
                    }
                }
            }
        }
        self.default_client.clone()
    }

    /// Call GitHub Copilot API endpoint
    pub async fn call_copilot(
        &self,
        endpoint: &str,
        copilot_token: &str,
        body: Option<&serde_json::Value>,
        extra_headers: Option<&[(String, String)]>,
        account_type: Option<&str>,
        account_id: Option<&str>,
        method: reqwest::Method,
    ) -> Result<UpstreamCallResult, String> {
        let base_url = copilot_base_url(account_type);
        let url = format!("{}{}", base_url, endpoint);

        let client = self.get_client(account_id).await;

        let mut request = client
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", copilot_token))
            .header("Content-Type", "application/json")
            .header("Editor-Version", "vscode/1.99.0")
            .header("Editor-Plugin-Version", "copilot-chat/0.35.0")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("X-GitHub-Api-Version", "2025-04-01")
            .header("openai-intent", "conversation-panel")
            .header("openai-organization", "github-copilot");

        // Apply custom user agent or default
        let ua = {
            let guard = self.user_agent_override.read().await;
            guard.clone()
        };
        request = request.header(
            "User-Agent",
            ua.unwrap_or_else(|| "GitHubCopilotChat/0.35.0".to_string()),
        );

        // Apply extra headers
        if let Some(headers) = extra_headers {
            for (key, value) in headers {
                request = request.header(key.as_str(), value.as_str());
            }
        }

        // Apply body
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("copilot_upstream_error: {}", e))?;

        Ok(UpstreamCallResult {
            response,
            fallback_attempts: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copilot_base_url() {
        assert_eq!(
            copilot_base_url(None),
            "https://api.githubcopilot.com"
        );
        assert_eq!(
            copilot_base_url(Some("individual")),
            "https://api.githubcopilot.com"
        );
        assert_eq!(
            copilot_base_url(Some("business")),
            "https://api.business.githubcopilot.com"
        );
        assert_eq!(
            copilot_base_url(Some("enterprise")),
            "https://api.enterprise.githubcopilot.com"
        );
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("userexample@gmail.com"), "use***@gm***");
        assert_eq!(mask_email("ab@x.com"), "ab***@x.***");
        assert_eq!(mask_email("noemail"), "noema***");
    }
}
