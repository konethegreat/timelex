// Written by Kone Tshivhinda & Claude
//! Header-only MCP routing (ADR-0013): the tool table from `contracts/mcp/routes.json` and the
//! header rules from the 2026-07-28 specification.
//!
//! The gateway never reads the JSON-RPC body. It decides from `MCP-Protocol-Version`, `Mcp-Method`
//! and `Mcp-Name` alone, and the service re-checks the same headers against the body.

use std::collections::HashMap;

use serde::Deserialize;

/// The only MCP protocol version this build accepts.
pub const PROTOCOL_VERSION: &str = "2026-07-28";

#[derive(Deserialize)]
struct Entry {
    name: String,
    service: String,
}

#[derive(Deserialize)]
struct File {
    tools: Vec<Entry>,
}

/// Tool name to owning service.
pub struct Table {
    tools: HashMap<String, String>,
}

impl Table {
    /// Parse the routing table. A duplicate, empty or malformed entry fails start-up rather than
    /// sending a tool to the wrong service.
    pub fn parse(json: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(json)
            .map_err(|e| format!("contracts/mcp/routes.json is not valid for this shape: {e}"))?;
        let mut tools = HashMap::new();
        for entry in file.tools {
            if entry.name.is_empty() || entry.service.is_empty() {
                return Err("contracts/mcp/routes.json has a tool without a name or service".into());
            }
            if tools.insert(entry.name.clone(), entry.service).is_some() {
                return Err(format!(
                    "contracts/mcp/routes.json lists the tool {} twice",
                    entry.name
                ));
            }
        }
        if tools.is_empty() {
            return Err("contracts/mcp/routes.json lists no tools".into());
        }
        Ok(Self { tools })
    }

    /// The table compiled into this build, so a deployment carries no extra file to go stale.
    pub fn built_in() -> Result<Self, String> {
        Self::parse(include_str!("../../../contracts/mcp/routes.json"))
    }

    /// The service that owns `tool`, if the tool is in the catalogue.
    pub fn service_for(&self, tool: &str) -> Option<&str> {
        self.tools.get(tool).map(String::as_str)
    }
}

/// What the headers ask the gateway to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome<'a> {
    /// A `tools/call` for this tool name.
    ToolCall(&'a str),
    /// A valid MCP method this version does not route yet.
    Unrouted,
}

/// Decide from headers alone. An error is the `detail` of a 400 and never echoes an attacker's text.
pub fn classify<'a>(
    version: Option<&str>,
    method: Option<&str>,
    name: Option<&'a str>,
) -> Result<Outcome<'a>, &'static str> {
    if version != Some(PROTOCOL_VERSION) {
        return Err("MCP-Protocol-Version must be 2026-07-28");
    }
    let Some(method) = method.filter(|m| valid_method(m)) else {
        return Err("Mcp-Method is missing or malformed");
    };
    if method != "tools/call" {
        return Ok(Outcome::Unrouted);
    }
    let Some(name) = name.filter(|n| valid_name(n)) else {
        return Err("Mcp-Name is missing or malformed for tools/call");
    };
    Ok(Outcome::ToolCall(name))
}

/// `Mcp-Method`: 1 to 64 characters from `[A-Za-z0-9_./:-]`.
fn valid_method(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b':' | b'-'))
}

/// `Mcp-Name`: 1 to 128 characters from `[A-Za-z0-9_.-]`.
fn valid_name(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_table_covers_the_contract() {
        let table = Table::built_in().unwrap();
        assert_eq!(table.service_for("list_users"), Some("identity"));
        assert_eq!(table.service_for("get_trust_balance"), Some("trust"));
        assert_eq!(table.service_for("prepare_proforma"), Some("billing"));
        assert_eq!(table.service_for("no_such_tool"), None);
    }

    #[test]
    fn a_broken_table_is_refused() {
        for bad in [
            "",
            "{}",
            r#"{"tools": []}"#,
            r#"{"tools": [{"name": "", "service": "vault"}]}"#,
            r#"{"tools": [{"name": "t", "service": ""}]}"#,
            r#"{"tools": [{"name": "t", "service": "vault"}, {"name": "t", "service": "identity"}]}"#,
        ] {
            assert!(Table::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_tools_call_needs_the_right_version_method_and_name() {
        assert_eq!(
            classify(Some("2026-07-28"), Some("tools/call"), Some("list_users")),
            Ok(Outcome::ToolCall("list_users"))
        );
        let long_method = "x".repeat(65);
        let long_name = "n".repeat(129);
        for (version, method, name, detail) in [
            (
                None,
                Some("tools/call"),
                Some("list_users"),
                "MCP-Protocol-Version",
            ),
            (
                Some("2025-11-25"),
                Some("tools/call"),
                Some("list_users"),
                "MCP-Protocol-Version",
            ),
            (Some("2026-07-28"), None, Some("list_users"), "Mcp-Method"),
            (
                Some("2026-07-28"),
                Some("tools call"),
                Some("list_users"),
                "Mcp-Method",
            ),
            (
                Some("2026-07-28"),
                Some(long_method.as_str()),
                Some("list_users"),
                "Mcp-Method",
            ),
            (Some("2026-07-28"), Some("tools/call"), None, "Mcp-Name"),
            (
                Some("2026-07-28"),
                Some("tools/call"),
                Some("list users"),
                "Mcp-Name",
            ),
            (
                Some("2026-07-28"),
                Some("tools/call"),
                Some(long_name.as_str()),
                "Mcp-Name",
            ),
        ] {
            let err = classify(version, method, name).unwrap_err();
            assert!(err.contains(detail), "{version:?} {method:?} {name:?}: {err}");
        }
    }

    #[test]
    fn other_valid_methods_are_unrouted_rather_than_malformed() {
        for method in ["tools/list", "server/discover", "tasks/get", "prompts/get"] {
            assert_eq!(
                classify(Some("2026-07-28"), Some(method), None),
                Ok(Outcome::Unrouted),
                "{method}"
            );
        }
    }
}
