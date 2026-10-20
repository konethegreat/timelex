# ADR-0002: Language allocation: Rust, Python, TypeScript

Status: Accepted · Date: 2026-10-20 · Deciders: Kone Tshivhinda, Claude

## Context
The brief asks for "the fastest programming languages like Rust, Python, etc." in a multi-language approach. Our workloads are I/O- and model-latency-bound, so raw speed matters in few places. ClearCut already settled on Rust, TypeScript and a thin Python worker with a three-language ceiling ([ClearCut ADR-0001](https://github.com/konethegreat/clearcut/blob/main/docs/adr/0001-multi-language-stack.md)). Kone is new to Rust, develops on Windows, and the target firms are Windows/Microsoft 365 shops.

## Decision
| Language | Owns | Why |
|---|---|---|
| **Rust** | Domain rules and state machines (units, ledger, policy), gateway, `mcp-host`, `vault`, `capture-hub`, desktop agent, services' HTTP/MCP layers | Correctness for money and policy; memory safety for key handling and the desktop agent; predictable latency; small static binaries (the Windows agent); official MCP SDK is Rust-capable (beta) |
| **Python** | OCR, embeddings/retrieval, calibration, evaluation harness, redaction, forecasting later | Ecosystem. **No business rules** in Python; workers are stateless functions behind a typed contract |
| **TypeScript** | Web app, Outlook add-in (Office.js), SDK, Claude plugin packaging | Ecosystem; the Outlook add-in cannot be anything else |

Rules:
1. **Three languages is the ceiling.** A fourth needs a new ADR that names what the three cannot do.
2. Rust services keep domain crates **I/O-free** and use async only in transport layers (Kone is learning Rust).
3. Cross-language types come from **JSON Schema in `contracts/`**; Rust types derive/validate against it, and the SDK is generated from OpenAPI. A contract test in CI fails on drift (this ADR's check is `crates/timelex-types`'s example-validation test).
4. The desktop agent is Rust (`windows` crate for UI Automation and foreground-window events), Windows first, macOS later.

## Alternatives considered
- *Go* for services: fast to write and deploy, but overlaps Rust; rejected as in ClearCut.
- *C#/.NET* for the Windows agent and Graph connectors: excellent Microsoft integration, but a fourth language for one developer. Revisit only if Rust's Windows UI Automation story proves painful (T2-R03 spike).
- *All Python:* quickest start; weakest guarantees for money, policy and key handling; poor fit for a small native agent.
- *All TypeScript/Node:* simple hiring; rejected for the same correctness reasons.

## Consequences
- CI has three toolchains (cargo, uv/pytest, npm). Dependabot covers all three.
- Cross-language drift is the main risk; contract tests are mandatory.
- Rust's learning curve: pair-review early PRs, keep the core simple, prefer clarity to cleverness.
