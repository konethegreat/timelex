// Written by Kone Tshivhinda & Claude
//! Field-name redaction and free-text scrubbing for log records.

/// Field names whose values never reach a log sink. Matching is case-insensitive and treats `-`,
/// `.` and a space as `_`, so `Authorization`, `api-key` and `private.key` are all caught.
const SENSITIVE: &[&str] = &[
    "authorization",
    "bearer",
    "cookie",
    "credential",
    "api_key",
    "token",
    "secret",
    "password",
    "value",
    "master_key",
    "signing_key",
    "private_key",
];

pub(crate) const REDACTED: &str = "[redacted]";

/// Whether a field name carries a credential or a stored secret value.
pub(crate) fn is_sensitive(field: &str) -> bool {
    let normalized = field.to_ascii_lowercase().replace(['-', '.', ' '], "_");
    SENSITIVE
        .iter()
        .any(|name| normalized == *name || normalized.ends_with(&format!("_{name}")))
}

/// Replace anything that follows `Bearer ` with a marker, so a token pasted into a message cannot
/// escape. Field-name redaction is the primary rule; this is the backstop for free text.
pub(crate) fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(position) = rest.to_ascii_lowercase().find("bearer ") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..position]);
        out.push_str("Bearer ");
        out.push_str(REDACTED);
        let after = &rest[position + "bearer ".len()..];
        let end = after
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '}' | ']'))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_names_are_matched_case_and_separator_insensitively() {
        for name in [
            "token",
            "API_KEY",
            "authorization",
            "Master-Key",
            "refresh_token",
            "client_secret",
            "password",
            "value",
            "Cookie",
            "set-cookie",
            "private.key",
            "bearer",
        ] {
            assert!(is_sensitive(name), "{name} should be sensitive");
        }
    }

    #[test]
    fn ordinary_names_are_kept() {
        for name in [
            "route",
            "method",
            "status",
            "last_four",
            "tenant_id",
            "kind",
            "secret_id",
            "message",
        ] {
            assert!(!is_sensitive(name), "{name} should not be sensitive");
        }
    }

    #[test]
    fn bearer_credentials_in_free_text_are_scrubbed() {
        let text = "called with Bearer not-a-real-token and then stopped";
        let scrubbed = scrub(text);
        assert!(!scrubbed.contains("not-a-real-token"));
        assert!(scrubbed.contains("Bearer [redacted]"));
        assert!(scrubbed.contains("and then stopped"));
    }

    #[test]
    fn scrubbing_is_case_insensitive_and_handles_quotes() {
        let scrubbed = scrub("header was \"BEARER abc.def\", done");
        assert!(!scrubbed.contains("abc.def"));
        assert!(scrubbed.contains("done"));
    }
}
