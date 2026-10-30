// Written by Kone Tshivhinda & Claude
//! Contract checks that need no running service: the MCP routing table must match the catalogue in
//! `contracts/mcp/tools.md`. The gateway routes `tools/call` from this table (ADR-0013), so a tool
//! that exists in one file and not the other would be a silent hole in the outward MCP face.

use std::collections::BTreeSet;

use serde::Deserialize;

#[derive(Deserialize)]
struct Routes {
    version: String,
    tools: Vec<Route>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Route {
    name: String,
    service: String,
    read_only: bool,
}

fn routes() -> Routes {
    serde_json::from_str(include_str!("../../../contracts/mcp/routes.json"))
        .expect("contracts/mcp/routes.json is not valid JSON for this shape")
}

/// `(service, tool, read_only)` from the catalogue table in `contracts/mcp/tools.md`.
fn catalogue() -> BTreeSet<(String, String, bool)> {
    let mut out = BTreeSet::new();
    for line in include_str!("../../../contracts/mcp/tools.md").lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let columns: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if columns.len() != 4 {
            continue;
        }
        let (service, tool, access) = (columns[0], columns[1].trim_matches('`'), columns[2]);
        if service == "Service" || service.starts_with("---") {
            continue;
        }
        let read_only = match access {
            "R" => true,
            "W" => false,
            other => panic!("catalogue row {service}/{tool} has access {other:?}, expected R or W"),
        };
        assert!(
            out.insert((service.to_owned(), tool.to_owned(), read_only)),
            "duplicate catalogue row {service}/{tool}"
        );
    }
    assert!(
        !out.is_empty(),
        "no catalogue rows found in contracts/mcp/tools.md"
    );
    out
}

#[test]
fn mcp_routes_match_the_catalogue() {
    let routes = routes();
    assert_eq!(routes.version, "0.1.0");

    let from_json: BTreeSet<(String, String, bool)> = routes
        .tools
        .iter()
        .map(|r| (r.service.clone(), r.name.clone(), r.read_only))
        .collect();
    assert_eq!(
        from_json.len(),
        routes.tools.len(),
        "a tool appears twice in contracts/mcp/routes.json"
    );
    assert_eq!(
        from_json,
        catalogue(),
        "contracts/mcp/routes.json and contracts/mcp/tools.md disagree; update both"
    );
}

#[test]
fn every_route_names_a_tool_and_a_service() {
    for route in routes().tools {
        assert!(!route.name.is_empty(), "empty tool name");
        assert!(!route.service.is_empty(), "tool {} has no service", route.name);
        assert!(
            route
                .service
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "tool {} names an invalid service {:?}",
            route.name,
            route.service
        );
    }
}
