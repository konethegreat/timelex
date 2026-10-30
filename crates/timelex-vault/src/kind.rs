// Written by Kone Tshivhinda & Claude
//! The kinds of secret the vault holds, and how far a key of each kind may reach.
//!
//! A kind is not a free-form tag: each one has a fixed name (stored in the database and used in the
//! API path) and a fixed set of provider hosts. An allowed-host list may narrow that set for one
//! secret, never widen it (ADR-0004 point 6).

/// The kinds of secret the vault accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    /// An Anthropic workspace API key, used by `mcp-host` for model calls.
    AnthropicApiKey,
    /// A Microsoft 365 OAuth refresh token.
    M365Oauth,
    /// A Google OAuth refresh token.
    GoogleOauth,
    /// Any other provider key or token; it must name its hosts explicitly.
    GenericApiKey,
}

impl SecretKind {
    /// Every kind, in the order the API lists them.
    pub const ALL: [SecretKind; 4] = [
        SecretKind::AnthropicApiKey,
        SecretKind::M365Oauth,
        SecretKind::GoogleOauth,
        SecretKind::GenericApiKey,
    ];

    /// The stored name, also the `{kind}` in `PUT /v1/secrets/{kind}`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AnthropicApiKey => "anthropic_api_key",
            Self::M365Oauth => "m365_oauth",
            Self::GoogleOauth => "google_oauth",
            Self::GenericApiKey => "generic_api_key",
        }
    }

    /// Parse a stored name. Case-sensitive: each kind has exactly one spelling.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// A human-readable name for the interface.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AnthropicApiKey => "Anthropic API key",
            Self::M365Oauth => "Microsoft 365 connection",
            Self::GoogleOauth => "Google connection",
            Self::GenericApiKey => "API key",
        }
    }

    /// The hosts a key of this kind may be used against unless an admin narrows the list.
    ///
    /// These are narrow on purpose: the token endpoints and the APIs we call, not whole domains.
    pub fn default_hosts(&self) -> &'static [&'static str] {
        match self {
            Self::AnthropicApiKey => &["api.anthropic.com"],
            Self::M365Oauth => &["login.microsoftonline.com", "graph.microsoft.com"],
            Self::GoogleOauth => &[
                "oauth2.googleapis.com",
                "gmail.googleapis.com",
                "www.googleapis.com",
            ],
            // No default: the admin must say which host the key is for.
            Self::GenericApiKey => &[],
        }
    }

    /// A kind with no default hosts needs an explicit list on every create.
    pub fn requires_explicit_hosts(&self) -> bool {
        self.default_hosts().is_empty()
    }
}

impl std::fmt::Display for SecretKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_stored_name() {
        for kind in SecretKind::ALL {
            assert_eq!(SecretKind::parse(kind.as_str()), Some(kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
    }

    #[test]
    fn stored_names_are_distinct_and_unknown_names_are_rejected() {
        for (i, a) in SecretKind::ALL.iter().enumerate() {
            for b in &SecretKind::ALL[i + 1..] {
                assert_ne!(a.as_str(), b.as_str());
            }
        }
        for bad in ["", "Anthropic_API_Key", "anthropic", "generic", "m365-oauth"] {
            assert_eq!(SecretKind::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn provider_kinds_have_default_hosts_and_generic_does_not() {
        for kind in [
            SecretKind::AnthropicApiKey,
            SecretKind::M365Oauth,
            SecretKind::GoogleOauth,
        ] {
            assert!(!kind.default_hosts().is_empty(), "{kind}");
            assert!(!kind.requires_explicit_hosts(), "{kind}");
        }
        assert!(SecretKind::GenericApiKey.default_hosts().is_empty());
        assert!(SecretKind::GenericApiKey.requires_explicit_hosts());
        assert_eq!(SecretKind::AnthropicApiKey.default_hosts(), ["api.anthropic.com"]);
    }

    #[test]
    fn default_hosts_are_already_lowercase_and_bare() {
        for kind in SecretKind::ALL {
            for host in kind.default_hosts() {
                assert_eq!(*host, host.to_ascii_lowercase(), "{host}");
                assert!(!host.contains("://") && !host.contains('/'), "{host}");
                assert!(host.contains('.'), "{host} is not a fully qualified name");
            }
        }
    }
}
