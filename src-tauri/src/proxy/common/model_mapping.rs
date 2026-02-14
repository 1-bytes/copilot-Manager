// Model name mapping for Copilot upstream
use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Legacy alias mappings for Copilot.
///
/// Copilot natively supports standard model names (gpt-4o, claude-sonnet-4, etc.)
/// so most requests pass through as-is. This table only handles legacy aliases
/// that clients may still send.
static LEGACY_ALIASES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // ── OpenAI legacy aliases ───────────────────────────────────────────
    m.insert("gpt-4", "gpt-4o");
    m.insert("gpt-4-turbo", "gpt-4o");
    m.insert("gpt-4-turbo-preview", "gpt-4o");
    m.insert("gpt-4-0125-preview", "gpt-4o");
    m.insert("gpt-4-1106-preview", "gpt-4o");
    m.insert("gpt-4-0613", "gpt-4o");
    m.insert("gpt-4o-2024-05-13", "gpt-4o");
    m.insert("gpt-4o-2024-08-06", "gpt-4o");
    m.insert("gpt-4o-mini-2024-07-18", "gpt-4o-mini");
    m.insert("gpt-3.5-turbo", "gpt-4o-mini");
    m.insert("gpt-3.5-turbo-16k", "gpt-4o-mini");
    m.insert("gpt-3.5-turbo-0125", "gpt-4o-mini");
    m.insert("gpt-3.5-turbo-1106", "gpt-4o-mini");
    m.insert("gpt-3.5-turbo-0613", "gpt-4o-mini");

    // ── Claude legacy / upgrade aliases ─────────────────────────────────
    // Opus isn't available on Copilot; upgrade to best available Sonnet
    m.insert("claude-3-opus", "claude-sonnet-4");
    m.insert("claude-3-opus-20240229", "claude-sonnet-4");
    m.insert("claude-opus-4", "claude-sonnet-4");
    m.insert("claude-opus-4-5", "claude-sonnet-4-5");
    m.insert("claude-opus-4-5-thinking", "claude-sonnet-4-5");
    m.insert("claude-opus-4-6", "claude-sonnet-4-5");
    m.insert("claude-opus-4-6-thinking", "claude-sonnet-4-5");
    // Older Sonnet date-stamped aliases
    m.insert("claude-3-5-sonnet-20241022", "claude-3.5-sonnet");
    m.insert("claude-3-5-sonnet-20240620", "claude-3.5-sonnet");
    // Haiku -> mini Sonnet (Copilot doesn't carry Haiku)
    m.insert("claude-3-haiku", "claude-3.5-sonnet");
    m.insert("claude-3-haiku-20240307", "claude-3.5-sonnet");
    m.insert("claude-haiku-4", "claude-3.5-sonnet");
    m.insert("claude-haiku-4-5-20251001", "claude-3.5-sonnet");

    // ── Gemini legacy aliases ───────────────────────────────────────────
    m.insert("gemini-2.5-flash-thinking", "gemini-2.0-flash");
    m.insert("gemini-2.5-flash-lite", "gemini-2.0-flash");

    // ── Internal virtual model ──────────────────────────────────────────
    m.insert("internal-background-task", "gpt-4o-mini");

    m
});

/// Copilot natively supported models (identity passthrough).
static COPILOT_NATIVE_MODELS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // OpenAI models
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "o1",
        "o1-mini",
        "o3",
        "o3-mini",
        "o4-mini",
        // Anthropic models via Copilot
        "claude-3.5-sonnet",
        "claude-sonnet-4",
        "claude-sonnet-4-5",
        // Google models via Copilot
        "gemini-2.0-flash",
        "gemini-2.5-pro",
    ]
});

/// Map model names for Copilot upstream.
///
/// # Mapping strategy
/// 1. **Exact alias match**: Check LEGACY_ALIASES table
/// 2. **Passthrough**: All other model names are sent as-is to Copilot
///    (Copilot uses standard model names and will return an error for unknown ones)
///
/// # Parameters
/// - `input`: Original model name from the client request
///
/// # Returns
/// The mapped Copilot model name
///
/// # Examples
/// ```ignore
/// // Legacy alias
/// assert_eq!(map_model_to_copilot("gpt-4"), "gpt-4o");
/// assert_eq!(map_model_to_copilot("gpt-3.5-turbo"), "gpt-4o-mini");
///
/// // Native passthrough
/// assert_eq!(map_model_to_copilot("gpt-4o"), "gpt-4o");
/// assert_eq!(map_model_to_copilot("claude-sonnet-4"), "claude-sonnet-4");
///
/// // Unknown model passthrough
/// assert_eq!(map_model_to_copilot("some-future-model"), "some-future-model");
/// ```
pub fn map_model_to_copilot(input: &str) -> String {
    // 1. Check exact alias match
    if let Some(mapped) = LEGACY_ALIASES.get(input) {
        return mapped.to_string();
    }

    // 2. Passthrough: send as-is to Copilot
    input.to_string()
}

/// Backward-compatible alias for callers that still reference the old name.
#[inline]
pub fn map_claude_model_to_gemini(input: &str) -> String {
    map_model_to_copilot(input)
}

/// Get all known model names (built-in aliases + native models)
pub fn get_supported_models() -> Vec<String> {
    let mut models: Vec<String> = LEGACY_ALIASES.keys().map(|s| s.to_string()).collect();
    for m in COPILOT_NATIVE_MODELS.iter() {
        let s = m.to_string();
        if !models.contains(&s) {
            models.push(s);
        }
    }
    models
}

/// Dynamically get all available models (built-in + user custom mappings)
pub async fn get_all_dynamic_models(
    custom_mapping: &tokio::sync::RwLock<std::collections::HashMap<String, String>>,
) -> Vec<String> {
    use std::collections::HashSet;
    let mut model_ids = HashSet::new();

    // 1. Built-in native models
    for m in COPILOT_NATIVE_MODELS.iter() {
        model_ids.insert(m.to_string());
    }

    // 2. All legacy alias source names
    for m in get_supported_models() {
        model_ids.insert(m);
    }

    // 3. User custom mappings
    {
        let mapping = custom_mapping.read().await;
        for key in mapping.keys() {
            model_ids.insert(key.clone());
        }
    }

    let mut sorted_ids: Vec<_> = model_ids.into_iter().collect();
    sorted_ids.sort();
    sorted_ids
}

/// Wildcard matching - supports multiple wildcards
///
/// **Note**: Matching is **case-sensitive**. Pattern `GPT-4*` will NOT match `gpt-4-turbo`.
///
/// Examples:
/// - `gpt-4*` matches `gpt-4`, `gpt-4-turbo`
/// - `claude-*-sonnet-*` matches `claude-3-5-sonnet-20241022`
/// - `*-thinking` matches `claude-opus-4-5-thinking`
/// - `a*b*c` matches `a123b456c`
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();

    // No wildcard - exact match
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut text_pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue; // Skip empty segments from consecutive wildcards
        }

        if i == 0 {
            // First segment must match start
            if !text[text_pos..].starts_with(part) {
                return false;
            }
            text_pos += part.len();
        } else if i == parts.len() - 1 {
            // Last segment must match end
            return text[text_pos..].ends_with(part);
        } else {
            // Middle segments - find next occurrence
            if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }
    }

    true
}

/// Core model routing engine.
/// Priority: exact custom match > wildcard custom match > system default mapping
///
/// # Parameters
/// - `original_model`: Original model name from client
/// - `custom_mapping`: User-defined mapping table
///
/// # Returns
/// The resolved target model name
pub fn resolve_model_route(
    original_model: &str,
    custom_mapping: &std::collections::HashMap<String, String>,
) -> String {
    // 1. Exact custom match (highest priority)
    if let Some(target) = custom_mapping.get(original_model) {
        crate::modules::logger::log_info(&format!("[Router] Exact mapping: {} -> {}", original_model, target));
        return target.clone();
    }

    // 2. Wildcard match - most specific (highest non-wildcard chars) wins
    let mut best_match: Option<(&str, &str, usize)> = None;

    for (pattern, target) in custom_mapping.iter() {
        if pattern.contains('*') && wildcard_match(pattern, original_model) {
            let specificity = pattern.chars().count() - pattern.matches('*').count();
            if best_match.is_none() || specificity > best_match.unwrap().2 {
                best_match = Some((pattern.as_str(), target.as_str(), specificity));
            }
        }
    }

    if let Some((pattern, target, _)) = best_match {
        crate::modules::logger::log_info(&format!(
            "[Router] Wildcard match: {} -> {} (rule: {})",
            original_model, target, pattern
        ));
        return target.to_string();
    }

    // 3. System default mapping (legacy aliases + passthrough)
    let result = map_model_to_copilot(original_model);
    if result != original_model {
        crate::modules::logger::log_info(&format!("[Router] System default mapping: {} -> {}", original_model, result));
    }
    result
}

/// Normalize any model name to a standard protection ID for quota tracking.
///
/// Standard IDs:
/// - `gpt`: All GPT / OpenAI reasoning model variants
/// - `claude`: All Claude / Anthropic variants
/// - `gemini`: All Gemini / Google variants
/// - `o-series`: All OpenAI reasoning models (o1, o3, o4-mini)
///
/// Returns `None` if the model doesn't match any protected category.
pub fn normalize_to_standard_id(model_name: &str) -> Option<String> {
    let lower = model_name.to_lowercase();

    // OpenAI reasoning models (o1, o3, o4-mini)
    if lower.starts_with("o1") || lower.starts_with("o3") || lower.starts_with("o4") {
        return Some("o-series".to_string());
    }

    // GPT models
    if lower.starts_with("gpt-") {
        return Some("gpt".to_string());
    }

    // Claude / Anthropic models
    if lower.contains("claude") || lower.contains("sonnet") || lower.contains("opus") || lower.contains("haiku") {
        return Some("claude".to_string());
    }

    // Gemini / Google models
    if lower.contains("gemini") {
        return Some("gemini".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_mapping() {
        // Legacy aliases
        assert_eq!(map_model_to_copilot("gpt-4"), "gpt-4o");
        assert_eq!(map_model_to_copilot("gpt-3.5-turbo"), "gpt-4o-mini");
        assert_eq!(map_model_to_copilot("claude-3-opus"), "claude-sonnet-4");

        // Native passthrough
        assert_eq!(map_model_to_copilot("gpt-4o"), "gpt-4o");
        assert_eq!(map_model_to_copilot("claude-sonnet-4"), "claude-sonnet-4");
        assert_eq!(map_model_to_copilot("gemini-2.0-flash"), "gemini-2.0-flash");
        assert_eq!(map_model_to_copilot("o3-mini"), "o3-mini");

        // Unknown model passthrough
        assert_eq!(map_model_to_copilot("unknown-model"), "unknown-model");

        // Backward-compat alias
        assert_eq!(map_claude_model_to_gemini("gpt-4"), "gpt-4o");
        assert_eq!(map_claude_model_to_gemini("unknown-model"), "unknown-model");
    }

    #[test]
    fn test_normalize_to_standard_id() {
        assert_eq!(normalize_to_standard_id("gpt-4o"), Some("gpt".to_string()));
        assert_eq!(normalize_to_standard_id("gpt-4.1-mini"), Some("gpt".to_string()));
        assert_eq!(normalize_to_standard_id("claude-sonnet-4"), Some("claude".to_string()));
        assert_eq!(normalize_to_standard_id("claude-3.5-sonnet"), Some("claude".to_string()));
        assert_eq!(normalize_to_standard_id("gemini-2.0-flash"), Some("gemini".to_string()));
        assert_eq!(normalize_to_standard_id("gemini-2.5-pro"), Some("gemini".to_string()));
        assert_eq!(normalize_to_standard_id("o3-mini"), Some("o-series".to_string()));
        assert_eq!(normalize_to_standard_id("o1"), Some("o-series".to_string()));
        assert_eq!(normalize_to_standard_id("o4-mini"), Some("o-series".to_string()));
        assert_eq!(normalize_to_standard_id("random-unknown"), None);
    }

    #[test]
    fn test_wildcard_priority() {
        let mut custom = HashMap::new();
        custom.insert("gpt*".to_string(), "fallback".to_string());
        custom.insert("gpt-4*".to_string(), "specific".to_string());
        custom.insert("claude-opus-*".to_string(), "opus-default".to_string());
        custom.insert("claude-opus*thinking".to_string(), "opus-thinking".to_string());

        // More specific pattern wins
        assert_eq!(resolve_model_route("gpt-4-turbo", &custom), "specific");
        assert_eq!(resolve_model_route("gpt-3.5", &custom), "fallback");
        // Suffix constraint is more specific than prefix-only
        assert_eq!(resolve_model_route("claude-opus-4-5-thinking", &custom), "opus-thinking");
        assert_eq!(resolve_model_route("claude-opus-4", &custom), "opus-default");
    }

    #[test]
    fn test_multi_wildcard_support() {
        let mut custom = HashMap::new();
        custom.insert("claude-*-sonnet-*".to_string(), "sonnet-versioned".to_string());
        custom.insert("gpt-*-*".to_string(), "gpt-multi".to_string());
        custom.insert("*thinking*".to_string(), "has-thinking".to_string());

        assert_eq!(
            resolve_model_route("claude-3-5-sonnet-20241022", &custom),
            "sonnet-versioned"
        );
        assert_eq!(
            resolve_model_route("gpt-4-turbo-preview", &custom),
            "gpt-multi"
        );
        assert_eq!(
            resolve_model_route("claude-thinking-extended", &custom),
            "has-thinking"
        );

        // Negative case: *thinking* should NOT match models without "thinking"
        assert_eq!(
            resolve_model_route("random-model-name", &custom),
            "random-model-name"  // Falls back to system default (passthrough)
        );
    }

    #[test]
    fn test_wildcard_edge_cases() {
        let mut custom = HashMap::new();
        custom.insert("prefix*".to_string(), "prefix-match".to_string());
        custom.insert("*".to_string(), "catch-all".to_string());
        custom.insert("a*b*c".to_string(), "multi-wild".to_string());

        // Specificity: "prefix*" (6) > "*" (0)
        assert_eq!(resolve_model_route("prefix-anything", &custom), "prefix-match");
        // Catch-all has lowest specificity
        assert_eq!(resolve_model_route("random-model", &custom), "catch-all");
        // Multi-wildcard: "a*b*c" (3)
        assert_eq!(resolve_model_route("a-test-b-foo-c", &custom), "multi-wild");
    }
}
