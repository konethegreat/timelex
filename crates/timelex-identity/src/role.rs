// Written by Kone Tshivhinda & Claude
//! Roles and permissions. Who may do what is decided here, in one table, and nowhere else.
//!
//! A **role** is a job (admin, fee earner, ...). A **permission** is one thing that can be done
//! (`users:manage`). Each role grants a fixed set of permissions; a user's permissions are the union
//! over their roles. An API key carries a list of permissions ("scopes") that must be a subset of its
//! owner's permissions, so a key can never do more than the person or machine it belongs to.
//!
//! Adding a permission or role means editing the matrix in [`Role::permissions`]; the tests below
//! then check the safety properties still hold.

use std::collections::BTreeSet;

use crate::error::IdentityError;

/// A thing that can be done. The string form (for example `time:write`) is what is stored in key
/// scopes and shown in APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Permission {
    /// List and view users.
    UsersRead,
    /// Invite, change and disable users and their roles.
    UsersManage,
    /// Issue and revoke API keys. Powerful: a holder can issue a key up to any user's permissions, so
    /// treat it like admin (only the admin role has it).
    KeysManage,
    /// Change firm settings, including autonomy settings. Humans only (AUTONOMY.md).
    SettingsManage,
    /// View time entries.
    TimeRead,
    /// Create and correct time entries.
    TimeWrite,
    /// View WIP, pro-formas and fee-note data.
    BillingRead,
    /// Prepare and certify pro-formas.
    BillingWrite,
    /// Read the audit log.
    AuditRead,
    /// Submit captured activity events.
    CaptureWrite,
}

impl Permission {
    /// Every permission, in a fixed order.
    pub const ALL: [Permission; 10] = [
        Permission::UsersRead,
        Permission::UsersManage,
        Permission::KeysManage,
        Permission::SettingsManage,
        Permission::TimeRead,
        Permission::TimeWrite,
        Permission::BillingRead,
        Permission::BillingWrite,
        Permission::AuditRead,
        Permission::CaptureWrite,
    ];

    /// Permissions that only a human may hold. A machine ("agent") never gets these, whatever
    /// anyone configures.
    pub const HUMAN_ONLY: [Permission; 3] = [
        Permission::UsersManage,
        Permission::KeysManage,
        Permission::SettingsManage,
    ];

    /// The stored and displayed spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::UsersRead => "users:read",
            Permission::UsersManage => "users:manage",
            Permission::KeysManage => "keys:manage",
            Permission::SettingsManage => "settings:manage",
            Permission::TimeRead => "time:read",
            Permission::TimeWrite => "time:write",
            Permission::BillingRead => "billing:read",
            Permission::BillingWrite => "billing:write",
            Permission::AuditRead => "audit:read",
            Permission::CaptureWrite => "capture:write",
        }
    }

    /// Parse the stored spelling. Unknown text is an error, so a row with an unrecognised scope
    /// fails closed instead of being ignored.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        Permission::ALL
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| IdentityError::UnknownPermission(s.to_owned()))
    }
}

/// A job a user can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Runs the firm's account: users, keys, settings.
    Admin,
    /// Records and corrects time.
    FeeEarner,
    /// Prepares and certifies billing.
    Billing,
    /// Read-only access for the firm's auditor.
    Auditor,
    /// A machine, such as a capture connector. Cannot be combined with any other role.
    Agent,
}

impl Role {
    /// Every role, in a fixed order.
    pub const ALL: [Role; 5] = [
        Role::Admin,
        Role::FeeEarner,
        Role::Billing,
        Role::Auditor,
        Role::Agent,
    ];

    /// The stored spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::FeeEarner => "fee_earner",
            Role::Billing => "billing",
            Role::Auditor => "auditor",
            Role::Agent => "agent",
        }
    }

    /// Parse the stored spelling.
    pub fn parse(s: &str) -> Result<Self, IdentityError> {
        Role::ALL
            .into_iter()
            .find(|r| r.as_str() == s)
            .ok_or_else(|| IdentityError::UnknownRole(s.to_owned()))
    }

    /// What this role may do. This table is the whole access policy for Phase 1.
    pub fn permissions(&self) -> &'static [Permission] {
        use Permission::*;
        match self {
            Role::Admin => &[
                UsersRead,
                UsersManage,
                KeysManage,
                SettingsManage,
                TimeRead,
                BillingRead,
                AuditRead,
            ],
            Role::FeeEarner => &[TimeRead, TimeWrite],
            Role::Billing => &[TimeRead, BillingRead, BillingWrite],
            Role::Auditor => &[UsersRead, TimeRead, BillingRead, AuditRead],
            Role::Agent => &[CaptureWrite, TimeRead, TimeWrite],
        }
    }
}

/// The roles one user holds. Construction enforces: the `agent` role is never combined with another.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleSet(BTreeSet<Role>);

impl RoleSet {
    /// Build from roles (duplicates collapse). Fails if `agent` is mixed with any other role.
    pub fn new(roles: impl IntoIterator<Item = Role>) -> Result<Self, IdentityError> {
        let set: BTreeSet<Role> = roles.into_iter().collect();
        if set.contains(&Role::Agent) && set.len() > 1 {
            return Err(IdentityError::AgentRoleIsExclusive);
        }
        Ok(Self(set))
    }

    /// Parse stored role names.
    pub fn parse<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Self, IdentityError> {
        let roles = names
            .into_iter()
            .map(Role::parse)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(roles)
    }

    /// Does the user hold this role?
    pub fn contains(&self, role: Role) -> bool {
        self.0.contains(&role)
    }

    /// True if no roles are held.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Roles in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = Role> + '_ {
        self.0.iter().copied()
    }

    /// Union of the permissions of every role held.
    pub fn permissions(&self) -> BTreeSet<Permission> {
        self.0
            .iter()
            .flat_map(|r| r.permissions().iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip() {
        for p in Permission::ALL {
            assert_eq!(Permission::parse(p.as_str()), Ok(p));
        }
        for r in Role::ALL {
            assert_eq!(Role::parse(r.as_str()), Ok(r));
        }
    }

    #[test]
    fn unknown_strings_are_errors() {
        assert!(matches!(
            Permission::parse("users:delete"),
            Err(IdentityError::UnknownPermission(_))
        ));
        assert!(matches!(
            Permission::parse("USERS:READ"),
            Err(IdentityError::UnknownPermission(_))
        ));
        assert!(matches!(Role::parse("owner"), Err(IdentityError::UnknownRole(_))));
    }

    #[test]
    fn agent_role_is_exclusive() {
        assert!(RoleSet::new([Role::Agent]).is_ok());
        for other in [Role::Admin, Role::FeeEarner, Role::Billing, Role::Auditor] {
            assert_eq!(
                RoleSet::new([Role::Agent, other]),
                Err(IdentityError::AgentRoleIsExclusive)
            );
        }
        assert!(RoleSet::new([Role::Admin, Role::FeeEarner]).is_ok());
    }

    #[test]
    fn duplicate_roles_collapse() {
        let set = RoleSet::new([Role::Billing, Role::Billing]).unwrap();
        assert_eq!(set.iter().count(), 1);
    }

    #[test]
    fn agents_never_hold_human_only_permissions() {
        // This is the safety property from AUTONOMY.md: a machine cannot manage users, mint keys or
        // change autonomy settings. It must stay true when the matrix is edited.
        for p in Role::Agent.permissions() {
            assert!(!Permission::HUMAN_ONLY.contains(p), "agent holds {}", p.as_str());
        }
    }

    #[test]
    fn only_admins_hold_human_only_permissions() {
        for role in Role::ALL {
            let holds_any = role
                .permissions()
                .iter()
                .any(|p| Permission::HUMAN_ONLY.contains(p));
            assert_eq!(holds_any, role == Role::Admin, "{}", role.as_str());
        }
    }

    #[test]
    fn auditor_is_read_only() {
        for p in Role::Auditor.permissions() {
            assert!(
                !matches!(
                    p,
                    Permission::UsersManage
                        | Permission::KeysManage
                        | Permission::SettingsManage
                        | Permission::TimeWrite
                        | Permission::BillingWrite
                        | Permission::CaptureWrite
                ),
                "auditor holds {}",
                p.as_str()
            );
        }
    }

    #[test]
    fn matrix_has_no_duplicates_and_uses_only_known_permissions() {
        for role in Role::ALL {
            let perms = role.permissions();
            let unique: BTreeSet<_> = perms.iter().collect();
            assert_eq!(
                unique.len(),
                perms.len(),
                "{} lists a permission twice",
                role.as_str()
            );
        }
    }

    #[test]
    fn permissions_are_the_union_of_roles() {
        let set = RoleSet::new([Role::FeeEarner, Role::Billing]).unwrap();
        let perms = set.permissions();
        assert!(perms.contains(&Permission::TimeWrite));
        assert!(perms.contains(&Permission::BillingWrite));
        assert!(!perms.contains(&Permission::UsersManage));
    }

    #[test]
    fn empty_role_set_grants_nothing() {
        assert!(RoleSet::default().permissions().is_empty());
    }
}
