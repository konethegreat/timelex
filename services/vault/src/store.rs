// Written by Kone Tshivhinda & Claude
//! The vault store: every operation of `contracts/openapi/vault.v1.yaml` except the provider call.
//!
//! Division of labour (ADR-0002): the rules are functions in `timelex-vault`; this file loads what
//! they need, calls them, seals or opens with the key-encryption key, writes the row and appends the
//! audit entry. It adds no rules of its own.
//!
//! Every query runs inside a [`TenantTx`](timelex_db::TenantTx), so Postgres row-level security
//! confines it to one firm. Every write also appends one audit entry in the same transaction, under
//! a row lock on that firm's chain head, so the chain cannot fork.
//!
//! **Nothing here returns a value** except [`VaultStore::open_for_test`], which exists for the
//! provider check and for leases later. The plaintext is wiped when the returned [`SecretValue`]
//! drops, and no method puts it in an error, a log line or a metadata struct.

use std::collections::BTreeSet;
use std::sync::Arc;

use sqlx::postgres::PgRow;
use sqlx::Row;
use time::OffsetDateTime;
use timelex_db::{TenantPool, TenantTx};
use timelex_identity::{require_permission, IdentityError, Permission, TenantId, UserId};
use timelex_vault::{
    entry_hash, prepare, AuditAction, AuditRecord, Binding, Envelope, KeyWrapper, SealEntropy, SecretId,
    SecretKind, SecretValue, DEK_LEN, GENESIS, NONCE_LEN,
};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::StoreError;

/// A source of "now". Injected so tests can move time.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// Who is acting, for operations that need a caller. Built from a verified internal `principal`
/// token; the store trusts nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// The firm.
    pub tenant: TenantId,
    /// The user acting.
    pub user: UserId,
    /// What they may do.
    pub permissions: BTreeSet<Permission>,
}

impl From<&timelex_auth::Principal> for Actor {
    fn from(p: &timelex_auth::Principal) -> Self {
        Self {
            tenant: p.tenant,
            user: p.user,
            permissions: p.permissions.clone(),
        }
    }
}

/// What a secret looks like from outside: metadata only, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMetadata {
    pub id: SecretId,
    pub kind: SecretKind,
    pub label: String,
    /// The last four characters of the value; the only part ever shown.
    pub last_four: String,
    pub allowed_hosts: Vec<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub last_tested_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

/// `PUT /v1/secrets/{kind}`: the new value for a kind. Replaces a live secret of that kind.
///
/// The value is a `String` because it arrives as JSON text. `Debug` hides it, and [`VaultStore::put`]
/// wipes it as soon as the sealed copy exists.
pub struct PutSecretRequest {
    pub label: String,
    pub value: String,
    pub allowed_hosts: Vec<String>,
}

impl std::fmt::Debug for PutSecretRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PutSecretRequest")
            .field("label", &self.label)
            .field("value", &"****")
            .field("allowed_hosts", &self.allowed_hosts)
            .finish()
    }
}

/// Whether a PUT created a new secret (201) or replaced the live one of that kind (200).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutOutcome {
    Created(SecretMetadata),
    Replaced(SecretMetadata),
}

/// An opened secret, for the provider check. Never leaves the process.
pub struct LoadedSecret {
    pub id: SecretId,
    pub kind: SecretKind,
    pub label: String,
    /// Wiped on drop.
    pub value: SecretValue,
}

impl std::fmt::Debug for LoadedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedSecret")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("value", &"****")
            .finish()
    }
}

/// How a provider check ended. The detail is what goes in the audit chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
}

impl TestOutcome {
    fn detail(&self) -> &'static str {
        match self {
            Self::Passed => "ok",
            Self::Failed => "failed",
        }
    }
}

/// The vault store.
#[derive(Clone)]
pub struct VaultStore {
    pool: TenantPool,
    wrapper: Arc<dyn KeyWrapper + Send + Sync>,
    clock: Clock,
}

impl VaultStore {
    /// A store with the system clock and the given key-encryption key.
    pub fn new(pool: TenantPool, wrapper: Arc<dyn KeyWrapper + Send + Sync>) -> Self {
        Self::with_clock(pool, wrapper, Arc::new(OffsetDateTime::now_utc))
    }

    /// A store with an injected clock (for tests).
    pub fn with_clock(pool: TenantPool, wrapper: Arc<dyn KeyWrapper + Send + Sync>, clock: Clock) -> Self {
        Self { pool, wrapper, clock }
    }

    /// Check the database answers. For the health endpoint; reads no tenant data.
    pub async fn ping(&self) -> Result<(), StoreError> {
        Ok(self.pool.ping().await?)
    }

    /// Store a secret of `kind`. A live secret of the same kind is replaced: its old ciphertext and
    /// wrapped data key are overwritten, which destroys them (the row and its metadata stay).
    pub async fn put(
        &self,
        actor: impl Into<Actor>,
        kind: SecretKind,
        req: PutSecretRequest,
    ) -> Result<PutOutcome, StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let PutSecretRequest {
            label,
            mut value,
            allowed_hosts,
        } = req;
        let prepared = prepare(kind, &label, &value, &allowed_hosts);
        value.zeroize();
        let prepared = prepared?;
        let last_four = prepared.last_four();
        let now = self.now();

        for attempt in 0..2 {
            let mut tx = self.pool.begin(actor.tenant).await?;
            let live: Option<(Uuid, OffsetDateTime)> = sqlx::query_as(
                "SELECT id, created_at FROM vault.secrets \
                 WHERE kind = $1 AND revoked_at IS NULL FOR UPDATE",
            )
            .bind(kind.as_str())
            .fetch_optional(tx.conn())
            .await?;

            let id = live.as_ref().map_or_else(Uuid::new_v4, |(id, _)| *id);
            let sealed = self.seal(actor.tenant, SecretId::from_uuid(id), kind, &prepared.value)?;

            if let Some((_, created_at)) = live {
                replace_value(&mut tx, id, &prepared, &sealed, now).await?;
                append_audit(
                    &mut tx,
                    now,
                    AuditAction::Rotated,
                    actor.user,
                    SecretId::from_uuid(id),
                    kind.as_str(),
                )
                .await?;
                tx.commit().await?;
                return Ok(PutOutcome::Replaced(SecretMetadata {
                    id: SecretId::from_uuid(id),
                    kind,
                    label: prepared.label,
                    last_four,
                    allowed_hosts: prepared.allowed_hosts,
                    created_at,
                    updated_at: now,
                    last_tested_at: None,
                    last_used_at: None,
                    revoked_at: None,
                }));
            }

            match insert_secret(
                &mut tx,
                actor.tenant,
                id,
                kind,
                &prepared,
                &sealed,
                actor.user,
                now,
            )
            .await
            {
                Ok(()) => {}
                // A concurrent PUT inserted the live row between our SELECT and INSERT. Its
                // transaction has committed (the unique index waited for it), so try again: the next
                // pass sees the row and replaces it.
                Err(e) if attempt == 0 && is_live_conflict(&e) => continue,
                Err(e) => return Err(e.into()),
            }
            append_audit(
                &mut tx,
                now,
                AuditAction::Created,
                actor.user,
                SecretId::from_uuid(id),
                kind.as_str(),
            )
            .await?;
            tx.commit().await?;
            return Ok(PutOutcome::Created(SecretMetadata {
                id: SecretId::from_uuid(id),
                kind,
                label: prepared.label,
                last_four,
                allowed_hosts: prepared.allowed_hosts,
                created_at: now,
                updated_at: now,
                last_tested_at: None,
                last_used_at: None,
                revoked_at: None,
            }));
        }
        Err(StoreError::Internal(
            "another request kept replacing this kind of secret; try again".into(),
        ))
    }

    /// List this firm's secrets as metadata. Revoked ones are left out unless asked for.
    pub async fn list(
        &self,
        actor: impl Into<Actor>,
        include_revoked: bool,
    ) -> Result<Vec<SecretMetadata>, StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        let rows = sqlx::query(
            "SELECT id, kind, label, last_four, allowed_hosts, created_at, updated_at, \
                    last_tested_at, last_used_at, revoked_at \
             FROM vault.secrets WHERE ($1 OR revoked_at IS NULL) \
             ORDER BY created_at DESC, id",
        )
        .bind(include_revoked)
        .fetch_all(tx.conn())
        .await?;
        rows.iter().map(metadata).collect()
    }

    /// Replace a secret's value, keeping its id, label, kind and hosts. The old value is destroyed.
    pub async fn rotate(
        &self,
        actor: impl Into<Actor>,
        id: SecretId,
        mut value: String,
    ) -> Result<SecretMetadata, StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let now = self.now();
        let mut tx = self.pool.begin(actor.tenant).await?;
        let row = sqlx::query(
            "SELECT kind, label, allowed_hosts, created_at, revoked_at \
             FROM vault.secrets WHERE id = $1 FOR UPDATE",
        )
        .bind(id.as_uuid())
        .fetch_optional(tx.conn())
        .await?;
        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };
        if row.get::<Option<OffsetDateTime>, _>("revoked_at").is_some() {
            return Err(StoreError::Revoked);
        }
        let kind = parse_kind(row.get("kind"))?;
        let label: String = row.get("label");
        let stored_hosts: Vec<String> = row.get("allowed_hosts");
        let prepared = prepare(kind, &label, &value, &stored_hosts);
        value.zeroize();
        let prepared = prepared?;
        let last_four = prepared.last_four();
        let created_at: OffsetDateTime = row.get("created_at");
        let sealed = self.seal(actor.tenant, id, kind, &prepared.value)?;

        replace_value(&mut tx, id.as_uuid(), &prepared, &sealed, now).await?;
        append_audit(&mut tx, now, AuditAction::Rotated, actor.user, id, kind.as_str()).await?;
        tx.commit().await?;

        Ok(SecretMetadata {
            id,
            kind,
            label: prepared.label,
            last_four,
            allowed_hosts: prepared.allowed_hosts,
            created_at,
            updated_at: now,
            last_tested_at: None,
            last_used_at: None,
            revoked_at: None,
        })
    }

    /// Revoke a secret: destroy its value and keep the metadata. Revoking twice succeeds and logs
    /// once, so a retried request cannot tell whether the first attempt worked.
    pub async fn revoke(&self, actor: impl Into<Actor>, id: SecretId) -> Result<(), StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let now = self.now();
        let mut tx = self.pool.begin(actor.tenant).await?;
        let revoked: Option<Uuid> = sqlx::query_scalar(
            "UPDATE vault.secrets \
                SET ciphertext = NULL, value_nonce = NULL, wrapped_dek = NULL, dek_nonce = NULL, \
                    key_version = NULL, revoked_at = $2, revoked_by = $3, updated_at = $2 \
              WHERE id = $1 AND revoked_at IS NULL \
              RETURNING id",
        )
        .bind(id.as_uuid())
        .bind(now)
        .bind(actor.user.as_uuid())
        .fetch_optional(tx.conn())
        .await?;
        if revoked.is_some() {
            let kind = live_kind(&mut tx, id).await?;
            append_audit(&mut tx, now, AuditAction::Revoked, actor.user, id, kind.as_str()).await?;
        } else if !exists(&mut tx, id).await? {
            return Err(StoreError::NotFound);
        }
        tx.commit().await?;
        Ok(())
    }

    /// Open a secret for the provider check. The only way out of the vault in v0, and the plaintext
    /// is wiped on drop. Leases (P1-13) will reuse this.
    pub async fn open_for_test(
        &self,
        actor: impl Into<Actor>,
        id: SecretId,
    ) -> Result<LoadedSecret, StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        let row = sqlx::query(
            "SELECT kind, label, ciphertext, value_nonce, wrapped_dek, dek_nonce, key_version, \
                    revoked_at \
             FROM vault.secrets WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(tx.conn())
        .await?;
        let Some(row) = row else {
            return Err(StoreError::NotFound);
        };
        if row.get::<Option<OffsetDateTime>, _>("revoked_at").is_some() {
            return Err(StoreError::Revoked);
        }
        let kind = parse_kind(row.get("kind"))?;
        let label: String = row.get("label");
        let sealed = sealed_from(&row)?;
        let plaintext = Envelope::open(
            &*self.wrapper,
            Binding {
                tenant: actor.tenant,
                secret: id,
                kind,
            },
            &sealed,
        )
        .map_err(|e| StoreError::Internal(format!("a stored secret would not open: {e}")))?;
        let text = std::str::from_utf8(&plaintext)
            .map_err(|_| StoreError::Internal("a stored secret is not text".into()))?;
        let value = SecretValue::parse(text)
            .map_err(|e| StoreError::Internal(format!("a stored secret is malformed: {e}")))?;
        Ok(LoadedSecret {
            id,
            kind,
            label,
            value,
        })
    }

    /// Record that a provider check ran. Updates `lastTestedAt` and appends the audit entry; it does
    /// **not** touch `lastUsedAt`, because testing a key is not using it.
    pub async fn record_test(
        &self,
        actor: impl Into<Actor>,
        id: SecretId,
        outcome: TestOutcome,
    ) -> Result<(), StoreError> {
        let actor: Actor = actor.into();
        settings_only(&actor)?;
        let now = self.now();
        let mut tx = self.pool.begin(actor.tenant).await?;
        let updated: Option<Uuid> = sqlx::query_scalar(
            "UPDATE vault.secrets SET last_tested_at = $2 \
              WHERE id = $1 AND revoked_at IS NULL RETURNING id",
        )
        .bind(id.as_uuid())
        .bind(now)
        .fetch_optional(tx.conn())
        .await?;
        if updated.is_none() {
            return Err(if exists(&mut tx, id).await? {
                StoreError::Revoked
            } else {
                StoreError::NotFound
            });
        }
        append_audit(
            &mut tx,
            now,
            AuditAction::Tested,
            actor.user,
            id,
            outcome.detail(),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    fn seal(
        &self,
        tenant: TenantId,
        secret: SecretId,
        kind: SecretKind,
        value: &SecretValue,
    ) -> Result<timelex_vault::Sealed, StoreError> {
        let binding = Binding { tenant, secret, kind };
        Ok(Envelope::seal(
            &*self.wrapper,
            binding,
            value.expose_secret().as_bytes(),
            fresh_entropy()?,
        )?)
    }

    fn now(&self) -> OffsetDateTime {
        // Microseconds are Postgres's precision and the audit hash's unit; truncating here means the
        // stored timestamp and the hashed timestamp are the same value.
        let now = (self.clock)();
        now.replace_nanosecond(now.nanosecond() / 1_000 * 1_000)
            .unwrap_or(now)
    }
}

fn settings_only(actor: &Actor) -> Result<(), StoreError> {
    match require_permission(&actor.permissions, Permission::SettingsManage) {
        Ok(()) => Ok(()),
        Err(IdentityError::PermissionDenied(_)) => Err(StoreError::Forbidden),
        Err(e) => Err(StoreError::Internal(e.to_string())),
    }
}

fn fresh_entropy() -> Result<SealEntropy, StoreError> {
    let mut entropy = SealEntropy {
        dek: [0u8; DEK_LEN],
        dek_nonce: [0u8; NONCE_LEN],
        value_nonce: [0u8; NONCE_LEN],
    };
    for bytes in [
        entropy.dek.as_mut_slice(),
        entropy.dek_nonce.as_mut_slice(),
        entropy.value_nonce.as_mut_slice(),
    ] {
        getrandom::fill(bytes).map_err(|e| StoreError::Internal(format!("no randomness: {e}")))?;
    }
    Ok(entropy)
}

fn parse_kind(stored: &str) -> Result<SecretKind, StoreError> {
    SecretKind::parse(stored).ok_or_else(|| StoreError::Internal(format!("stored kind {stored} is unknown")))
}

/// A row with a live secret of this kind already exists (the partial unique index refused the insert).
fn is_live_conflict(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|d| d.constraint())
        .is_some_and(|c| c == "secrets_one_live_kind")
}

fn metadata(row: &PgRow) -> Result<SecretMetadata, StoreError> {
    Ok(SecretMetadata {
        id: SecretId::from_uuid(row.try_get("id")?),
        kind: parse_kind(row.try_get("kind")?)?,
        label: row.try_get("label")?,
        last_four: row.try_get("last_four")?,
        allowed_hosts: row.try_get("allowed_hosts")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_tested_at: row.try_get("last_tested_at")?,
        last_used_at: row.try_get("last_used_at")?,
        revoked_at: row.try_get("revoked_at")?,
    })
}

fn sealed_from(row: &PgRow) -> Result<timelex_vault::Sealed, StoreError> {
    let ciphertext: Option<Vec<u8>> = row.try_get("ciphertext")?;
    let Some(ciphertext) = ciphertext else {
        return Err(StoreError::Internal("a live secret has no ciphertext".into()));
    };
    Ok(timelex_vault::Sealed {
        ciphertext,
        value_nonce: nonce(row.try_get("value_nonce")?)?,
        wrapped_dek: row.try_get("wrapped_dek")?,
        dek_nonce: nonce(row.try_get("dek_nonce")?)?,
        key_version: row.try_get("key_version")?,
    })
}

fn nonce(bytes: Option<Vec<u8>>) -> Result<[u8; NONCE_LEN], StoreError> {
    let bytes = bytes.ok_or_else(|| StoreError::Internal("a live secret has no nonce".into()))?;
    <[u8; NONCE_LEN]>::try_from(bytes.as_slice())
        .map_err(|_| StoreError::Internal("a stored nonce has the wrong length".into()))
}

async fn exists(tx: &mut TenantTx, id: SecretId) -> Result<bool, StoreError> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM vault.secrets WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_optional(tx.conn())
        .await?;
    Ok(found.is_some())
}

async fn live_kind(tx: &mut TenantTx, id: SecretId) -> Result<SecretKind, StoreError> {
    let kind: String = sqlx::query_scalar("SELECT kind FROM vault.secrets WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_one(tx.conn())
        .await?;
    parse_kind(&kind)
}

#[allow(clippy::too_many_arguments)] // one INSERT; a struct would only rename the arguments
async fn insert_secret(
    tx: &mut TenantTx,
    tenant: TenantId,
    id: Uuid,
    kind: SecretKind,
    prepared: &timelex_vault::PreparedSecret,
    sealed: &timelex_vault::Sealed,
    actor: UserId,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO vault.secrets \
           (tenant_id, id, kind, label, last_four, allowed_hosts, ciphertext, value_nonce, \
            wrapped_dek, dek_nonce, key_version, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(tenant.as_uuid())
    .bind(id)
    .bind(kind.as_str())
    .bind(&prepared.label)
    .bind(prepared.last_four())
    .bind(&prepared.allowed_hosts)
    .bind(&sealed.ciphertext)
    .bind(sealed.value_nonce.as_slice())
    .bind(&sealed.wrapped_dek)
    .bind(sealed.dek_nonce.as_slice())
    .bind(&sealed.key_version)
    .bind(actor.as_uuid())
    .bind(now)
    .bind(now)
    .execute(tx.conn())
    .await
    .map(|_| ())
}

/// Overwrite a live row's value. Clears `lastTestedAt` and `lastUsedAt`: the new value has not been
/// tested or used.
async fn replace_value(
    tx: &mut TenantTx,
    id: Uuid,
    prepared: &timelex_vault::PreparedSecret,
    sealed: &timelex_vault::Sealed,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let updated = sqlx::query(
        "UPDATE vault.secrets \
            SET label = $2, last_four = $3, allowed_hosts = $4, ciphertext = $5, value_nonce = $6, \
                wrapped_dek = $7, dek_nonce = $8, key_version = $9, updated_at = $10, \
                last_tested_at = NULL, last_used_at = NULL \
          WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(&prepared.label)
    .bind(prepared.last_four())
    .bind(&prepared.allowed_hosts)
    .bind(&sealed.ciphertext)
    .bind(sealed.value_nonce.as_slice())
    .bind(&sealed.wrapped_dek)
    .bind(sealed.dek_nonce.as_slice())
    .bind(&sealed.key_version)
    .bind(now)
    .execute(tx.conn())
    .await?;
    if updated.rows_affected() != 1 {
        return Err(StoreError::Internal(
            "the secret changed while its value was being replaced".into(),
        ));
    }
    Ok(())
}

/// Append one entry to the firm's audit chain: lock the head, hash, insert, move the head.
async fn append_audit(
    tx: &mut TenantTx,
    at: OffsetDateTime,
    action: AuditAction,
    actor: UserId,
    secret: SecretId,
    detail: &str,
) -> Result<(), StoreError> {
    let tenant = tx.tenant();
    // The first entry for a firm creates the head; the ON CONFLICT makes two first writers safe.
    sqlx::query(
        "INSERT INTO vault.chain_heads (tenant_id, seq, hash) VALUES ($1, 0, $2) \
         ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant.as_uuid())
    .bind(GENESIS.as_slice())
    .execute(tx.conn())
    .await?;
    let head = sqlx::query("SELECT seq, hash FROM vault.chain_heads WHERE tenant_id = $1 FOR UPDATE")
        .bind(tenant.as_uuid())
        .fetch_one(tx.conn())
        .await?;
    let prev_seq: i64 = head.try_get("seq")?;
    let prev: Vec<u8> = head.try_get("hash")?;
    let prev = <[u8; 32]>::try_from(prev.as_slice())
        .map_err(|_| StoreError::Internal("the audit head hash has the wrong length".into()))?;

    let record = AuditRecord {
        seq: prev_seq + 1,
        at_micros: (at.unix_timestamp_nanos() / 1_000) as i64,
        action,
        actor,
        secret,
        detail,
    };
    let hash = entry_hash(&prev, &record);
    sqlx::query(
        "INSERT INTO vault.secret_audit \
           (tenant_id, seq, at, action, actor_id, secret_id, detail, prev_hash, entry_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(tenant.as_uuid())
    .bind(record.seq)
    .bind(at)
    .bind(action.as_str())
    .bind(actor.as_uuid())
    .bind(secret.as_uuid())
    .bind(detail)
    .bind(prev.as_slice())
    .bind(hash.as_slice())
    .execute(tx.conn())
    .await?;
    sqlx::query("UPDATE vault.chain_heads SET seq = $2, hash = $3 WHERE tenant_id = $1")
        .bind(tenant.as_uuid())
        .bind(record.seq)
        .bind(hash.as_slice())
        .execute(tx.conn())
        .await?;
    Ok(())
}
