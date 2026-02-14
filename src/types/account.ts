export interface Account {
    id: string;
    email: string;
    name?: string;
    github_login?: string;
    copilot_plan?: string;
    auth_method?: string;  // "device_flow" | "oauth_app" | "personal_access_token"
    token: TokenData;
    quota?: QuotaData;
    disabled?: boolean;
    disabled_reason?: string;
    disabled_at?: number;
    proxy_disabled?: boolean;
    proxy_disabled_reason?: string;
    proxy_disabled_at?: number;
    protected_models?: string[];
    custom_label?: string;  // 用户自定义标签
    created_at: number;
    last_used: number;
}

export interface TokenData {
    github_token: string;
    copilot_token?: string;
    copilot_token_expires_at: number;
    sku?: string;
    chat_enabled?: boolean;
    account_type?: string;  // "individual" | "business" | "enterprise"
}

export interface QuotaData {
    plan: string;
    chat_enabled: boolean;
    models: ModelQuota[];
    last_updated: number;
    is_forbidden: boolean;
    quota_reset_date?: string;
    quota_snapshots?: QuotaSnapshot[];
}

export interface ModelQuota {
    name: string;
    percentage: number;
    reset_time: string;
}

export interface QuotaSnapshot {
    timestamp: number;
    models: ModelQuota[];
}

export interface AccountExportItem {
    email: string;
    github_token: string;
    github_login?: string;
}
