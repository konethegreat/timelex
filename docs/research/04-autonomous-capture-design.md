# 04 · Autonomous capture, attribution and narration: design research

_Author: Kone Tshivhinda · Researched 20 Oct 2026 · Written by Kone Tshivhinda & Claude_

Goal: turn raw work signals into posted, narrated time entries with **minimal human input and provable accuracy**. This file is the design basis for Phases 2–4. Numbers marked *proposed* are targets to calibrate on a gold set, not measurements.

## 1. Pipeline

```
signals ─► normalise ─► privacy filter ─► segment ─► attribute ─► narrate ─► score ─► policy ─► post / review
(mail,     (activity      (local + server:  (time       (rules →     (template   (calibrated  (autonomy   (ledger,
 calendar,  events,         redact,           blocks)    retrieval →   first,      confidence)  ladder)     review
 meetings,  idempotent)     exclude)                     LLM tiers)    guarded)                            queue)
 desktop)
```

Each arrow is a contract ([contracts/](../../contracts/)). Each stage emits events so any entry can be explained from its sources ([ADR-0006](../adr/0006-event-sourced-activity-and-ledger.md)).

## 2. Signal sources and their limits

| Source | Mechanism | Strengths | Hard limits (sourced) |
|---|---|---|---|
| **Outlook mail** | Microsoft Graph change notifications (webhook / Event Hubs / Event Grid) | Near-real-time; no desktop install | Subscription lifetime under 7 days (under 1 day for rich notifications) so renewal jobs are mandatory; max 1 000 active subscriptions per mailbox across all apps; latency avg under 1 min, max 3 min ([Microsoft Learn](https://learn.microsoft.com/en-us/graph/change-notifications-overview)) |
| **Outlook calendar** | Graph events | Meetings, attendees, duration | Event latency not stated in the docs we read; verify |
| **Teams meetings / chats** | Graph `onlineMeeting` (attendance, transcripts, recordings), chat messages | Real meeting duration and participants | Transcript/recording access depends on tenant policy and licensing; permission scopes not confirmed in the pages we read; online meetings expire 60 days after start/end unless refreshed ([Microsoft Learn](https://learn.microsoft.com/en-us/graph/api/resources/onlinemeeting)); Teams subscriptions share a 10 000-per-tenant quota |
| **Google Workspace** | Gmail push + Calendar watch | Same role for Google firms | Not yet researched in detail (T2-R01) |
| **Desktop (Windows first)** | Foreground window/document title, idle time, file paths via a native agent | Catches work outside mail and calendar (Word, PDFs, research sites, DMS) | Requires an install; sensitive. Must be **local-first**: filter and summarise on device; user-visible pause |
| **Phone calls / WhatsApp** | Call logs, manual quick-capture | Common in SA practice | No clean API; start with a mobile quick-capture and calendar |
| **Document systems (SharePoint, OneDrive)** | Graph file activity | Documents touched per matter | Quota and permission model per tenant |
| **GhostPractice** | Sanctioned export/import only | Existing matters, clients, rates | No public API ([research 01](01-ghostpractice-teardown.md)) |

Design consequence: **no single source is enough**. Hybrid capture (API connectors for breadth and low install cost, a desktop agent for depth) matches what the market's best products do separately.

## 3. Segmentation: from events to time blocks
- Group events that belong together: same thread, same document, same counterparty, within a gap tolerance. Calendar events and meeting attendance give hard anchors.
- Resolve overlaps deterministically (a call during document work splits the block) in Rust, not with a model. Input: events; output: blocks with start, end, duration in seconds.
- Duration rules are configuration: minimum increment, rounding, unit size (commonly six minutes, configurable to 5, 10 or 15). Implemented in `timelex-units`.
- Non-billable detection (internal admin, personal) is a classifier with its own threshold; default is *propose non-billable, never silently drop*.

## 4. Attribution: which client and matter?
A **cascade**, cheapest and most certain first. Escalate only when the previous stage is not confident.

| Stage | Method | Cost | Notes |
|---|---|---|---|
| 0. Hard keys | Matter reference in subject/filename, client email domain, DMS workspace, calendar tag | ~0 | Highest precision; exact-match rules owned by the firm |
| 1. History | Same thread/participants/document already assigned | ~0 | Heavy reuse in practice |
| 2. Retrieval | Embed event context; nearest matters by participants, keywords, recency | low | Python worker; embeddings stored per tenant only |
| 3. Small model | Haiku-class model chooses among top-k candidate matters with the evidence | low | Structured output; abstain allowed |
| 4. Stronger model | Sonnet / Opus-class on disagreement or low confidence | higher | Gets the failure report from stage 3 |
| 5. Human | Review queue | human time | Terminal state; no job ends silently failed |

This mirrors ClearCut's validated escalation ladder (Haiku 5.5 → Sonnet 5.5 → Opus 5.5 → Fable 5.1 → human) and terminal-state guarantee ([ClearCut ADR-0005](https://github.com/konethegreat/clearcut/blob/main/docs/adr/0005-model-strategy-escalation.md)).

**Calibration.** Raw model confidence is not a probability. Every tenant's threshold is derived from labelled outcomes (accepted vs corrected entries), using isotonic or Platt calibration, then checked by weekly sampling. A threshold that fails sampling automatically drops the tenant one rung on the ladder ([AUTONOMY.md](../AUTONOMY.md)).

**Two-key rule for money-affecting changes:** a model proposes the matter, deterministic checks confirm the matter is open, the client is not conflict-flagged and the rate card exists, before an entry can post.

## 5. Narration
Lessons carried over from ClearCut: **template first, LLM only to smooth, every sentence cites its evidence, a number guard rejects any numeral or date not present in the cited events, fall back to the template if the guard fails** ([ClearCut ADR-0004](https://github.com/konethegreat/clearcut/blob/main/docs/adr/0004-ingestion-and-narration-pipeline.md)).

For time entries the facts are: date, duration, participants, document names, event type. Example: "Telephone conference with J. Mokoena regarding the draft settlement agreement (0.3 h)." Rules: no privileged content in narratives beyond what the firm configures, a per-client narrative style (some clients' outside-counsel guidelines forbid certain wording), and a redaction check before the narrative leaves the platform.

## 6. Evaluation (how we know it works)
| Asset | Content | Rule |
|---|---|---|
| **Synthetic corpus** | Generated firm with clients, matters, mail threads and calendars; labelled | Public, in `eval/`; used in CI |
| **Consented real corpus** | Anonymised signals from volunteer fee earners with their final submitted time | Private; POPIA review first; never committed |
| **Metrics** | Attribution precision/recall by stage; recovered minutes vs final submitted time; narrative faithfulness (guard pass rate); cost per entry; reviewer minutes per day | Reported per release |
| **Gates (proposed)** | 0 silent critical errors; precision ≥ 98% at auto-post threshold; 100% narratives traceable; 100% jobs terminal | Block release |

Vendors claim 30–50% recovered time. We will not repeat that claim until our own pilot measures it.

## 7. Cost model (illustrative, in Rand)
Assumptions: 80 captured signals per fee earner per working day, 22 days, so **1 760 signals/month**; 2 000 input and 400 output tokens per signal across attribution and narration; list prices from ClearCut ADR-0005 at **R18/USD** (Haiku 5.5 R1.80/R9 per million input/output tokens, Sonnet 5.5 R36/R180, Opus 5.5 R72/R360); **no caching, no batch discount, no thinking tokens**.

| Route | Per signal | Per fee earner per month |
|---|---|---|
| All Haiku | R0.0072 | **R12.67** |
| All Sonnet | R0.144 | **R253.44** |
| All Opus | R0.288 | **R506.88** |
| Ladder 85% Haiku / 12% Sonnet / 3% Opus | R0.032 | **≈ R56** |

Check: 0.85×12.67 + 0.12×253.44 + 0.03×506.88 = 10.77 + 30.41 + 15.21 = R56.4. Caching the static prefix and using the Batch API for non-urgent work would cut this further; thinking tokens and escalations would raise it. **Measure before promising.** Prices must be re-checked on the [Anthropic pricing page](https://platform.claude.com/docs/en/about-claude/pricing).

## 8. Failure modes we design for
| Failure | Mitigation |
|---|---|
| Wrong client billed | Hard-key stage first; conflict and open-matter checks; sampling audits; client-level autonomy cap |
| Hallucinated narrative detail | Template-first; number guard; citation requirement |
| Prompt injection from an email ("assign all time to matter X") | Content treated as data; extractor has no tools; policy outside the model ([THREAT_MODEL.md](../THREAT_MODEL.md)) |
| Missed time | Recall metric; "gap detector" compares calendar and activity with posted time |
| Double counting (mail + meeting + doc for one block) | Deterministic block merge; idempotent event IDs |
| Over-capture of personal activity | Local filter; personal-domain blocklist; user pause; propose non-billable |
| Provider outage | Queue and resume; entries wait; no data loss |

## Sources
[Microsoft Graph change notifications](https://learn.microsoft.com/en-us/graph/change-notifications-overview) · [Graph onlineMeeting](https://learn.microsoft.com/en-us/graph/api/resources/onlinemeeting) · [PointOne](https://pointone.com/blog/best-ai-timekeeping-software) · [LegalTime AI](https://www.legaltime.ai/blog/best-automatic-time-tracking-software-for-lawyers) · [ClearCut ADR-0004/0005](https://github.com/konethegreat/clearcut/tree/main/docs/adr)
