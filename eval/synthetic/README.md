# Synthetic gold set

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

A deterministic, labelled, entirely fictional law firm used to measure capture, segmentation and
attribution ([task P3-01](../../docs/TASKS.md), [research 04 §6](../../docs/research/04-autonomous-capture-design.md)).
The corpus is public and lives in [`corpus/`](corpus). The same seed always produces the same bytes,
so a change to the generator is visible as a diff in review.

Nothing here is real. Every name is invented, every address ends in a reserved domain (`.example` or
`.test`), and no content body, credential or personal data is present.

## What it contains

| File | Content |
|---|---|
| `firm.json` | One tenant, six users, eight clients, 24 matters, rate cards, the firm's domain exclusions and the 20 business days |
| `threads.json` | Mail threads: subject, thread reference, sender, recipients, copied recipients and sent time |
| `calendar.json` | Meetings: subject, organizer, attendees, start and end |
| `events.json` | Activity events in schema `1.0`, with canonical ids computed exactly as `contracts/events/README.md` requires |
| `labels.json` | One ground-truth label per event, in the same order |
| `manifest.json` | Generator version, seed, counts, and a SHA-256 and byte count per file |

Events cover `m365_mail`, `m365_calendar`, `desktop_agent` and `manual`, with kinds `email_sent`,
`email_received`, `meeting`, `call`, `document_edit`, `document_view` and `manual_entry`.

## Labels

Each label has:

| Field | Meaning |
|---|---|
| `eventId` | The event this label describes |
| `expectedStorage` | `stored`, or `excluded` when the firm's domain policy must drop the signal |
| `expectedOutcome` | `auto_post`, `review`, `not_billable` or `excluded` |
| `clientId`, `matterId` | The correct attribution; both are `null` for excluded and internal signals |
| `billable` | Whether the work is chargeable |
| `evidence` | `hard_key`, `counterparty`, `content`, `history` or `none` |
| `candidateMatterIds` | Every matter an honest reader could choose from the signal alone |
| `ambiguous` | True when more than one open matter is a candidate |

`expectedOutcome` is the honest target for the signal, not a promise about any one attribution stage.
A matter reference in the subject or filename is a hard key and may auto-post. A subject that names
only the client, or a document title that names only the client, is ambiguous when that client has
more than one open matter, so it expects review. A signal on a closed matter always expects review.
Internal and newsletter signals are stored but not billable. A signal from an excluded domain is
never stored.

## What it deliberately includes

- Hard keys (matter references in mail subjects and file names) for the auto-post path.
- Client-only signals with several open matters, so precision is not measured on easy cases.
- Two closed matters, so the deterministic open-matter check has something to catch.
- Excluded domains (`adverse.example.test`, `personal.example.test`) in mail and calendar.
- Internal firm-only mail and meetings, plus external newsletters that are stored but not billable.
- One non-billable courtesy call that still names a client and matter.
- Metadata-only privacy flags everywhere, matching the platform's current gate.

## Limits

- No message bodies or attachments. Attribution that depends on reading content is out of scope
  until the redaction and content path exists; this corpus measures the metadata stages first.
- No assignment history. The `history` evidence value is reserved but not generated, so stage 1 is
  not exercised yet.
- One firm and one 20-day window. It is a regression corpus, not a survey of practice shapes.
- Excluded signals are present in `events.json` because a connector would observe them; a capture
  run must drop them. That is the point of the `excluded` label.

## Use

```bash
cd eval/synthetic
uv sync --locked
uv run python -m timelex_goldset generate --seed 20261102 --out corpus
uv run python -m timelex_goldset check --corpus corpus
uv run pytest -q
```

The interpreter is pinned to CPython 3.12 in `.python-version`. The standard library guarantees the
`random()` stream across versions but not the higher-level helpers the generator uses, so the pinned
interpreter is part of what makes the committed bytes reproducible.

`generate` rewrites the five corpus files and the manifest. `check` regenerates in memory and fails
if any committed file differs, is missing, or is unexpected. CI runs `check`, so an edited corpus
cannot pass without the generator changing too.

To build a different firm, pass another seed and a different output directory:

```bash
uv run python -m timelex_goldset generate --seed 7 --out /tmp/gold-set-7
```

## Manifest and drift

The manifest records the generator version and seed, plus the digest of every file it wrote. A
generator change that alters output changes the digests; commit the regenerated corpus and the code
in the same change. Line endings are normalised to LF in the corpus so a Windows checkout does not
look like drift.
