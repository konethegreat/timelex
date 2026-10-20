// Written by Kone Tshivhinda & Claude
//! The identity store: the four operations of `contracts/openapi/identity.v1.yaml`.
//!
//! Division of labour (ADR-0002): every *rule* is a function in `timelex-identity`; this file loads the
//! facts those rules need, calls them, and writes the result. It adds no rules of its own. All queries
//! run inside a [`TenantTx`](timelex_db::TenantTx), so Postgres row-level security confines each
//! operation to one firm.

use std::collections::BTreeSet;
use std::sync::Arc;

use sqlx::Row;
use time::OffsetDateTime;
use timelex_db::TenantPool;
use timelex_identity::{
    check_key_issuance, check_key_usable, effective_permissions, require_permission, validate_new_tenant,
    ApiKeyId, ApiKeyToken, KeyHash, KeyIssuanceRequest, KeyState, NewTenantRequest, Permission, RoleSet,
    TenantId, TenantSlug, TenantStatus, UserId, UserStatus,
};
use uuid::Uuid;

use crate::error::{AuthError, StoreError};

/// How often `last_used_at` is refreshed at most, so authenticating does not write on every request.
const LAST_USED_REFRESH_SECONDS: i64 = 60;

/// A source of "now". Injected so tests can move time.
pub type Clock = Arc<dyn Fn() -> OffsetDateTime + Send + Sync>;

/// A key just created. The token is available only here, once.
#[derive(Debug)]
pub struct IssuedKey {
    /// Key id (not secret).
    pub id: ApiKeyId,
    /// The user the key acts as.
    pub user_id: UserId,
    /// Label.
    pub name: String,
    /// The full secret. Show it to the creator and drop it.
    pub token: ApiKeyToken,
    /// What the key may do.
    pub scopes: BTreeSet<Permission>,
    /// Expiry, if any.
    pub expires_at: Option<OffsetDateTime>,
    /// Creation time.
    pub created_at: OffsetDateTime,
}

/// A firm that was just created.
#[derive(Debug)]
pub struct CreatedTenant {
    /// The new firm's id.
    pub tenant_id: TenantId,
    /// Its slug.
    pub slug: TenantSlug,
    /// The first administrator.
    pub admin_user_id: UserId,
    /// The first administrator's key. There are no passwords, so this is their credential.
    pub initial_key: IssuedKey,
}

/// Who an API key acts as, as established by [`IdentityStore::authenticate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The firm.
    pub tenant: TenantId,
    /// The user.
    pub user: UserId,
    /// The key used.
    pub key: ApiKeyId,
    /// The user's roles now.
    pub roles: RoleSet,
    /// What the key may do now: its scopes limited by the roles above.
    pub permissions: BTreeSet<Permission>,
}

/// Input for issuing a key.
#[derive(Debug, Clone)]
pub struct IssueKeyRequest {
    /// The user the key acts as; `None` means the caller.
    pub owner: Option<UserId>,
    /// Label.
    pub name: String,
    /// Requested scopes.
    pub scopes: Vec<Permission>,
    /// Requested expiry.
    pub expires_at: Option<OffsetDateTime>,
}

/// The identity store.
#[derive(Clone)]
pub struct IdentityStore {
    pool: TenantPool,
    clock: Clock,
}

fn random32() -> Result<[u8; 32], StoreError> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| StoreError::Internal(format!("no randomness: {e}")))?;
    Ok(b)
}

fn parse_roles(rows: &[String]) -> Result<RoleSet, String> {
    RoleSet::parse(rows.iter().map(String::as_str)).map_err(|e| format!("stored roles invalid: {e}"))
}

fn parse_scopes(scopes: &[String]) -> Result<BTreeSet<Permission>, String> {
    scopes
        .iter()
        .map(|s| Permission::parse(s))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("stored scopes invalid: {e}"))
}

impl IdentityStore {
    /// A store using the system clock.
    pub fn new(pool: TenantPool) -> Self {
        Self::with_clock(pool, Arc::new(OffsetDateTime::now_utc))
    }

    /// A store with an injected clock (for tests).
    pub fn with_clock(pool: TenantPool, clock: Clock) -> Self {
        Self { pool, clock }
    }

    /// Create a firm, its first admin and the admin's first key in one transaction.
    ///
    /// Who may call this (a platform operator) is the gateway's decision, not the store's.
    pub async fn create_tenant(&self, req: &NewTenantRequest<'_>) -> Result<CreatedTenant, StoreError> {
        let boot = validate_new_tenant(req)?;
        let now = (self.clock)();
        let tenant = TenantId::from_uuid(Uuid::new_v4());
        let admin = UserId::from_uuid(Uuid::new_v4());
        let key_id = ApiKeyId::from_uuid(Uuid::new_v4());

        // The initial key goes through the same rule as any other key.
        let admin_permissions = boot.admin_roles.permissions();
        let scopes: Vec<Permission> = admin_permissions.iter().copied().collect();
        let validated = check_key_issuance(&KeyIssuanceRequest {
            actor_permissions: &admin_permissions,
            owner_status: boot.admin_status,
            owner_roles: &boot.admin_roles,
            scopes: &scopes,
            name: "initial admin key",
            now,
            expires_at: None,
        })?;
        let token = ApiKeyToken::new(tenant, random32()?);

        let mut tx = self.pool.begin(tenant).await?;
        let inserted = sqlx::query(
            "INSERT INTO identity.tenants (id, slug, display_name, status, created_at) VALUES ($1, $2, $3, 'active', $4)",
        )
        .bind(tenant.as_uuid())
        .bind(boot.slug.as_str())
        .bind(&boot.display_name)
        .bind(now)
        .execute(tx.conn())
        .await;
        if let Err(e) = inserted {
            let slug_taken = e
                .as_database_error()
                .and_then(|d| d.constraint())
                .is_some_and(|c| c == "tenants_slug_key");
            return Err(if slug_taken {
                StoreError::SlugTaken
            } else {
                e.into()
            });
        }
        sqlx::query(
            "INSERT INTO identity.users (tenant_id, id, email, display_name, status, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(tenant.as_uuid())
        .bind(admin.as_uuid())
        .bind(boot.admin_email.as_str())
        .bind(&boot.admin_display_name)
        .bind(boot.admin_status.as_str())
        .bind(now)
        .execute(tx.conn())
        .await?;
        for role in boot.admin_roles.iter() {
            // The first admin is granted their role by themselves: there is nobody else yet.
            sqlx::query(
                "INSERT INTO identity.user_roles (tenant_id, user_id, role, granted_by, granted_at) VALUES ($1, $2, $3, $2, $4)",
            )
            .bind(tenant.as_uuid())
            .bind(admin.as_uuid())
            .bind(role.as_str())
            .bind(now)
            .execute(tx.conn())
            .await?;
        }
        insert_key(
            &mut tx,
            tenant,
            key_id,
            admin,
            admin,
            &token,
            &validated.name,
            &validated.scopes,
            None,
            now,
        )
        .await?;
        tx.commit().await?;

        Ok(CreatedTenant {
            tenant_id: tenant,
            slug: boot.slug,
            admin_user_id: admin,
            initial_key: IssuedKey {
                id: key_id,
                user_id: admin,
                name: validated.name,
                token,
                scopes: validated.scopes,
                expires_at: None,
                created_at: now,
            },
        })
    }

    /// Issue an API key for `req.owner` (default: the caller), acting as `actor`.
    pub async fn issue_api_key(
        &self,
        actor: &Principal,
        req: IssueKeyRequest,
    ) -> Result<IssuedKey, StoreError> {
        let now = (self.clock)();
        let owner = req.owner.unwrap_or(actor.user);
        let mut tx = self.pool.begin(actor.tenant).await?;

        // Row-level security hides other firms' users, so a foreign id is just "not found".
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM identity.users WHERE id = $1")
            .bind(owner.as_uuid())
            .fetch_optional(tx.conn())
            .await?;
        let status = status.ok_or(StoreError::NotFound)?;
        let owner_status = UserStatus::parse(&status).map_err(|e| StoreError::Internal(e.to_string()))?;
        let role_names: Vec<String> =
            sqlx::query_scalar("SELECT role FROM identity.user_roles WHERE user_id = $1")
                .bind(owner.as_uuid())
                .fetch_all(tx.conn())
                .await?;
        let owner_roles = parse_roles(&role_names).map_err(StoreError::Internal)?;

        let validated = check_key_issuance(&KeyIssuanceRequest {
            actor_permissions: &actor.permissions,
            owner_status,
            owner_roles: &owner_roles,
            scopes: &req.scopes,
            name: &req.name,
            now,
            expires_at: req.expires_at,
        })?;

        let key_id = ApiKeyId::from_uuid(Uuid::new_v4());
        let token = ApiKeyToken::new(actor.tenant, random32()?);
        insert_key(
            &mut tx,
            actor.tenant,
            key_id,
            owner,
            actor.user,
            &token,
            &validated.name,
            &validated.scopes,
            validated.expires_at,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(IssuedKey {
            id: key_id,
            user_id: owner,
            name: validated.name,
            token,
            scopes: validated.scopes,
            expires_at: validated.expires_at,
            created_at: now,
        })
    }

    /// Revoke a key. Revoking an already-revoked key succeeds (it stays revoked).
    pub async fn revoke_api_key(&self, actor: &Principal, key: ApiKeyId) -> Result<(), StoreError> {
        require_permission(&actor.permissions, Permission::KeysManage)?;
        let now = (self.clock)();
        let mut tx = self.pool.begin(actor.tenant).await?;
        let revoked: Option<Uuid> = sqlx::query_scalar(
            "UPDATE identity.api_keys SET revoked_at = $1, revoked_by = $2 WHERE id = $3 AND revoked_at IS NULL RETURNING id",
        )
        .bind(now)
        .bind(actor.user.as_uuid())
        .bind(key.as_uuid())
        .fetch_optional(tx.conn())
        .await?;
        if revoked.is_none() {
            let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM identity.api_keys WHERE id = $1")
                .bind(key.as_uuid())
                .fetch_optional(tx.conn())
                .await?;
            if exists.is_none() {
                return Err(StoreError::NotFound);
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Resolve a presented token to the principal it acts as, or say why not.
    ///
    /// The tenant named inside the token is only a *claim*: it chooses which firm's rows are searched.
    /// The key must hash-match a row inside that firm, so a valid secret presented with another firm's
    /// prefix finds nothing.
    pub async fn authenticate(&self, presented: &str) -> Result<Principal, AuthError> {
        let token = ApiKeyToken::parse(presented).map_err(AuthError::Malformed)?;
        let now = (self.clock)();
        let tenant = token.tenant();
        let hash = token.hash();
        let mut tx = self.pool.begin(tenant).await?;

        let row = sqlx::query(
            "SELECT k.id, k.user_id, k.key_hash, k.scopes, k.expires_at, k.revoked_at, k.last_used_at, \
                    u.status AS user_status, t.status AS tenant_status \
             FROM identity.api_keys k \
             JOIN identity.users u ON u.tenant_id = k.tenant_id AND u.id = k.user_id \
             JOIN identity.tenants t ON t.id = k.tenant_id \
             WHERE k.key_hash = $1",
        )
        .bind(hash.as_bytes().as_slice())
        .fetch_optional(tx.conn())
        .await?;
        let row = row.ok_or(AuthError::UnknownKey)?;

        // A second, constant-time check on top of the SQL equality.
        let stored: Vec<u8> = row.try_get("key_hash")?;
        let stored = KeyHash::from_bytes(&stored)
            .ok_or_else(|| AuthError::Internal("stored hash has the wrong length".into()))?;
        if !stored.matches(&hash) {
            return Err(AuthError::UnknownKey);
        }

        let user_status: String = row.try_get("user_status")?;
        let tenant_status: String = row.try_get("tenant_status")?;
        let scopes: Vec<String> = row.try_get("scopes")?;
        let state = KeyState {
            expires_at: row.try_get("expires_at")?,
            revoked_at: row.try_get("revoked_at")?,
            user_status: UserStatus::parse(&user_status).map_err(|e| AuthError::Internal(e.to_string()))?,
            tenant_status: TenantStatus::parse(&tenant_status)
                .map_err(|e| AuthError::Internal(e.to_string()))?,
        };
        check_key_usable(&state, now).map_err(AuthError::Rejected)?;

        let key_id: Uuid = row.try_get("id")?;
        let user_id: Uuid = row.try_get("user_id")?;
        let last_used: Option<OffsetDateTime> = row.try_get("last_used_at")?;
        let role_names: Vec<String> =
            sqlx::query_scalar("SELECT role FROM identity.user_roles WHERE user_id = $1")
                .bind(user_id)
                .fetch_all(tx.conn())
                .await?;
        let roles = parse_roles(&role_names).map_err(AuthError::Internal)?;
        let scopes = parse_scopes(&scopes).map_err(AuthError::Internal)?;
        let permissions = effective_permissions(&scopes, &roles);

        if last_used.is_none_or(|t| (now - t).whole_seconds() >= LAST_USED_REFRESH_SECONDS) {
            sqlx::query("UPDATE identity.api_keys SET last_used_at = $1 WHERE id = $2")
                .bind(now)
                .bind(key_id)
                .execute(tx.conn())
                .await?;
            tx.commit().await?;
        }

        Ok(Principal {
            tenant,
            user: UserId::from_uuid(user_id),
            key: ApiKeyId::from_uuid(key_id),
            roles,
            permissions,
        })
    }
}

#[allow(clippy::too_many_arguments)] // one INSERT, shared by two callers; a struct would only rename the arguments
async fn insert_key(
    tx: &mut timelex_db::TenantTx,
    tenant: TenantId,
    id: ApiKeyId,
    owner: UserId,
    created_by: UserId,
    token: &ApiKeyToken,
    name: &str,
    scopes: &BTreeSet<Permission>,
    expires_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    let scope_names: Vec<&str> = scopes.iter().map(Permission::as_str).collect();
    sqlx::query(
        "INSERT INTO identity.api_keys \
           (tenant_id, id, user_id, name, key_prefix, key_hash, scopes, created_by, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(tenant.as_uuid())
    .bind(id.as_uuid())
    .bind(owner.as_uuid())
    .bind(name)
    .bind(token.display_prefix())
    .bind(token.hash().as_bytes().as_slice())
    .bind(&scope_names)
    .bind(created_by.as_uuid())
    .bind(now)
    .bind(expires_at)
    .execute(tx.conn())
    .await?;
    Ok(())
}
