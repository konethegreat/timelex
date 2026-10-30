// Written by Kone Tshivhinda & Claude
//! Envelope encryption (ADR-0004 point 2).
//!
//! Each secret is encrypted with its own data key (AES-256-GCM, 32 bytes, one 96-bit nonce) and
//! that data key is itself encrypted by a key-encryption key held behind the [`KeyWrapper`] trait.
//! In v0 the wrapper is a 32-byte master key from the environment; a cloud KMS or Workload Identity
//! Federation slots in behind the same trait later, without touching stored rows or callers.
//!
//! Two things protect a stored row beyond the keys:
//!
//! * **Binding.** Both the value and the wrapped data key are sealed with additional authenticated
//!   data naming the tenant, the secret id and the kind. A row copied from one firm, or relabelled
//!   as another kind, fails to open rather than being used against the wrong provider.
//! * **One version tag.** `key_version` records which master key wrapped the data key, so a future
//!   key rotation can find rows it still has to re-wrap.
//!
//! This crate never draws randomness: [`Envelope::seal`] takes the data key and both nonces from
//! the caller, and [`Envelope::open`] takes only what was stored. Tests therefore run with fixed
//! bytes, and the service is the one place that talks to the operating system for entropy.

use std::fmt;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use timelex_identity::TenantId;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::VaultError;
use crate::kind::SecretKind;
use crate::secret::SecretId;

/// Bytes in a data key (AES-256).
pub const DEK_LEN: usize = 32;
/// Bytes in a GCM nonce. 96 bits is the size AES-GCM is specified for.
pub const NONCE_LEN: usize = 12;
/// Bytes the GCM tag adds to each sealed value.
pub const TAG_LEN: usize = 16;
/// The master-key version this crate writes. A row with another version was sealed elsewhere.
pub const KEY_VERSION: &str = "v1";

/// What a sealed row looks like: no plaintext, and nothing that names a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The secret value, encrypted, ending with the GCM tag.
    pub ciphertext: Vec<u8>,
    /// Nonce for `ciphertext`.
    pub value_nonce: [u8; NONCE_LEN],
    /// The data key, encrypted by the key-encryption key, ending with the GCM tag.
    pub wrapped_dek: Vec<u8>,
    /// Nonce for `wrapped_dek`.
    pub dek_nonce: [u8; NONCE_LEN],
    /// Which key-encryption key wrapped the data key.
    pub key_version: String,
}

/// The random material one seal needs. The service draws it from the operating system.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SealEntropy {
    /// The per-secret data key.
    pub dek: [u8; DEK_LEN],
    /// Nonce for the wrapped data key.
    pub dek_nonce: [u8; NONCE_LEN],
    /// Nonce for the secret value.
    pub value_nonce: [u8; NONCE_LEN],
}

impl fmt::Debug for SealEntropy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SealEntropy(..)")
    }
}

/// The tenant, secret and kind a sealed value is bound to. Changing any of them makes the value
/// fail to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub tenant: TenantId,
    pub secret: SecretId,
    pub kind: SecretKind,
}

impl Binding {
    /// Additional authenticated data for one purpose. Each field is length-prefixed, so two
    /// different tuples can never produce the same bytes (no separator to inject into).
    fn aad(&self, purpose: &'static [u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(128);
        for part in [
            b"timelex-vault".as_slice(),
            b"envelope-v1".as_slice(),
            purpose,
            self.tenant.as_uuid().as_bytes(),
            self.secret.as_uuid().as_bytes(),
            self.kind.as_str().as_bytes(),
        ] {
            out.extend_from_slice(&(part.len() as u32).to_le_bytes());
            out.extend_from_slice(part);
        }
        out
    }
}

/// Where the key-encryption key lives. The vault service holds one of these; nothing else can
/// unwrap a data key.
pub trait KeyWrapper {
    /// The version tag stored with each row this wrapper seals.
    fn key_version(&self) -> &'static str;

    /// Encrypt a data key with the key-encryption key. The nonce is supplied by the caller.
    fn wrap(&self, dek: &[u8; DEK_LEN], nonce: [u8; NONCE_LEN], aad: &[u8]) -> Result<Vec<u8>, VaultError>;

    /// Decrypt a data key. The result is wiped when dropped.
    fn unwrap_dek(
        &self,
        wrapped: &[u8],
        nonce: [u8; NONCE_LEN],
        aad: &[u8],
    ) -> Result<Zeroizing<[u8; DEK_LEN]>, VaultError>;
}

/// The data-key side of an envelope: sealed values and how to open them.
pub struct Envelope;

impl Envelope {
    /// Seal `plaintext` under a fresh data key wrapped by `wrapper`.
    pub fn seal(
        wrapper: &dyn KeyWrapper,
        binding: Binding,
        plaintext: &[u8],
        entropy: SealEntropy,
    ) -> Result<Sealed, VaultError> {
        let cipher = cipher(&entropy.dek)?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&entropy.value_nonce),
                Payload {
                    msg: plaintext,
                    aad: &binding.aad(b"value"),
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        let wrapped_dek = wrapper.wrap(&entropy.dek, entropy.dek_nonce, &binding.aad(b"dek"))?;
        Ok(Sealed {
            ciphertext,
            value_nonce: entropy.value_nonce,
            wrapped_dek,
            dek_nonce: entropy.dek_nonce,
            key_version: wrapper.key_version().to_owned(),
        })
    }

    /// Open a sealed value. Fails if the row was tampered with, bound to another tenant, secret or
    /// kind, or wrapped by a key this wrapper does not hold.
    pub fn open(
        wrapper: &dyn KeyWrapper,
        binding: Binding,
        sealed: &Sealed,
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        if sealed.key_version != wrapper.key_version() {
            return Err(VaultError::Crypto);
        }
        let dek = wrapper.unwrap_dek(&sealed.wrapped_dek, sealed.dek_nonce, &binding.aad(b"dek"))?;
        let cipher = cipher(&dek[..])?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&sealed.value_nonce),
                Payload {
                    msg: &sealed.ciphertext,
                    aad: &binding.aad(b"value"),
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        Ok(Zeroizing::new(plaintext))
    }
}

fn cipher(key: &[u8]) -> Result<Aes256Gcm, VaultError> {
    Aes256Gcm::new_from_slice(key).map_err(|_| VaultError::Crypto)
}

/// A key-encryption key held in memory, for v0 and for self-hosters who seal the master key in
/// their environment. Not a KMS: it is the smallest wrapper that keeps the same trait shape.
pub struct LocalKeyWrapper {
    master: Zeroizing<[u8; DEK_LEN]>,
}

impl LocalKeyWrapper {
    /// Wrap a 32-byte master key. The caller is responsible for loading it from somewhere safe.
    pub fn new(master: [u8; DEK_LEN]) -> Self {
        Self {
            master: Zeroizing::new(master),
        }
    }
}

impl KeyWrapper for LocalKeyWrapper {
    fn key_version(&self) -> &'static str {
        KEY_VERSION
    }

    fn wrap(&self, dek: &[u8; DEK_LEN], nonce: [u8; NONCE_LEN], aad: &[u8]) -> Result<Vec<u8>, VaultError> {
        cipher(&self.master[..])?
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: dek, aad })
            .map_err(|_| VaultError::Crypto)
    }

    fn unwrap_dek(
        &self,
        wrapped: &[u8],
        nonce: [u8; NONCE_LEN],
        aad: &[u8],
    ) -> Result<Zeroizing<[u8; DEK_LEN]>, VaultError> {
        let bytes = cipher(&self.master[..])?
            .decrypt(Nonce::from_slice(&nonce), Payload { msg: wrapped, aad })
            .map_err(|_| VaultError::Crypto)?;
        let dek = <[u8; DEK_LEN]>::try_from(bytes.as_slice()).map_err(|_| VaultError::Crypto)?;
        Ok(Zeroizing::new(dek))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tenant() -> TenantId {
        TenantId::from_uuid(Uuid::from_u128(11))
    }

    fn secret() -> SecretId {
        SecretId::from_uuid(Uuid::from_u128(22))
    }

    fn binding() -> Binding {
        Binding {
            tenant: tenant(),
            secret: secret(),
            kind: SecretKind::AnthropicApiKey,
        }
    }

    fn entropy() -> SealEntropy {
        SealEntropy {
            dek: [3u8; DEK_LEN],
            dek_nonce: [4u8; NONCE_LEN],
            value_nonce: [5u8; NONCE_LEN],
        }
    }

    fn wrapper() -> LocalKeyWrapper {
        LocalKeyWrapper::new([9u8; DEK_LEN])
    }

    fn seal(w: &LocalKeyWrapper) -> Sealed {
        Envelope::seal(w, binding(), b"sk-ant-api03-roundtrip", entropy()).unwrap()
    }

    #[test]
    fn a_sealed_value_opens_back_to_the_same_bytes() {
        let w = wrapper();
        let sealed = seal(&w);
        let opened = Envelope::open(&w, binding(), &sealed).unwrap();
        assert_eq!(&opened[..], b"sk-ant-api03-roundtrip");
    }

    #[test]
    fn the_ciphertext_holds_none_of_the_plaintext() {
        let sealed = seal(&wrapper());
        let haystack = sealed.ciphertext.windows(4).any(|w| w == b"sk-a");
        assert!(!haystack);
        assert_eq!(sealed.ciphertext.len(), "sk-ant-api03-roundtrip".len() + TAG_LEN);
        assert_eq!(sealed.wrapped_dek.len(), DEK_LEN + TAG_LEN);
        assert_eq!(sealed.key_version, KEY_VERSION);
    }

    #[test]
    fn sealing_is_deterministic_for_the_same_entropy() {
        let w = wrapper();
        assert_eq!(seal(&w), seal(&w));
    }

    #[test]
    fn another_master_key_cannot_open_it() {
        let sealed = seal(&wrapper());
        let other = LocalKeyWrapper::new([10u8; DEK_LEN]);
        assert_eq!(
            Envelope::open(&other, binding(), &sealed).err(),
            Some(VaultError::Crypto)
        );
    }

    #[test]
    fn a_row_bound_to_another_tenant_secret_or_kind_does_not_open() {
        let w = wrapper();
        let sealed = seal(&w);
        let cases = [
            Binding {
                tenant: TenantId::from_uuid(Uuid::from_u128(12)),
                ..binding()
            },
            Binding {
                secret: SecretId::from_uuid(Uuid::from_u128(23)),
                ..binding()
            },
            Binding {
                kind: SecretKind::GenericApiKey,
                ..binding()
            },
        ];
        for b in cases {
            assert_eq!(
                Envelope::open(&w, b, &sealed).err(),
                Some(VaultError::Crypto),
                "{b:?}"
            );
        }
    }

    #[test]
    fn a_tampered_ciphertext_or_wrapped_key_does_not_open() {
        let w = wrapper();
        for tamper in [
            |s: &mut Sealed| s.ciphertext[0] ^= 1,
            |s: &mut Sealed| s.ciphertext.truncate(s.ciphertext.len() - 1),
            |s: &mut Sealed| s.wrapped_dek[0] ^= 1,
            |s: &mut Sealed| s.value_nonce[0] ^= 1,
            |s: &mut Sealed| s.dek_nonce[0] ^= 1,
        ] {
            let mut sealed = seal(&w);
            tamper(&mut sealed);
            assert_eq!(
                Envelope::open(&w, binding(), &sealed).err(),
                Some(VaultError::Crypto)
            );
        }
    }

    #[test]
    fn a_row_from_another_key_version_does_not_open() {
        let w = wrapper();
        let mut sealed = seal(&w);
        sealed.key_version = "v0".to_owned();
        assert_eq!(
            Envelope::open(&w, binding(), &sealed).err(),
            Some(VaultError::Crypto)
        );
    }

    #[test]
    fn an_empty_value_still_seals_and_opens() {
        // The API requires at least 8 characters, but the envelope must not care: a future kind
        // (say a JSON credential bundle) may legitimately be empty.
        let w = wrapper();
        let sealed = Envelope::seal(&w, binding(), b"", entropy()).unwrap();
        assert_eq!(sealed.ciphertext.len(), TAG_LEN);
        assert!(Envelope::open(&w, binding(), &sealed).unwrap().is_empty());
    }

    #[test]
    fn entropy_is_never_printed() {
        let shown = format!("{:?}", entropy());
        assert_eq!(shown, "SealEntropy(..)");
        let sealed = format!("{:?}", seal(&wrapper()));
        assert!(!sealed.contains("sk-ant-api03-roundtrip"));
    }

    #[test]
    fn aad_names_every_field_and_differs_by_purpose() {
        let b = binding();
        let value = b.aad(b"value");
        let dek = b.aad(b"dek");
        assert_ne!(value, dek, "value and data-key seals must not share AAD");
        for part in [
            tenant().as_uuid().as_bytes().as_slice(),
            secret().as_uuid().as_bytes().as_slice(),
            SecretKind::AnthropicApiKey.as_str().as_bytes(),
        ] {
            assert!(value.windows(part.len()).any(|w| w == part));
        }
    }
}
