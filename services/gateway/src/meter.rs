// Written by Kone Tshivhinda & Claude
//! Request metering: one metadata record per handled request (ADR-0013).
//!
//! Never the body, the credential or anything personal: only who (ids and kind), which route and
//! service, what happened (status, duration, bytes) and whether a limit refused it. Records go to one
//! JSON line on stdout, and running totals are available on the loopback `/internal/usage` listener.
//!
//! Totals live in memory and reset on restart, so they are an operational signal, not a billing record.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

/// What kind of caller made the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CallerKind {
    /// No usable credential: the request was refused before the caller could be named.
    Unidentified,
    /// The platform operator, holding one of the configured operator keys.
    Operator,
    /// The gateway itself. Public routes never mint this kind; the variant keeps the record honest.
    Service,
    /// A firm's user or machine, identified by its API key.
    Principal,
}

/// One handled request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord<'a> {
    /// When the gateway answered, RFC 3339.
    pub at: String,
    pub caller: CallerKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<Uuid>,
    /// The gateway route, for example `apiKeys.issue` or `mcp.call`.
    pub route: &'static str,
    /// The service the request was aimed at; absent when it was refused before routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<&'a str>,
    /// The MCP tool for a `tools/call`, once the routing table named its service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<&'a str>,
    pub method: String,
    pub status: u16,
    pub duration_ms: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub rate_limited: bool,
}

/// Per-firm (or per-operator) running totals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub requests: u64,
    pub rate_limited: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    requests: u64,
    rate_limited: u64,
    unidentified: u64,
    services: u64,
    bytes_in: u64,
    bytes_out: u64,
    operator: UsageTotals,
    tenants: BTreeMap<Uuid, UsageTotals>,
}

/// What `/internal/usage` answers.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    /// When this process started, RFC 3339.
    pub since: String,
    /// Seconds since start, so nobody mistakes the totals for a long history.
    pub uptime_seconds: u64,
    /// Every handled request, including refused ones.
    pub totals: TotalsView,
    /// The platform operator's share.
    pub operator: UsageTotals,
    /// Each firm that has made a request, by tenant id.
    pub tenants: BTreeMap<Uuid, UsageTotals>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalsView {
    pub requests: u64,
    pub rate_limited: u64,
    pub unidentified: u64,
    pub services: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

type Sink = Box<dyn Fn(&str) + Send + Sync>;

/// Where records go and what they add up to.
pub struct Meter {
    started: Instant,
    since: String,
    totals: Mutex<Totals>,
    sink: Sink,
}

impl Meter {
    /// The production meter: one JSON line per request on stdout.
    pub fn stdout() -> Self {
        Self::with_sink(|line| println!("{line}"))
    }

    /// A meter that hands each line to `sink`. Used by tests to keep stdout quiet and read records back.
    pub fn with_sink(sink: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self {
            started: Instant::now(),
            since: now_rfc3339(),
            totals: Mutex::new(Totals::default()),
            sink: Box::new(sink),
        }
    }

    /// Emit one record and add it to the running totals.
    pub fn record(&self, record: &UsageRecord<'_>) {
        // Serialising a fixed struct cannot fail; if it somehow did, the totals below still count it.
        if let Ok(line) = serde_json::to_string(record) {
            (self.sink)(&line);
        }
        let mut totals = self.totals.lock().unwrap_or_else(|e| e.into_inner());
        totals.requests += 1;
        totals.rate_limited += u64::from(record.rate_limited);
        totals.bytes_in += record.bytes_in;
        totals.bytes_out += record.bytes_out;
        match record.caller {
            CallerKind::Unidentified => totals.unidentified += 1,
            CallerKind::Service => totals.services += 1,
            CallerKind::Operator => add(&mut totals.operator, record),
            CallerKind::Principal => {
                if let Some(tenant) = record.tenant_id {
                    add(totals.tenants.entry(tenant).or_default(), record);
                }
            }
        }
    }

    /// The running totals as `/internal/usage` shows them.
    pub fn snapshot(&self) -> UsageSnapshot {
        let totals = self.totals.lock().unwrap_or_else(|e| e.into_inner());
        UsageSnapshot {
            since: self.since.clone(),
            uptime_seconds: self.started.elapsed().as_secs(),
            totals: TotalsView {
                requests: totals.requests,
                rate_limited: totals.rate_limited,
                unidentified: totals.unidentified,
                services: totals.services,
                bytes_in: totals.bytes_in,
                bytes_out: totals.bytes_out,
            },
            operator: totals.operator.clone(),
            tenants: totals.tenants.clone(),
        }
    }
}

fn add(usage: &mut UsageTotals, record: &UsageRecord<'_>) {
    usage.requests += 1;
    usage.rate_limited += u64::from(record.rate_limited);
    usage.bytes_in += record.bytes_in;
    usage.bytes_out += record.bytes_out;
}

/// The current time as RFC 3339. An empty string can only follow an impossible calendar error.
pub(crate) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn record(caller: CallerKind, tenant: Option<u128>, rate_limited: bool) -> UsageRecord<'static> {
        UsageRecord {
            at: "2026-10-30T09:00:00Z".into(),
            caller,
            tenant_id: tenant.map(Uuid::from_u128),
            user_id: tenant.map(|t| Uuid::from_u128(t + 1)),
            key_id: tenant.map(|t| Uuid::from_u128(t + 2)),
            route: "apiKeys.issue",
            service: Some("identity"),
            tool: None,
            method: "POST".into(),
            status: if rate_limited { 429 } else { 201 },
            duration_ms: 3,
            bytes_in: 40,
            bytes_out: 80,
            rate_limited,
        }
    }

    fn meter() -> (Meter, Arc<Mutex<Vec<String>>>) {
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = lines.clone();
        (
            Meter::with_sink(move |line| sink.lock().unwrap().push(line.to_owned())),
            lines,
        )
    }

    #[test]
    fn each_record_is_one_json_line_and_totals_are_kept_per_firm() {
        let (m, lines) = meter();
        m.record(&record(CallerKind::Principal, Some(11), false));
        m.record(&record(CallerKind::Principal, Some(11), true));
        m.record(&record(CallerKind::Principal, Some(12), false));
        m.record(&record(CallerKind::Operator, None, false));
        m.record(&record(CallerKind::Unidentified, None, false));

        let lines = lines.lock().unwrap();
        assert_eq!(lines.len(), 5);
        let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(first["caller"], "principal");
        assert_eq!(first["tenantId"], Uuid::from_u128(11).to_string());
        assert_eq!(first["route"], "apiKeys.issue");
        assert_eq!(first["service"], "identity");
        assert_eq!(first["status"], 201);
        assert_eq!(first["rateLimited"], false);
        assert!(first.get("body").is_none(), "no body field exists");
        assert!(first.get("tool").is_none(), "no tool for a REST call");

        let s = m.snapshot();
        assert_eq!(s.totals.requests, 5);
        assert_eq!(s.totals.rate_limited, 1);
        assert_eq!(s.totals.unidentified, 1);
        assert_eq!(s.totals.bytes_in, 5 * 40);
        assert_eq!(s.totals.bytes_out, 5 * 80);
        assert_eq!(s.operator.requests, 1);
        assert_eq!(s.tenants[&Uuid::from_u128(11)].requests, 2);
        assert_eq!(s.tenants[&Uuid::from_u128(11)].rate_limited, 1);
        assert_eq!(s.tenants[&Uuid::from_u128(12)].requests, 1);
        assert_eq!(s.tenants.len(), 2);
    }

    #[test]
    fn an_mcp_record_names_the_tool_and_its_service() {
        let (m, lines) = meter();
        let record = UsageRecord {
            route: "mcp.call",
            service: Some("time-ledger"),
            tool: Some("propose_entry"),
            ..record(CallerKind::Principal, Some(11), false)
        };
        m.record(&record);
        let json: serde_json::Value = serde_json::from_str(&lines.lock().unwrap()[0]).unwrap();
        assert_eq!(json["route"], "mcp.call");
        assert_eq!(json["service"], "time-ledger");
        assert_eq!(json["tool"], "propose_entry");
    }

    #[test]
    fn a_snapshot_says_when_it_started_and_serialises_tenant_keys_as_ids() {
        let (m, _) = meter();
        m.record(&record(CallerKind::Principal, Some(11), false));
        let json = serde_json::to_value(m.snapshot()).unwrap();
        assert!(json["since"].as_str().unwrap().ends_with('Z'));
        assert!(json["uptimeSeconds"].is_u64());
        let key = Uuid::from_u128(11).to_string();
        assert_eq!(json["tenants"][&key]["requests"], 1);
        assert_eq!(json["totals"]["unidentified"], 0);
    }
}
