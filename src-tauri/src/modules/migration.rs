use crate::models::{AccountExportResponse, TokenData};
use crate::modules::{account, logger};

/// Import accounts from exported data.
/// In the new Copilot model, AccountExportItem contains `github_token`.
/// Imported accounts are saved with the token; the UI should prompt re-auth if needed.
pub fn import_accounts_from_export(response: &AccountExportResponse) -> Result<ImportStats, String> {
    let mut stats = ImportStats::default();

    for item in &response.accounts {
        stats.total += 1;

        let github_token = &item.github_token;
        if github_token.is_empty() {
            stats.skipped += 1;
            stats.details.push(format!("{}: empty github_token, skipped", item.email));
            continue;
        }

        let token = TokenData::new(github_token.clone(), None, 0);

        match account::upsert_account(item.email.clone(), None, token) {
            Ok(_acc) => {
                stats.imported += 1;
                logger::log_info(&format!("Imported account: {}", item.email));
            }
            Err(e) => {
                stats.failed += 1;
                stats.details.push(format!("{}: {}", item.email, e));
                logger::log_error(&format!("Failed to import {}: {}", item.email, e));
            }
        }
    }

    logger::log_info(&format!(
        "Migration import complete: {} total, {} imported, {} failed, {} skipped",
        stats.total, stats.imported, stats.failed, stats.skipped
    ));

    Ok(stats)
}

/// Import a single account from a github token string
pub fn import_single_token(email: &str, github_token: &str) -> Result<(), String> {
    let token = TokenData::new(github_token.to_string(), None, 0);
    let _acc = account::upsert_account(email.to_string(), None, token)?;
    logger::log_info(&format!("Single token import: {}", email));
    Ok(())
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ImportStats {
    pub total: usize,
    pub imported: usize,
    pub failed: usize,
    pub skipped: usize,
    pub details: Vec<String>,
}
