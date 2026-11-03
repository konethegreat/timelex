# matters

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

| | |
|---|---|
| **Status** | ✅ minimal client and matter records, aliases, rate cards and CSV import (P3-04) |
| **Language** | Rust |
| **Owns data** | `matters` schema: `clients`, `matters`, `aliases`, `rate_cards` |
| **Phase** | 3 (minimal), 6 (conflicts and contacts) |
| **Contract** | [`contracts/openapi/matters.v1.yaml`](../../contracts/openapi/matters.v1.yaml) 0.1.0 |
| **Decision** | [ADR-0027](../../docs/adr/0027-matters-natural-keys-and-import.md) |

## Purpose

Who the clients are, what the matters are, and what they cost. Attribution needs this service before
it can name anything: a work signal carries a domain, a counterparty or a document title, and the
matters service is the source of truth that turns those into a client and a matter.

## What it does

- **Clients and matters, keyed the way the firm keys them.** `clientId` and `matterId` are the firm's
  own identifiers (1 to 64 characters from `A-Z a-z 0-9 . _ : -`). There is no surrogate public id, so
  an export from another practice system round-trips and the gold set's `cl_…` / `mt_…` labels need no
  mapping.
- **References are canonical and unique.** A matter's `reference` is stored with a trimmed,
  lower-cased `reference_key` that is unique within the firm; a second matter claiming it is a 409.
- **Aliases are explicit hard keys.** `matter_reference`, `client_name` and `client_domain` values are
  canonicalised and each names exactly one target. The same value for the same target is idempotent;
  the same value for another target is a 409. Creating a matter does not invent aliases, so a renamed
  reference cannot leave a stale resolution behind.
- **One rate card per matter.** Hourly rate in whole ZAR cents, unit minutes, rounding rule and
  minimum units; `PUT` replaces it. Matter-specific discounts and fee arrangements are P5-01.
- **CSV import.** One record kind per file (`clients`, `matters`, `aliases`, `rate-cards`), strict
  headers, at most 5 000 rows and 2 MiB, applied in one transaction as upserts by natural key. A bad
  row fails the whole file with a logical row number and no cell value echoed. Load clients, then
  matters, then aliases and rate cards.

## Contract

Private REST surface, no public gateway route yet:

- `GET /health`
- `GET|POST /v1/clients`
- `GET|POST /v1/matters` (`?clientId=` to narrow)
- `GET /v1/rate-cards`, `PUT /v1/matters/{matterId}/rate-card`
- `GET|POST /v1/aliases` (`?kind=` and `?value=` to narrow)
- `POST /v1/import/{kind}` with `Content-Type: text/csv`

Reads require a `principal` internal token with `time:read`; writes and imports require
`settings:manage`. The firm comes from the verified token. Every query runs inside a tenant
transaction under row-level security (ADR-0011).

## Not yet

- Gateway routes and a public contract version. The service listens on its private interface only.
- `matter.opened` / `matter.closed` events; they need an event contract and a consumer.
- Contacts, the conflict index and `POST /v1/conflicts/check` (Phase 6).
- MCP tools `search_matters` and `check_conflicts`.
- Practice-system adapters (ADR-0009); CSV is the import path for now.

## Run the tests

Need a throwaway Postgres: see [deploy/postgres/README.md](../../deploy/postgres/README.md).

```
TIMELEX_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres cargo test -p timelex-matters-service
```

The suite covers domain validation, the CSV parser, the HTTP surface with permissions and tenancy,
transaction rollback, idempotent re-import, and the full gold-set firm (8 clients, 24 matters, 32
aliases, 24 rate cards).

## Run the service

```
TIMELEX_MATTERS_DATABASE_URL=postgres://svc_matters:<password>@127.0.0.1:55433/postgres \
TIMELEX_GATEWAY_PUBLIC_KEYS=k1=<base64 public key> \
TIMELEX_MATTERS_ADDR=127.0.0.1:8083 \
cargo run -p timelex-matters-service
```

The connection string is a shape, not a working URL: put the real role password in the environment,
never in a file in the repository. Apply migrations first with
`cargo run -p timelex-matters-service -- migrate` and `TIMELEX_MIGRATOR_DATABASE_URL`.

| Variable | Used by | Meaning |
|---|---|---|
| `TIMELEX_MATTERS_DATABASE_URL` | serve | connection URL for the `svc_matters` role |
| `TIMELEX_MIGRATOR_DATABASE_URL` | migrate | connection URL for `timelex_migrator` |
| `TIMELEX_GATEWAY_PUBLIC_KEYS` | serve | gateway public keys (`kid=base64,...`) |
| `TIMELEX_MATTERS_ADDR` | serve | listen address, default `127.0.0.1:8083` |

See also: [ARCHITECTURE.md](../../docs/ARCHITECTURE.md), [ADR-0001](../../docs/adr/0001-service-boundaries-and-contract-first.md).
