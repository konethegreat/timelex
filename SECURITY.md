# Security Policy

TimeLex is designed to handle confidential legal and financial data and to store customers' API keys. It is **pre-alpha**: the security controls in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) are a plan, not yet a claim.

## Reporting a vulnerability
Please report privately through [GitHub Security Advisories](https://github.com/konethegreat/timelex/security/advisories/new). **Do not open public issues for security problems, and do not include real client data or live API keys in a report.**

You can expect an acknowledgement within 5 working days and a first assessment within 15 working days. We will credit reporters who want credit.

## Scope
In scope: this repository's code, contracts, CI configuration and documentation claims that could mislead users into an unsafe configuration.
Out of scope for now: third-party services (Microsoft, Google, Anthropic) and anything requiring a hosted TimeLex tenant (none exists yet).

## Supported versions
Only `main` is supported while the project is 0.x.

## If you think you have exposed a key
Revoke it at the provider first (Anthropic Console, Microsoft Entra, Google Cloud), then report here if the exposure involved TimeLex code.

_Written by Kone Tshivhinda & Claude_
