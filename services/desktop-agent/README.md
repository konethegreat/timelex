# desktop-agent

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | 🟡 specified (spike T2-R03 first) |
| **Language** | Rust (Windows first, `windows` crate) |
| **Phase** | 2 |
| **Owns data** | local only: config, queue of events not yet synced |

## Purpose
Small native agent that captures what email and calendar cannot: the document or site a fee earner is working in, and idle time.

## Responsibilities
- Foreground window and document title via Windows UI Automation; idle detection
- Local privacy filter: exclusion lists, redaction, personal-domain blocklist, user-visible pause
- Offline queue; syncs to `capture-hub` `/v1/events` over HTTPS with a device credential
- Signed binaries and signed updates

## Contract
- **REST:** none (client of `capture-hub`)
- **MCP tools:** none
- **Events published:** posts activity events
- **Events consumed:** tenant capture policy

## Non-goals
- No keystroke logging
- No screen recording by default
- Never sends anything the local filter excluded
- macOS comes later

## Run
Not runnable yet. See [TASKS.md](../../docs/TASKS.md) for the tasks that create it.

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
