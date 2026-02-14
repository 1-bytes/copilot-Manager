/// Database module - stubbed out after Copilot migration.
/// Antigravity state.vscdb injection is no longer needed.

use std::path::PathBuf;

/// Get IDE database path (stub - returns error in Copilot mode)
#[allow(dead_code)]
pub fn get_db_path() -> Result<PathBuf, String> {
    Err("db_injection_not_applicable_in_copilot_mode".to_string())
}

/// Inject Token and Email into database (stub - no-op in Copilot mode)
#[allow(dead_code)]
pub fn inject_token(
    _db_path: &PathBuf,
    _access_token: &str,
    _refresh_token: &str,
    _expiry: i64,
    _email: &str,
) -> Result<String, String> {
    Err("token_injection_not_applicable_in_copilot_mode".to_string())
}
