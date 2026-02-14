use std::sync::{Mutex, OnceLock};
use serde::Serialize;
use crate::modules::oauth;
use crate::modules::logger;

// ============================================================================
// Device Flow State
// ============================================================================

struct DeviceFlowState {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_at: i64,
    interval: i64,
    cancelled: bool,
}

static DEVICE_FLOW_STATE: OnceLock<Mutex<Option<DeviceFlowState>>> = OnceLock::new();

fn get_device_flow_state() -> &'static Mutex<Option<DeviceFlowState>> {
    DEVICE_FLOW_STATE.get_or_init(|| Mutex::new(None))
}

/// Info returned to the frontend so the user can authorize
#[derive(Debug, Clone, Serialize)]
pub struct DeviceFlowInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
}

// ============================================================================
// Public API
// ============================================================================

/// Start a GitHub device flow: request a device code, store it, and return info for the user.
/// Optionally opens the verification URL in the default browser.
pub async fn start_device_flow(
    app_handle: Option<tauri::AppHandle>,
    client_id_override: Option<&str>,
) -> Result<DeviceFlowInfo, String> {
    // Cancel any previous flow
    let _ = cancel_device_flow();

    let resp = oauth::request_device_code(client_id_override).await?;

    let now = chrono::Utc::now().timestamp();
    let expires_at = now + resp.expires_in;

    let info = DeviceFlowInfo {
        user_code: resp.user_code.clone(),
        verification_uri: resp.verification_uri.clone(),
        expires_in: resp.expires_in,
    };

    // Save state
    if let Ok(mut lock) = get_device_flow_state().lock() {
        *lock = Some(DeviceFlowState {
            device_code: resp.device_code,
            user_code: resp.user_code,
            verification_uri: resp.verification_uri.clone(),
            expires_at,
            interval: resp.interval,
            cancelled: false,
        });
    }

    logger::log_info(&format!(
        "[DeviceFlow] Started. User code: {}, URI: {}",
        info.user_code, info.verification_uri
    ));

    // Optionally open browser
    if let Some(handle) = app_handle {
        use tauri_plugin_opener::OpenerExt;
        let _ = handle
            .opener()
            .open_url(&resp.verification_uri, None::<String>);
    }

    Ok(info)
}

/// Poll GitHub until the user authorizes (or the flow expires/is cancelled).
/// Returns the GitHub access token on success.
pub async fn complete_device_flow(
    _app_handle: Option<tauri::AppHandle>,
) -> Result<oauth::DeviceTokenResponse, String> {
    // Extract state
    let (device_code, expires_at, mut interval) = {
        let lock = get_device_flow_state()
            .lock()
            .map_err(|_| "device_flow_state_lock_poisoned".to_string())?;
        let state = lock.as_ref().ok_or("no_active_device_flow")?;
        if state.cancelled {
            return Err("device_flow_cancelled".to_string());
        }
        (
            state.device_code.clone(),
            state.expires_at,
            state.interval,
        )
    };

    // Ensure minimum 5s interval
    if interval < 5 {
        interval = 5;
    }

    loop {
        // Check expiry
        let now = chrono::Utc::now().timestamp();
        if now >= expires_at {
            cleanup_flow();
            return Err("device_flow_expired".to_string());
        }

        // Check cancelled
        {
            let lock = get_device_flow_state().lock().map_err(|_| "lock_poisoned".to_string())?;
            if let Some(state) = lock.as_ref() {
                if state.cancelled {
                    drop(lock);
                    cleanup_flow();
                    return Err("device_flow_cancelled".to_string());
                }
            } else {
                return Err("device_flow_state_cleared".to_string());
            }
        }

        // Wait before polling
        tokio::time::sleep(tokio::time::Duration::from_secs(interval as u64)).await;

        // Poll
        match oauth::poll_for_token(&device_code, None).await? {
            oauth::PollResult::Success(token_resp) => {
                logger::log_info("[DeviceFlow] Authorization successful!");
                cleanup_flow();
                return Ok(token_resp);
            }
            oauth::PollResult::Pending => {
                // Still waiting, continue
            }
            oauth::PollResult::SlowDown => {
                // Increase interval by 5 seconds
                interval += 5;
                logger::log_info(&format!(
                    "[DeviceFlow] Slow down requested, interval now {}s",
                    interval
                ));
            }
            oauth::PollResult::Expired => {
                cleanup_flow();
                return Err("device_flow_expired".to_string());
            }
            oauth::PollResult::Denied => {
                cleanup_flow();
                return Err("device_flow_access_denied".to_string());
            }
            oauth::PollResult::Error(e) => {
                cleanup_flow();
                return Err(format!("device_flow_poll_error: {}", e));
            }
        }
    }
}

/// Cancel the current device flow
pub fn cancel_device_flow() -> Result<(), String> {
    if let Ok(mut lock) = get_device_flow_state().lock() {
        if let Some(state) = lock.as_mut() {
            state.cancelled = true;
            logger::log_info("[DeviceFlow] Cancelled");
        }
        *lock = None;
    }
    Ok(())
}

/// Check if a device flow is currently active
pub fn is_device_flow_active() -> bool {
    if let Ok(lock) = get_device_flow_state().lock() {
        if let Some(state) = lock.as_ref() {
            let now = chrono::Utc::now().timestamp();
            return !state.cancelled && now < state.expires_at;
        }
    }
    false
}

/// Get current device flow info (if active)
pub fn get_current_device_flow_info() -> Option<DeviceFlowInfo> {
    if let Ok(lock) = get_device_flow_state().lock() {
        if let Some(state) = lock.as_ref() {
            let now = chrono::Utc::now().timestamp();
            if !state.cancelled && now < state.expires_at {
                return Some(DeviceFlowInfo {
                    user_code: state.user_code.clone(),
                    verification_uri: state.verification_uri.clone(),
                    expires_in: state.expires_at - now,
                });
            }
        }
    }
    None
}

fn cleanup_flow() {
    if let Ok(mut lock) = get_device_flow_state().lock() {
        *lock = None;
    }
}
