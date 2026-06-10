//! Shared wire-id sanitizer for optimize-run host projections.
//!
//! Run ids, candidate ids, case fragments, and revision labels all flow into
//! wire-visible identifiers that must stay within `[A-Za-z0-9_-]`. This is the
//! single owning concept for that mapping so the allowed character set and the
//! empty sentinel cannot silently diverge across the lowering, worker, and
//! projection submodules.

/// Maps `value` to a wire-safe token: ascii-alphanumeric, `_`, and `-` survive;
/// every other character becomes `_`. An empty result collapses to `"anon"` so
/// the token is never blank.
pub(super) fn sanitize_token(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "anon".to_owned()
    } else {
        cleaned
    }
}

/// Sanitizes `value` and guarantees a single leading `{prefix}_`. When `value`
/// already starts with `{prefix}_` the prefix is preserved through
/// sanitization rather than doubled.
pub(super) fn sanitize_with_prefix(prefix: &str, value: &str) -> String {
    if value.starts_with(&format!("{prefix}_")) {
        sanitize_token(value)
    } else {
        format!("{prefix}_{}", sanitize_token(value))
    }
}
