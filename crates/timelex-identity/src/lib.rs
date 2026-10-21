// Written by Kone Tshivhinda & Claude
//! Identity rules for TimeLex: firms (tenants), users, roles, permissions and API keys.
//!
//! This crate has **no I/O**: no database, no network, no clock, no randomness. Anything that changes
//! over time or needs entropy (the current time, 32 random bytes for a key) is passed in. That makes
//! every rule a function you can call in a test with fixed inputs. The `identity` service and the
//! database layer call these functions; they never re-implement a rule.
//!
//! ```
//! use timelex_identity::{ApiKeyToken, TenantId};
//! use uuid::Uuid;
//!
//! let tenant = TenantId::from_uuid(Uuid::nil());
//! let token = ApiKeyToken::new(tenant, [42u8; 32]); // the service supplies real random bytes
//! let parsed = ApiKeyToken::parse(token.expose_secret()).unwrap();
//! assert_eq!(parsed.tenant(), tenant);
//! assert!(parsed.hash().matches(&token.hash()));
//! ```

mod apikey;
mod error;
mod ids;
mod names;
mod role;
mod rules;
mod status;

pub use apikey::{check_key_usable, ApiKeyToken, KeyHash, KeyState};
pub use error::{AuthFailure, IdentityError, TokenError};
pub use ids::{ApiKeyId, TenantId, UserId};
pub use names::{validate_name, Email, TenantSlug, MAX_DISPLAY_NAME, MAX_KEY_NAME};
pub use role::{Permission, Role, RoleSet};
pub use rules::{
    check_admin_continuity, check_key_issuance, effective_permissions, require_permission,
    validate_new_tenant, KeyIssuanceRequest, NewTenantRequest, TenantBootstrap, UserState, ValidatedKey,
};
pub use status::{TenantStatus, UserStatus};
