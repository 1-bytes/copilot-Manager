use serde::{Deserialize, Serialize};

/// 模型配额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuota {
    pub name: String,
    pub percentage: i32,  // 剩余百分比 0-100
    pub reset_time: String,
}

/// 配额数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaData {
    pub models: Vec<ModelQuota>,
    pub last_updated: i64,
    #[serde(default)]
    pub is_forbidden: bool,
    /// Copilot plan (free_limited/individual/pro/business/enterprise)
    #[serde(default)]
    pub plan: Option<String>,
    /// Quota snapshots for tracking usage history
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quota_snapshots: Vec<serde_json::Value>,
    /// Quota reset date (ISO 8601 string)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_reset_date: Option<String>,
}

impl QuotaData {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            last_updated: chrono::Utc::now().timestamp(),
            is_forbidden: false,
            plan: None,
            quota_snapshots: Vec::new(),
            quota_reset_date: None,
        }
    }

    pub fn add_model(&mut self, name: String, percentage: i32, reset_time: String) {
        self.models.push(ModelQuota {
            name,
            percentage,
            reset_time,
        });
    }
}

impl Default for QuotaData {
    fn default() -> Self {
        Self::new()
    }
}
