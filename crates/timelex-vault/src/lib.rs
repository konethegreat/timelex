// Written by Kone Tshivhinda & Claude
//! Vault rules for TimeLex: secret kinds, validation, envelope encryption and the audit chain.
//!
//! This crate has **no I/O**: no database, no network, no clock, and no random numbers. Anything
//! that needs entropy (a data key, two nonces) or the current time is passed in, so every rule is a
//! function a test can call with fixed inputs. The `vault` service supplies the randomness and the
//! database; it never re-implements a rule.
//!
//! ```
//! use timelex_identity::TenantId;
//! use timelex_vault::{Binding, Envelope, LocalKeyWrapper, SealEntropy, SecretId, SecretKind};
//! use uuid::Uuid;
//!
//! let wrapper = LocalKeyWrapper::new([7u8; 32]); // the service loads this from its environment
//! let binding = Binding {
//!     tenant: TenantId::from_uuid(Uuid::from_u128(1)),
//!     secret: SecretId::from_uuid(Uuid::from_u128(2)),
//!     kind: SecretKind::AnthropicApiKey,
//! };
//! let entropy = SealEntropy { dek: [1u8; 32], dek_nonce: [2u8; 12], value_nonce: [3u8; 12] };
//! let sealed = Envelope::seal(&wrapper, binding, b"sk-ant-example", entropy).unwrap();
//! assert_eq!(&Envelope::open(&wrapper, binding, &sealed).unwrap()[..], b"sk-ant-example");
//! ```

mod audit;
mod crypto;
mod error;
mod kind;
mod secret;

pub use audit::{entry_hash, AuditAction, AuditRecord, GENESIS};
pub use crypto::{
    Binding, Envelope, KeyWrapper, LocalKeyWrapper, SealEntropy, Sealed, DEK_LEN, KEY_VERSION, NONCE_LEN,
    TAG_LEN,
};
pub use error::VaultError;
pub use kind::SecretKind;
pub use secret::{
    prepare, resolve_hosts, PreparedSecret, SecretId, SecretValue, MAX_HOSTS, MAX_LABEL_LEN, MAX_VALUE_LEN,
    MIN_VALUE_LEN,
};
