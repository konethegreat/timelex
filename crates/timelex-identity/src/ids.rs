// Written by Kone Tshivhinda & Claude
//! Typed identifiers. A `TenantId` and a `UserId` are both UUIDs underneath, but wrapping each in
//! its own type means the compiler stops you passing one where the other is expected.

use uuid::Uuid;

// A macro writes the same code once per identifier type, so the three types cannot drift apart.
// `$name` is replaced by each type name below. (macro_rules! is plain text substitution.)
macro_rules! id_type {
    ($name:ident, $what:literal) => {
        #[doc = concat!("Identifier of ", $what, ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// Wrap an existing UUID. The service layer creates new ids; this crate never
            /// generates randomness, so every function here gives the same output for the same input.
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// The underlying UUID (for database parameters).
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0.hyphenated())
            }
        }
    };
}

id_type!(TenantId, "a tenant (a firm)");
id_type!(UserId, "a user (a person or a machine)");
id_type!(ApiKeyId, "an API key");

impl TenantId {
    /// 32 lowercase hex characters, as used inside API-key tokens.
    pub fn to_hex32(&self) -> String {
        self.0.simple().to_string()
    }

    /// Parse exactly 32 lowercase hex characters. Anything else is rejected, so each tenant has
    /// exactly one valid spelling inside a token.
    pub fn from_hex32(s: &str) -> Option<Self> {
        if s.len() != 32
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        Uuid::try_parse(s).ok().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: &str = "0191a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

    #[test]
    fn hex32_round_trips() {
        let id = TenantId::from_uuid(Uuid::parse_str(U).unwrap());
        let hex = id.to_hex32();
        assert_eq!(hex, "0191a2b3c4d57e6f8a9b0c1d2e3f4a5b");
        assert_eq!(TenantId::from_hex32(&hex), Some(id));
    }

    #[test]
    fn hex32_rejects_every_other_spelling() {
        assert_eq!(TenantId::from_hex32(U), None, "hyphenated form");
        assert_eq!(
            TenantId::from_hex32("0191A2B3C4D57E6F8A9B0C1D2E3F4A5B"),
            None,
            "uppercase"
        );
        assert_eq!(
            TenantId::from_hex32("0191a2b3c4d57e6f8a9b0c1d2e3f4a5"),
            None,
            "31 characters"
        );
        assert_eq!(
            TenantId::from_hex32("0191a2b3c4d57e6f8a9b0c1d2e3f4a5g"),
            None,
            "non-hex"
        );
        assert_eq!(TenantId::from_hex32(""), None);
    }

    #[test]
    fn display_is_hyphenated() {
        let id = UserId::from_uuid(Uuid::parse_str(U).unwrap());
        assert_eq!(id.to_string(), U);
    }
}
