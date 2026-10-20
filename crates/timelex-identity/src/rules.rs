// Written by Kone Tshivhinda & Claude
//! Business rules that span more than one value: issuing keys, keeping an admin, creating a firm.
//!
//! These are plain functions over plain data. The service layer loads the facts (who is acting, how
//! many other admins exist), calls the function, and only then writes to the database.

use std::collections::BTreeSet;

use time::OffsetDateTime;

use crate::error::IdentityError;
use crate::names::{validate_name, Email, TenantSlug, MAX_DISPLAY_NAME, MAX_KEY_NAME};
use crate::role::{Permission, Role, RoleSet};
use crate::status::UserStatus;

/// Fail unless `permissions` contains `needed`. The one place the "does the caller hold X" question is asked.
pub fn require_permission(
    permissions: &BTreeSet<Permission>,
    needed: Permission,
) -> Result<(), IdentityError> {
    if permissions.contains(&needed) {
        Ok(())
    } else {
        Err(IdentityError::PermissionDenied(needed.as_str()))
    }
}

/// What an API key may do *right now*: its scopes, limited to what its owner's roles currently grant.
///
/// A key's scopes are fixed when it is issued, but the owner's roles can shrink afterwards (an admin
/// is demoted, a fee earner becomes an auditor). Intersecting at every request means a role change
/// takes effect on existing keys at once, without hunting them down.
pub fn effective_permissions(scopes: &BTreeSet<Permission>, owner_roles: &RoleSet) -> BTreeSet<Permission> {
    let granted = owner_roles.permissions();
    scopes.intersection(&granted).copied().collect()
}

/// A request to create a firm and its first administrator, as received from a platform operator.
#[derive(Debug, Clone, Copy)]
pub struct NewTenantRequest<'a> {
    /// Proposed slug.
    pub slug: &'a str,
    /// Firm name.
    pub display_name: &'a str,
    /// First admin's email.
    pub admin_email: &'a str,
    /// First admin's name.
    pub admin_display_name: &'a str,
}

/// A validated request: everything the service needs to create the firm in one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantBootstrap {
    /// Validated slug.
    pub slug: TenantSlug,
    /// Trimmed firm name.
    pub display_name: String,
    /// Normalised admin email.
    pub admin_email: Email,
    /// Trimmed admin name.
    pub admin_display_name: String,
    /// Always `[Admin]`. The first admin starts `Active` so the firm is usable at once.
    pub admin_roles: RoleSet,
    /// Always [`UserStatus::Active`].
    pub admin_status: UserStatus,
}

/// Validate a new-firm request. Who is allowed to *ask* (a platform operator) is decided by the
/// gateway, not here.
pub fn validate_new_tenant(req: &NewTenantRequest<'_>) -> Result<TenantBootstrap, IdentityError> {
    Ok(TenantBootstrap {
        slug: TenantSlug::parse(req.slug)?,
        display_name: validate_name(req.display_name, MAX_DISPLAY_NAME)?,
        admin_email: Email::parse(req.admin_email)?,
        admin_display_name: validate_name(req.admin_display_name, MAX_DISPLAY_NAME)?,
        admin_roles: RoleSet::new([Role::Admin])?,
        admin_status: UserStatus::Active,
    })
}

/// A request to issue an API key.
#[derive(Debug, Clone, Copy)]
pub struct KeyIssuanceRequest<'a> {
    /// What the person (or key) asking is allowed to do.
    pub actor_permissions: &'a BTreeSet<Permission>,
    /// Status of the user the key will act as.
    pub owner_status: UserStatus,
    /// Roles of that user.
    pub owner_roles: &'a RoleSet,
    /// Scopes requested for the key.
    pub scopes: &'a [Permission],
    /// Label for the key.
    pub name: &'a str,
    /// Current time, passed in so the function stays deterministic.
    pub now: OffsetDateTime,
    /// Requested expiry, if any.
    pub expires_at: Option<OffsetDateTime>,
}

/// A key request that passed every rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedKey {
    /// Trimmed label.
    pub name: String,
    /// De-duplicated scopes in a stable order.
    pub scopes: BTreeSet<Permission>,
    /// Expiry, if any.
    pub expires_at: Option<OffsetDateTime>,
}

/// Rules for issuing a key:
/// 1. the actor holds `keys:manage` (agents never do, so a machine cannot mint keys);
/// 2. the owner is an active user;
/// 3. at least one scope, and every scope is granted by the owner's roles (no privilege escalation);
/// 4. the name is 1 to 100 characters;
/// 5. any expiry is in the future.
pub fn check_key_issuance(req: &KeyIssuanceRequest<'_>) -> Result<ValidatedKey, IdentityError> {
    require_permission(req.actor_permissions, Permission::KeysManage)?;
    if req.owner_status != UserStatus::Active {
        return Err(IdentityError::KeyOwnerNotActive);
    }
    if req.scopes.is_empty() {
        return Err(IdentityError::NoScopes);
    }
    let owner_permissions = req.owner_roles.permissions();
    let mut scopes = BTreeSet::new();
    for scope in req.scopes {
        if !owner_permissions.contains(scope) {
            return Err(IdentityError::ScopeNotGranted(scope.as_str().to_owned()));
        }
        scopes.insert(*scope);
    }
    let name = validate_name(req.name, MAX_KEY_NAME)?;
    if let Some(expires_at) = req.expires_at {
        if expires_at <= req.now {
            return Err(IdentityError::ExpiryInPast);
        }
    }
    Ok(ValidatedKey {
        name,
        scopes,
        expires_at: req.expires_at,
    })
}

/// A user's roles and status at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserState {
    /// Roles held.
    pub roles: RoleSet,
    /// Status.
    pub status: UserStatus,
}

impl UserState {
    /// An *active admin* holds the admin role and is active. Only these keep the firm manageable.
    pub fn is_active_admin(&self) -> bool {
        self.status == UserStatus::Active && self.roles.contains(Role::Admin)
    }
}

/// A change must never leave a firm with no active admin (nobody could then issue keys or invite
/// anyone, and only a platform operator could rescue it).
///
/// `other_active_admins` is how many active admins exist *besides* this user. The service must read
/// it inside the same transaction, holding a row lock on the firm's admins, so two simultaneous
/// changes cannot both pass.
pub fn check_admin_continuity(
    before: &UserState,
    after: &UserState,
    other_active_admins: usize,
) -> Result<(), IdentityError> {
    if before.is_active_admin() && !after.is_active_admin() && other_active_admins == 0 {
        return Err(IdentityError::LastAdmin);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    fn perms(roles: &[Role]) -> BTreeSet<Permission> {
        RoleSet::new(roles.iter().copied()).unwrap().permissions()
    }

    // --- new firm ---

    fn new_firm<'a>() -> NewTenantRequest<'a> {
        NewTenantRequest {
            slug: "example-attorneys",
            display_name: "  Example Attorneys Inc  ",
            admin_email: "Partner@Example-Attorneys.example",
            admin_display_name: "Pat Partner",
        }
    }

    #[test]
    fn new_firm_is_normalised_and_starts_with_an_active_admin() {
        let b = validate_new_tenant(&new_firm()).unwrap();
        assert_eq!(b.slug.as_str(), "example-attorneys");
        assert_eq!(b.display_name, "Example Attorneys Inc");
        assert_eq!(b.admin_email.as_str(), "partner@example-attorneys.example");
        assert_eq!(b.admin_status, UserStatus::Active);
        assert!(b.admin_roles.contains(Role::Admin));
        assert_eq!(b.admin_roles.iter().count(), 1);
    }

    #[test]
    fn new_firm_rejects_each_bad_field() {
        let mut r = new_firm();
        r.slug = "Bad Slug";
        assert_eq!(validate_new_tenant(&r), Err(IdentityError::InvalidSlug));
        let mut r = new_firm();
        r.display_name = "  ";
        assert!(matches!(
            validate_new_tenant(&r),
            Err(IdentityError::InvalidName { .. })
        ));
        let mut r = new_firm();
        r.admin_email = "not-an-email";
        assert_eq!(validate_new_tenant(&r), Err(IdentityError::InvalidEmail));
        let mut r = new_firm();
        r.admin_display_name = "";
        assert!(matches!(
            validate_new_tenant(&r),
            Err(IdentityError::InvalidName { .. })
        ));
    }

    // --- key issuance ---

    struct Fixture {
        actor: BTreeSet<Permission>,
        owner_roles: RoleSet,
        scopes: Vec<Permission>,
    }

    fn fixture() -> Fixture {
        Fixture {
            actor: perms(&[Role::Admin]),
            owner_roles: RoleSet::new([Role::Agent]).unwrap(),
            scopes: vec![Permission::CaptureWrite],
        }
    }

    fn request<'a>(f: &'a Fixture) -> KeyIssuanceRequest<'a> {
        KeyIssuanceRequest {
            actor_permissions: &f.actor,
            owner_status: UserStatus::Active,
            owner_roles: &f.owner_roles,
            scopes: &f.scopes,
            name: "  capture connector ",
            now: now(),
            expires_at: None,
        }
    }

    #[test]
    fn admin_can_issue_a_key_within_the_owners_permissions() {
        let f = fixture();
        let v = check_key_issuance(&request(&f)).unwrap();
        assert_eq!(v.name, "capture connector");
        assert_eq!(v.scopes, BTreeSet::from([Permission::CaptureWrite]));
        assert_eq!(v.expires_at, None);
    }

    #[test]
    fn duplicate_scopes_collapse() {
        let mut f = fixture();
        f.scopes = vec![Permission::CaptureWrite, Permission::CaptureWrite];
        assert_eq!(check_key_issuance(&request(&f)).unwrap().scopes.len(), 1);
    }

    #[test]
    fn only_holders_of_keys_manage_can_issue() {
        for roles in [
            vec![Role::FeeEarner],
            vec![Role::Billing],
            vec![Role::Auditor],
            vec![Role::Agent],
        ] {
            let mut f = fixture();
            f.actor = perms(&roles);
            assert_eq!(
                check_key_issuance(&request(&f)),
                Err(IdentityError::PermissionDenied("keys:manage")),
                "{roles:?}"
            );
        }
    }

    #[test]
    fn a_machine_cannot_mint_keys_even_for_itself() {
        let mut f = fixture();
        f.actor = perms(&[Role::Agent]);
        assert!(check_key_issuance(&request(&f)).is_err());
    }

    #[test]
    fn scopes_cannot_exceed_the_owners_roles() {
        let mut f = fixture();
        f.owner_roles = RoleSet::new([Role::FeeEarner]).unwrap();
        f.scopes = vec![Permission::TimeWrite, Permission::BillingWrite];
        assert_eq!(
            check_key_issuance(&request(&f)),
            Err(IdentityError::ScopeNotGranted("billing:write".into()))
        );
    }

    #[test]
    fn an_agent_key_can_never_carry_a_human_only_scope() {
        for p in Permission::HUMAN_ONLY {
            let mut f = fixture();
            f.scopes = vec![p];
            assert!(
                matches!(
                    check_key_issuance(&request(&f)),
                    Err(IdentityError::ScopeNotGranted(_))
                ),
                "{}",
                p.as_str()
            );
        }
    }

    #[test]
    fn a_user_with_no_roles_cannot_hold_a_scoped_key() {
        let mut f = fixture();
        f.owner_roles = RoleSet::default();
        assert!(matches!(
            check_key_issuance(&request(&f)),
            Err(IdentityError::ScopeNotGranted(_))
        ));
    }

    #[test]
    fn empty_scope_list_is_rejected() {
        let mut f = fixture();
        f.scopes = vec![];
        assert_eq!(check_key_issuance(&request(&f)), Err(IdentityError::NoScopes));
    }

    #[test]
    fn owner_must_be_active() {
        for status in [UserStatus::Invited, UserStatus::Disabled] {
            let f = fixture();
            let mut r = request(&f);
            r.owner_status = status;
            assert_eq!(check_key_issuance(&r), Err(IdentityError::KeyOwnerNotActive));
        }
    }

    #[test]
    fn key_name_is_validated() {
        let f = fixture();
        let mut r = request(&f);
        r.name = "   ";
        assert!(matches!(
            check_key_issuance(&r),
            Err(IdentityError::InvalidName { max_chars: 100 })
        ));
        let long = "k".repeat(101);
        r.name = &long;
        assert!(check_key_issuance(&r).is_err());
    }

    #[test]
    fn expiry_must_be_in_the_future() {
        let f = fixture();
        let mut r = request(&f);
        r.expires_at = Some(now());
        assert_eq!(check_key_issuance(&r), Err(IdentityError::ExpiryInPast));
        r.expires_at = Some(now() - Duration::days(1));
        assert_eq!(check_key_issuance(&r), Err(IdentityError::ExpiryInPast));
        r.expires_at = Some(now() + Duration::seconds(1));
        assert!(check_key_issuance(&r).is_ok());
    }

    // --- permission helpers ---

    #[test]
    fn require_permission_names_the_missing_permission() {
        let have = perms(&[Role::FeeEarner]);
        assert_eq!(require_permission(&have, Permission::TimeWrite), Ok(()));
        assert_eq!(
            require_permission(&have, Permission::KeysManage),
            Err(IdentityError::PermissionDenied("keys:manage"))
        );
        assert!(require_permission(&BTreeSet::new(), Permission::TimeRead).is_err());
    }

    #[test]
    fn effective_permissions_are_scopes_limited_by_current_roles() {
        let scopes = BTreeSet::from([Permission::TimeRead, Permission::UsersManage]);
        let admin = RoleSet::new([Role::Admin]).unwrap();
        assert_eq!(effective_permissions(&scopes, &admin), scopes);
        // Demoted to auditor: UsersManage disappears from the same key at once.
        let auditor = RoleSet::new([Role::Auditor]).unwrap();
        assert_eq!(
            effective_permissions(&scopes, &auditor),
            BTreeSet::from([Permission::TimeRead])
        );
        // No roles left: the key can do nothing.
        assert!(effective_permissions(&scopes, &RoleSet::default()).is_empty());
    }

    #[test]
    fn effective_permissions_never_exceed_the_scopes() {
        // A powerful owner with a narrow key: the key stays narrow.
        let scopes = BTreeSet::from([Permission::TimeRead]);
        let admin = RoleSet::new([Role::Admin]).unwrap();
        assert_eq!(effective_permissions(&scopes, &admin), scopes);
    }

    // --- last admin ---

    fn state(roles: &[Role], status: UserStatus) -> UserState {
        UserState {
            roles: RoleSet::new(roles.iter().copied()).unwrap(),
            status,
        }
    }

    #[test]
    fn the_last_active_admin_cannot_be_disabled() {
        let before = state(&[Role::Admin], UserStatus::Active);
        let after = state(&[Role::Admin], UserStatus::Disabled);
        assert_eq!(
            check_admin_continuity(&before, &after, 0),
            Err(IdentityError::LastAdmin)
        );
    }

    #[test]
    fn the_last_active_admin_cannot_lose_the_admin_role() {
        let before = state(&[Role::Admin, Role::FeeEarner], UserStatus::Active);
        let after = state(&[Role::FeeEarner], UserStatus::Active);
        assert_eq!(
            check_admin_continuity(&before, &after, 0),
            Err(IdentityError::LastAdmin)
        );
    }

    #[test]
    fn an_admin_may_step_down_when_another_active_admin_exists() {
        let before = state(&[Role::Admin], UserStatus::Active);
        let after = state(&[Role::FeeEarner], UserStatus::Active);
        assert_eq!(check_admin_continuity(&before, &after, 1), Ok(()));
        let disabled = state(&[Role::Admin], UserStatus::Disabled);
        assert_eq!(check_admin_continuity(&before, &disabled, 2), Ok(()));
    }

    #[test]
    fn changes_that_keep_the_admin_active_are_always_fine() {
        let before = state(&[Role::Admin], UserStatus::Active);
        let after = state(&[Role::Admin, Role::Billing], UserStatus::Active);
        assert_eq!(check_admin_continuity(&before, &after, 0), Ok(()));
    }

    #[test]
    fn changes_to_non_admins_never_trip_the_rule() {
        let before = state(&[Role::FeeEarner], UserStatus::Active);
        let after = state(&[Role::FeeEarner], UserStatus::Disabled);
        assert_eq!(check_admin_continuity(&before, &after, 0), Ok(()));
    }

    #[test]
    fn an_invited_or_disabled_admin_does_not_count_as_active() {
        for status in [UserStatus::Invited, UserStatus::Disabled] {
            assert!(!state(&[Role::Admin], status).is_active_admin());
        }
        // Disabling someone who is not an active admin cannot orphan the firm.
        let before = state(&[Role::Admin], UserStatus::Invited);
        let after = state(&[Role::Admin], UserStatus::Disabled);
        assert_eq!(check_admin_continuity(&before, &after, 0), Ok(()));
    }
}
