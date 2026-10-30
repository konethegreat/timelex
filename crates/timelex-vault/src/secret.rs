// Written by Kone Tshivhinda & Claude
//! What a secret is, and the checks a new secret must pass before it is sealed.
//!
//! A secret value exists in memory only while it is being sealed or used. The wrapper below wipes
//! its text on drop, has no `Display`, and hides the text from `Debug`, so the value cannot reach a
//! log by accident. Anything that may be shown (the last four characters, the label, the hosts) is
//! produced here, once, rather than by slicing the value wherever it is needed.

use std::fmt;

use timelex_identity::validate_name;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::VaultError;
use crate::kind::SecretKind;

/// Shortest accepted secret value. Anything shorter is almost certainly a typo.
pub const MIN_VALUE_LEN: usize = 8;
/// Longest accepted secret value: an OAuth refresh token is well under this.
pub const MAX_VALUE_LEN: usize = 4096;
/// Longest accepted label, in characters.
pub const MAX_LABEL_LEN: usize = 120;
/// Most allowed hosts one secret may name.
pub const MAX_HOSTS: usize = 16;

/// Identifier of one stored secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretId(Uuid);

impl SecretId {
    /// Wrap an existing UUID. The service creates new ids; this crate never draws randomness.
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The underlying UUID (for database parameters).
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

/// A secret value in memory. Wiped on drop; never printed.
pub struct SecretValue {
    text: Zeroizing<String>,
}

impl SecretValue {
    /// Check that `s` can be a secret value at all: 8 to 4096 bytes of printable ASCII with no
    /// spaces. That covers API keys and OAuth tokens and excludes anything that would corrupt a
    /// log line or a configuration file.
    pub fn parse(s: &str) -> Result<Self, VaultError> {
        let b = s.as_bytes();
        if b.len() < MIN_VALUE_LEN {
            return Err(VaultError::InvalidValue("the value is shorter than 8 characters"));
        }
        if b.len() > MAX_VALUE_LEN {
            return Err(VaultError::InvalidValue(
                "the value is longer than 4096 characters",
            ));
        }
        if !b.iter().all(u8::is_ascii_graphic) {
            return Err(VaultError::InvalidValue(
                "the value must be printable ASCII with no spaces",
            ));
        }
        Ok(Self {
            text: Zeroizing::new(s.to_owned()),
        })
    }

    /// The value. Deliberately awkward to call: only sealing and the provider check may see it.
    pub fn expose_secret(&self) -> &str {
        &self.text
    }

    /// The last four characters, the only part of a value the API ever shows.
    pub fn last_four(&self) -> String {
        self.text[self.text.len() - 4..].to_owned()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretValue(****)")
    }
}

/// A new secret that passed every check.
pub struct PreparedSecret {
    pub kind: SecretKind,
    /// Trimmed label, 1 to 120 characters.
    pub label: String,
    pub value: SecretValue,
    /// Lowercase host names, already checked to be inside the kind's provider hosts.
    pub allowed_hosts: Vec<String>,
}

impl PreparedSecret {
    /// The last four characters of the value.
    pub fn last_four(&self) -> String {
        self.value.last_four()
    }
}

/// Validate a new secret. On success the caller has everything needed to seal it; on failure the
/// error says what is wrong with the input and never repeats the value.
///
/// `allowed_hosts` may be empty for a provider kind (the kind's defaults apply) but not for
/// `generic_api_key`, which has no defaults.
pub fn prepare(
    kind: SecretKind,
    label: &str,
    value: &str,
    allowed_hosts: &[String],
) -> Result<PreparedSecret, VaultError> {
    let label = validate_name(label, MAX_LABEL_LEN).map_err(|_| VaultError::InvalidLabel)?;
    let value = SecretValue::parse(value)?;
    kind_check(kind, &value)?;
    let allowed_hosts = resolve_hosts(kind, allowed_hosts)?;
    Ok(PreparedSecret {
        kind,
        label,
        value,
        allowed_hosts,
    })
}

/// Checks that only make sense for one kind. Kept apart from the general shape check so the reason
/// a key was refused is as specific as it can be without echoing the key.
fn kind_check(kind: SecretKind, value: &SecretValue) -> Result<(), VaultError> {
    if kind != SecretKind::AnthropicApiKey {
        return Ok(());
    }
    let text = value.expose_secret();
    if text.starts_with("sk-ant-admin") {
        return Err(VaultError::InvalidValue(
            "admin keys are not accepted; use a workspace key",
        ));
    }
    if !text.starts_with("sk-ant-") {
        return Err(VaultError::InvalidValue("an Anthropic key starts with sk-ant-"));
    }
    Ok(())
}

/// Work out the host list a secret will be limited to. An empty request means the kind's defaults;
/// a non-empty one must stay inside them.
pub fn resolve_hosts(kind: SecretKind, requested: &[String]) -> Result<Vec<String>, VaultError> {
    if requested.len() > MAX_HOSTS {
        return Err(VaultError::InvalidHosts("at most 16 hosts"));
    }
    if requested.is_empty() {
        if kind.requires_explicit_hosts() {
            return Err(VaultError::InvalidHosts(
                "a generic key needs at least one allowed host",
            ));
        }
        return Ok(kind.default_hosts().iter().map(|h| (*h).to_owned()).collect());
    }
    for (i, host) in requested.iter().enumerate() {
        check_host(host)?;
        if requested[..i].contains(host) {
            return Err(VaultError::InvalidHosts("the list contains a duplicate"));
        }
        // A kind with no provider hosts has nothing to widen, so any valid host is allowed.
        if !kind.requires_explicit_hosts() && !kind.default_hosts().iter().any(|d| within(host, d)) {
            return Err(VaultError::InvalidHost {
                host: host.clone(),
                reason: "is not one of this kind's provider hosts",
            });
        }
    }
    Ok(requested.to_vec())
}

/// Whether `host` is `provider` or a subdomain of it. A trailing `.evil.example` cannot pass.
fn within(host: &str, provider: &str) -> bool {
    host == provider
        || host
            .strip_suffix(provider)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// A bare DNS name: lowercase, at least two labels, no scheme, port, path or wildcard.
fn check_host(host: &str) -> Result<(), VaultError> {
    let refuse = |reason: &'static str| VaultError::InvalidHost {
        host: host.to_owned(),
        reason,
    };
    if host.is_empty() || host.len() > 253 {
        return Err(refuse("must be 1 to 253 characters"));
    }
    if host != host.to_ascii_lowercase() {
        return Err(refuse("must be lowercase"));
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return Err(refuse("must be a fully qualified name like api.example.com"));
    }
    for label in labels {
        let b = label.as_bytes();
        if b.is_empty() || b.len() > 63 {
            return Err(refuse("has an empty or overlong part between the dots"));
        }
        if b[0] == b'-' || b[b.len() - 1] == b'-' {
            return Err(refuse("has a part that starts or ends with a hyphen"));
        }
        if !b
            .iter()
            .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(refuse("may only contain lowercase letters, digits and hyphens"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn good_secrets_prepare_with_kind_defaults() {
        let p = prepare(
            SecretKind::AnthropicApiKey,
            "  Model key  ",
            "sk-ant-api03-abcdefgh",
            &[],
        )
        .unwrap();
        assert_eq!(p.label, "Model key", "the label is trimmed");
        assert_eq!(p.allowed_hosts, ["api.anthropic.com"]);
        assert_eq!(p.last_four(), "efgh");
    }

    #[test]
    fn the_value_has_a_shape_every_kind_shares() {
        let cases = [
            ("short", "abc1234"),
            ("long", &"a".repeat(MAX_VALUE_LEN + 1)),
            ("spaces", "abc def ghi"),
            ("tab", "abcdefgh\t"),
            ("non-ascii", "abcdefgé"),
            ("newline", "abcdefgh\n"),
        ];
        for (label, value) in cases {
            let e = prepare(SecretKind::GenericApiKey, "k", value, &[host("api.example.test")]);
            assert!(
                matches!(e, Err(VaultError::InvalidValue(_))),
                "{label}: {:?}",
                e.err()
            );
        }
        // The shortest accepted value is exactly eight printable characters.
        assert!(prepare(
            SecretKind::GenericApiKey,
            "k",
            "abcdefgh",
            &[host("api.example.test")]
        )
        .is_ok());
    }

    #[test]
    fn anthropic_keys_must_look_like_workspace_keys() {
        for (value, expected) in [
            ("sk-ant-api03-abcdefgh", None),
            (
                "AKIAIOSFODNN7EXAMPLE",
                Some("an Anthropic key starts with sk-ant-"),
            ),
            (
                "sk-ant-admin01-abcdefgh",
                Some("admin keys are not accepted; use a workspace key"),
            ),
        ] {
            let got = prepare(SecretKind::AnthropicApiKey, "k", value, &[]).err();
            match (got, expected) {
                (None, None) => {}
                (Some(VaultError::InvalidValue(reason)), Some(want)) => assert_eq!(reason, want),
                (other, _) => panic!("{value}: {other:?}"),
            }
        }
    }

    #[test]
    fn labels_are_trimmed_checked_and_bounded() {
        assert!(matches!(
            prepare(SecretKind::AnthropicApiKey, "   ", "sk-ant-abcdefgh", &[]),
            Err(VaultError::InvalidLabel)
        ));
        assert!(matches!(
            prepare(
                SecretKind::AnthropicApiKey,
                &"a".repeat(MAX_LABEL_LEN + 1),
                "sk-ant-abcdefgh",
                &[]
            ),
            Err(VaultError::InvalidLabel)
        ));
        assert!(matches!(
            prepare(
                SecretKind::AnthropicApiKey,
                "bad\u{7}name",
                "sk-ant-abcdefgh",
                &[]
            ),
            Err(VaultError::InvalidLabel)
        ));
        assert!(prepare(
            SecretKind::AnthropicApiKey,
            &"a".repeat(MAX_LABEL_LEN),
            "sk-ant-abcdefgh",
            &[]
        )
        .is_ok());
    }

    #[test]
    fn hosts_may_narrow_the_kind_but_never_widen_it() {
        let k = SecretKind::AnthropicApiKey;
        let narrow = resolve_hosts(k, &[host("eu.api.anthropic.com")]).unwrap();
        assert_eq!(narrow, ["eu.api.anthropic.com"]);
        for bad in ["api.openai.com", "anthropic.com", "example.test"] {
            assert!(
                matches!(
                    resolve_hosts(k, &[host(bad)]),
                    Err(VaultError::InvalidHost { .. })
                ),
                "{bad}"
            );
        }
        // A name that merely ends with the provider name as text is not a subdomain.
        assert!(resolve_hosts(k, &[host("notapi.anthropic.com.evil.test")]).is_err());
    }

    #[test]
    fn host_names_must_be_bare_lowercase_dns_names() {
        let g = SecretKind::GenericApiKey;
        let ok = [
            "api.example.test",
            "1.example.test",
            "a-b.c-d.test",
            "xn--bcher-kva.example.test",
        ];
        for h in ok {
            assert!(resolve_hosts(g, &[host(h)]).is_ok(), "{h}");
        }
        let bad = [
            "",
            "api",
            "API.example.test",
            "https://api.example.test",
            "api.example.test:443",
            "api.example.test/path",
            "*.example.test",
            "-api.example.test",
            "api-.example.test",
            "api..example.test",
            ".api.example.test",
            "api.example.test.",
            "api example.test",
            "api.exämple.test",
        ];
        for h in bad {
            assert!(
                matches!(resolve_hosts(g, &[host(h)]), Err(VaultError::InvalidHost { .. })),
                "{h:?}"
            );
        }
        let long_label = format!("{}.example.test", "a".repeat(64));
        assert!(resolve_hosts(g, &[host(&long_label)]).is_err());
    }

    #[test]
    fn generic_keys_must_name_their_hosts_and_lists_are_bounded() {
        assert!(matches!(
            resolve_hosts(SecretKind::GenericApiKey, &[]),
            Err(VaultError::InvalidHosts(_))
        ));
        let duplicate = [host("api.example.test"), host("api.example.test")];
        assert!(matches!(
            resolve_hosts(SecretKind::GenericApiKey, &duplicate),
            Err(VaultError::InvalidHosts(_))
        ));
        let too_many: Vec<String> = (0..=MAX_HOSTS).map(|i| format!("h{i}.example.test")).collect();
        assert!(matches!(
            resolve_hosts(SecretKind::GenericApiKey, &too_many),
            Err(VaultError::InvalidHosts(_))
        ));
        let just_right: Vec<String> = (0..MAX_HOSTS).map(|i| format!("h{i}.example.test")).collect();
        assert_eq!(
            resolve_hosts(SecretKind::GenericApiKey, &just_right)
                .unwrap()
                .len(),
            MAX_HOSTS
        );
    }

    #[test]
    fn last_four_is_the_tail_of_the_value_and_debug_shows_nothing() {
        let p = prepare(SecretKind::M365Oauth, "mail", "0.AXoA-refresh-token-9f3d", &[]).unwrap();
        assert_eq!(p.last_four(), "9f3d");
        let shown = format!("{:?}", p.value);
        assert_eq!(shown, "SecretValue(****)");
        assert!(!shown.contains("9f3d"));
    }

    #[test]
    fn ids_display_as_hyphenated_uuids() {
        let id = SecretId::from_uuid(Uuid::parse_str("0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b").unwrap());
        assert_eq!(id.to_string(), "0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b");
        assert_eq!(SecretId::from_uuid(id.as_uuid()), id);
    }
}
