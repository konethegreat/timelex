// Written by Kone Tshivhinda & Claude
//! Generate a new TimeLex service from the templates every existing service follows.
//!
//! A generated service arrives with the platform's shape: internal-token authentication, a tenant
//! scoped table under row-level security, `/health`, telemetry and database-backed tests. The crate is
//! added to the workspace and the service to the Postgres bootstrap, so `cargo test --workspace` and
//! CI pick it up without further wiring.

use std::fmt;
use std::path::{Path, PathBuf};

/// What a successful run created and changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Files written under `services/<name>`.
    pub created: Vec<PathBuf>,
    /// Existing workspace files that gained a line.
    pub patched: Vec<PathBuf>,
}

/// Why a service could not be generated.
#[derive(Debug, PartialEq, Eq)]
pub enum GenerateError {
    /// The name breaks the rule `platform.provision_service` enforces.
    InvalidName(&'static str),
    /// The directory already exists; refusing is safer than merging with someone's work.
    Exists(PathBuf),
    /// A file could not be read or written.
    Io { path: PathBuf, message: String },
    /// The workspace file no longer has the shape the generator edits.
    MissingMarker { path: PathBuf, marker: &'static str },
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(reason) => write!(f, "invalid service name: {reason}"),
            Self::Exists(path) => write!(f, "{} already exists", path.display()),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::MissingMarker { path, marker } => {
                write!(f, "{} no longer contains {marker:?}", path.display())
            }
        }
    }
}

impl std::error::Error for GenerateError {}

/// The files every generated service has, relative to `services/<name>`. Tokens are replaced by
/// [`Tokens::render`].
const TEMPLATES: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../templates/Cargo.toml.tmpl")),
    ("README.md", include_str!("../templates/README.md.tmpl")),
    ("src/lib.rs", include_str!("../templates/src/lib.rs.tmpl")),
    ("src/main.rs", include_str!("../templates/src/main.rs.tmpl")),
    ("src/error.rs", include_str!("../templates/src/error.rs.tmpl")),
    ("src/http.rs", include_str!("../templates/src/http.rs.tmpl")),
    ("src/store.rs", include_str!("../templates/src/store.rs.tmpl")),
    (
        "migrations/0001_items.sql",
        include_str!("../templates/migrations/0001_items.sql.tmpl"),
    ),
    ("tests/http.rs", include_str!("../templates/tests/http.rs.tmpl")),
];

const MEMBERS_MARKER: &str = "members = [";
const BOOTSTRAP_FILE: &str = "deploy/postgres/bootstrap/004_provision_services.sql";

/// Check a service name against the rule in `platform.provision_service`: two to 31 characters,
/// starting with a lowercase letter, then lowercase letters, digits or underscores, and not one of
/// Postgres's own names.
pub fn validate_name(name: &str) -> Result<(), GenerateError> {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => {
            return Err(GenerateError::InvalidName(
                "it must start with a lowercase letter",
            ))
        }
    }
    if !(2..=31).contains(&name.chars().count()) {
        return Err(GenerateError::InvalidName("it must be 2 to 31 characters long"));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(GenerateError::InvalidName(
            "it may use only lowercase letters, digits and underscores",
        ));
    }
    if matches!(name, "platform" | "public") || name.starts_with("pg_") {
        return Err(GenerateError::InvalidName("it is reserved by Postgres"));
    }
    Ok(())
}

/// Write a new service under `root/services/<name>` and wire it into the workspace.
///
/// Refuses to touch an existing service directory: a generator that merges with existing work is one
/// bad edit away from destroying it.
pub fn generate(root: &Path, name: &str) -> Result<Report, GenerateError> {
    validate_name(name)?;
    let service_dir = root.join("services").join(name);
    if service_dir.exists() {
        return Err(GenerateError::Exists(service_dir));
    }

    let tokens = Tokens::for_name(name);
    let mut created = Vec::with_capacity(TEMPLATES.len());
    for (relative, template) in TEMPLATES {
        let target = service_dir.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        write(&target, &tokens.render(template))?;
        created.push(target);
    }

    add_member(root, name)?;
    provision(root, name)?;
    Ok(Report {
        created,
        patched: vec![root.join("Cargo.toml"), root.join(BOOTSTRAP_FILE)],
    })
}

struct Tokens {
    name: String,
    crate_name: String,
    lib_ident: String,
    upper: String,
}

impl Tokens {
    fn for_name(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            crate_name: format!("timelex-{name}-service"),
            lib_ident: format!("timelex_{name}_service"),
            upper: name.to_ascii_uppercase(),
        }
    }

    fn render(&self, template: &str) -> String {
        template
            .replace("@crate@", &self.crate_name)
            .replace("@ident@", &self.lib_ident)
            .replace("@UPPER@", &self.upper)
            .replace("@name@", &self.name)
    }
}

fn add_member(root: &Path, name: &str) -> Result<(), GenerateError> {
    let path = root.join("Cargo.toml");
    let text = read(&path)?;
    let entry = format!("\"services/{name}\"");
    if text.contains(&entry) {
        return Ok(());
    }
    let missing = || GenerateError::MissingMarker {
        path: path.clone(),
        marker: MEMBERS_MARKER,
    };
    let start = text.find(MEMBERS_MARKER).ok_or_else(missing)? + MEMBERS_MARKER.len();
    let end = text[start..].find(']').ok_or_else(missing)? + start;
    let empty = text[start..end].trim().is_empty();
    let separator = if empty { "" } else { ", " };

    let mut updated = String::with_capacity(text.len() + entry.len() + separator.len());
    updated.push_str(text[..end].trim_end().trim_end_matches(','));
    updated.push_str(separator);
    updated.push_str(&entry);
    updated.push_str(&text[end..]);
    write(&path, &updated)
}

fn provision(root: &Path, name: &str) -> Result<(), GenerateError> {
    let path = root.join(BOOTSTRAP_FILE);
    let text = read(&path)?;
    let line = format!("CALL platform.provision_service('{name}');");
    if text.lines().any(|existing| existing.trim() == line) {
        return Ok(());
    }
    let mut updated = text;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&line);
    updated.push('\n');
    write(&path, &updated)
}

fn read(path: &Path) -> Result<String, GenerateError> {
    std::fs::read_to_string(path).map_err(|error| io_error(path, error))
}

fn write(path: &Path, content: &str) -> Result<(), GenerateError> {
    std::fs::write(path, content).map_err(|error| io_error(path, error))
}

fn io_error(path: &Path, error: std::io::Error) -> GenerateError {
    GenerateError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A throwaway workspace with the two files the generator edits.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let unique = format!(
                "timelex-new-service-{}-{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let root = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(root.join("deploy/postgres/bootstrap")).unwrap();
            std::fs::write(
                root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/timelex-db\"]\n",
            )
            .unwrap();
            std::fs::write(
                root.join("deploy/postgres/bootstrap/004_provision_services.sql"),
                "-- Provision the services that exist.\nCALL platform.provision_service('identity');\n",
            )
            .unwrap();
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn read(&self, path: &str) -> String {
            std::fs::read_to_string(self.0.join(path)).unwrap()
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/")
    }

    #[test]
    fn names_the_database_accepts_are_accepted() {
        for name in ["host", "matters", "capture_hub", "a1", "x".repeat(31).as_str()] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn names_outside_the_bootstrap_rule_are_refused() {
        for name in [
            "",
            "a",
            "Host",
            "1host",
            "_host",
            "host-name",
            "host name",
            "host.name",
            "platform",
            "public",
            "pg_stat",
            "x".repeat(32).as_str(),
        ] {
            assert!(validate_name(name).is_err(), "{name:?} should be refused");
        }
    }

    #[test]
    fn generate_writes_the_service_and_patches_the_workspace() {
        let root = TempRoot::new();
        let report = generate(root.path(), "host").unwrap();

        let created: Vec<String> = report
            .created
            .iter()
            .map(|path| relative(root.path(), path))
            .collect();
        for expected in [
            "services/host/Cargo.toml",
            "services/host/README.md",
            "services/host/src/lib.rs",
            "services/host/src/main.rs",
            "services/host/src/error.rs",
            "services/host/src/http.rs",
            "services/host/src/store.rs",
            "services/host/migrations/0001_items.sql",
            "services/host/tests/http.rs",
        ] {
            assert!(
                created.iter().any(|path| path == expected),
                "missing {expected}: {created:?}"
            );
        }
        assert_eq!(created.len(), 9, "{created:?}");

        let manifest = root.read("services/host/Cargo.toml");
        assert!(manifest.contains("name = \"timelex-host-service\""));
        assert!(manifest.contains("timelex-telemetry"));
        let library = root.read("services/host/src/lib.rs");
        assert!(library.contains("sqlx::migrate!(\"./migrations\")"));
        let store = root.read("services/host/src/store.rs");
        assert!(store.contains("host.items"));
        let http = root.read("services/host/src/http.rs");
        assert!(http.contains("timelex_telemetry::instrument(router, \"host\")"));
        let migration = root.read("services/host/migrations/0001_items.sql");
        assert!(migration.contains("CALL platform.apply_tenant_rls('host.items')"));
        assert!(migration.contains("TO svc_host"));
        let tests = root.read("services/host/tests/http.rs");
        assert!(tests.contains("service: \"host\""));

        let workspace = root.read("Cargo.toml");
        assert!(workspace.contains("\"services/host\""));
        let bootstrap = root.read("deploy/postgres/bootstrap/004_provision_services.sql");
        assert!(bootstrap.contains("CALL platform.provision_service('host');"));
        assert_eq!(report.patched.len(), 2);
    }

    #[test]
    fn names_with_underscores_reach_every_identifier() {
        let root = TempRoot::new();
        generate(root.path(), "capture_hub").unwrap();
        let manifest = root.read("services/capture_hub/Cargo.toml");
        assert!(manifest.contains("name = \"timelex-capture_hub-service\""));
        let main = root.read("services/capture_hub/src/main.rs");
        assert!(main.contains("timelex_capture_hub_service"));
        assert!(main.contains("TIMELEX_CAPTURE_HUB_DATABASE_URL"));
        assert!(!main.contains("@"));
    }

    #[test]
    fn every_template_token_is_replaced() {
        let root = TempRoot::new();
        let report = generate(root.path(), "host").unwrap();
        for path in &report.created {
            let text = std::fs::read_to_string(path).unwrap();
            for token in ["@name@", "@crate@", "@ident@", "@UPPER@"] {
                assert!(!text.contains(token), "{} still has {token}", path.display());
            }
        }
    }

    #[test]
    fn generating_the_same_name_twice_is_refused() {
        let root = TempRoot::new();
        generate(root.path(), "host").unwrap();
        let error = generate(root.path(), "host").unwrap_err();
        assert_eq!(
            error,
            GenerateError::Exists(root.path().join("services").join("host"))
        );
    }

    #[test]
    fn existing_members_and_provisioning_lines_are_not_duplicated() {
        let root = TempRoot::new();
        generate(root.path(), "host").unwrap();
        generate(root.path(), "matters").unwrap();
        let workspace = root.read("Cargo.toml");
        assert_eq!(workspace.matches("\"services/host\"").count(), 1, "{workspace}");
        assert_eq!(
            workspace.matches("\"services/matters\"").count(),
            1,
            "{workspace}"
        );
        let bootstrap = root.read("deploy/postgres/bootstrap/004_provision_services.sql");
        assert_eq!(bootstrap.matches("provision_service('host')").count(), 1);
        assert_eq!(bootstrap.matches("provision_service('matters')").count(), 1);
    }

    #[test]
    fn a_workspace_without_the_members_list_is_reported_not_guessed() {
        let root = TempRoot::new();
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let error = generate(root.path(), "host").unwrap_err();
        assert!(matches!(
            error,
            GenerateError::MissingMarker {
                marker: "members = [",
                ..
            }
        ));
    }
}
