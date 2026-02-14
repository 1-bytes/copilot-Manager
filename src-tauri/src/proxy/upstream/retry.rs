// 429 retry strategy
// Parses retry delay from Copilot / standard HTTP error responses

use regex::Regex;
use once_cell::sync::Lazy;

static DURATION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([\d.]+)\s*(ms|s|m|h)").unwrap()
});

/// Parse a duration string (e.g., "1.5s", "200ms", "1h16m0.667s")
pub fn parse_duration_ms(duration_str: &str) -> Option<u64> {
    let mut total_ms: f64 = 0.0;
    let mut matched = false;

    for cap in DURATION_RE.captures_iter(duration_str) {
        matched = true;
        let value: f64 = cap[1].parse().ok()?;
        let unit = &cap[2];

        match unit {
            "ms" => total_ms += value,
            "s" => total_ms += value * 1000.0,
            "m" => total_ms += value * 60.0 * 1000.0,
            "h" => total_ms += value * 60.0 * 60.0 * 1000.0,
            _ => {}
        }
    }

    if !matched {
        return None;
    }

    Some(total_ms.round() as u64)
}

/// Extract retry delay (in milliseconds) from an error response body.
///
/// Supports multiple error formats:
///
/// 1. **Copilot / OpenAI format**: `{ "error": { "message": "...", "type": "..." } }`
///    with optional `retry_after` or `retry-after` field in the error object.
///
/// 2. **Standard Retry-After as integer seconds**: When the body contains a
///    top-level `retry_after` field with a numeric value (seconds).
///
/// 3. **Generic `details` array with RetryInfo**: Any error body containing
///    `error.details[].retryDelay` (covers various API providers).
///
/// 4. **Plain numeric body**: If the body is just a number, treat as seconds.
pub fn parse_retry_delay(error_text: &str) -> Option<u64> {
    use serde_json::Value;

    // Try parsing as JSON first
    if let Ok(json) = serde_json::from_str::<Value>(error_text) {
        // ── Strategy 1: Top-level retry_after (numeric seconds) ────────
        if let Some(retry_after) = json.get("retry_after")
            .or_else(|| json.get("retry-after"))
            .or_else(|| json.get("Retry-After"))
        {
            if let Some(secs) = retry_after.as_f64() {
                return Some((secs * 1000.0).round() as u64);
            }
            if let Some(s) = retry_after.as_str() {
                // Could be a duration string like "1.5s" or a plain number
                if let Ok(secs) = s.parse::<f64>() {
                    return Some((secs * 1000.0).round() as u64);
                }
                return parse_duration_ms(s);
            }
        }

        // ── Strategy 2: error.retry_after ──────────────────────────────
        if let Some(error_obj) = json.get("error") {
            if let Some(retry_after) = error_obj.get("retry_after")
                .or_else(|| error_obj.get("retry-after"))
                .or_else(|| error_obj.get("Retry-After"))
            {
                if let Some(secs) = retry_after.as_f64() {
                    return Some((secs * 1000.0).round() as u64);
                }
                if let Some(s) = retry_after.as_str() {
                    if let Ok(secs) = s.parse::<f64>() {
                        return Some((secs * 1000.0).round() as u64);
                    }
                    return parse_duration_ms(s);
                }
            }

            // ── Strategy 3: error.details[] with RetryInfo ─────────────
            if let Some(details) = error_obj.get("details").and_then(|v| v.as_array()) {
                for detail in details {
                    // RetryInfo.retryDelay
                    if let Some(type_str) = detail.get("@type").and_then(|v| v.as_str()) {
                        if type_str.contains("RetryInfo") {
                            if let Some(retry_delay) = detail.get("retryDelay").and_then(|v| v.as_str()) {
                                return parse_duration_ms(retry_delay);
                            }
                        }
                    }

                    // metadata.quotaResetDelay
                    if let Some(quota_delay) = detail
                        .get("metadata")
                        .and_then(|m| m.get("quotaResetDelay"))
                        .and_then(|v| v.as_str())
                    {
                        return parse_duration_ms(quota_delay);
                    }
                }
            }
        }
    }

    // ── Strategy 4: Plain numeric body (seconds) ───────────────────────
    if let Ok(secs) = error_text.trim().parse::<f64>() {
        if secs > 0.0 && secs < 3600.0 {
            return Some((secs * 1000.0).round() as u64);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_ms() {
        assert_eq!(parse_duration_ms("1.5s"), Some(1500));
        assert_eq!(parse_duration_ms("200ms"), Some(200));
        assert_eq!(parse_duration_ms("1h16m0.667s"), Some(4560667));
        assert_eq!(parse_duration_ms("invalid"), None);
    }

    #[test]
    fn test_parse_retry_delay_copilot_format() {
        // Copilot-style: top-level retry_after in seconds
        let body = r#"{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error"}, "retry_after": 5}"#;
        assert_eq!(parse_retry_delay(body), Some(5000));

        // retry_after inside error object
        let body = r#"{"error": {"message": "Rate limit exceeded", "retry_after": 2.5}}"#;
        assert_eq!(parse_retry_delay(body), Some(2500));
    }

    #[test]
    fn test_parse_retry_delay_standard_header_in_body() {
        // Some APIs echo the Retry-After header value in the JSON body
        let body = r#"{"Retry-After": "10"}"#;
        assert_eq!(parse_retry_delay(body), Some(10000));
    }

    #[test]
    fn test_parse_retry_delay_details_array() {
        // Legacy format with details array (backward compat)
        let error_json = r#"{
            "error": {
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.RetryInfo",
                    "retryDelay": "1.203608125s"
                }]
            }
        }"#;
        assert_eq!(parse_retry_delay(error_json), Some(1204));
    }

    #[test]
    fn test_parse_retry_delay_plain_number() {
        assert_eq!(parse_retry_delay("30"), Some(30000));
        assert_eq!(parse_retry_delay("1.5"), Some(1500));
    }

    #[test]
    fn test_parse_retry_delay_no_match() {
        assert_eq!(parse_retry_delay("just some error text"), None);
        assert_eq!(parse_retry_delay(r#"{"error": {"message": "bad request"}}"#), None);
    }
}
