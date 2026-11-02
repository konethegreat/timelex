# Written by Kone Tshivhinda & Claude
import json
from collections import Counter
from datetime import datetime
from pathlib import Path

import pytest
from jsonschema import FormatChecker
from jsonschema.validators import Draft202012Validator

from timelex_goldset import activity_event_id, canonical_bytes, generate

SEED = 20261102
ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads((ROOT / "contracts/events/activity-event.schema.json").read_text())
VALIDATOR = Draft202012Validator(SCHEMA, format_checker=FormatChecker())


@pytest.fixture(scope="module")
def corpus():
    return generate(SEED)


def parse_time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def test_the_same_seed_produces_byte_identical_files():
    first = generate(SEED)
    second = generate(SEED)
    assert {name: canonical_bytes(body) for name, body in first.files().items()} == {
        name: canonical_bytes(body) for name, body in second.files().items()
    }
    assert canonical_bytes(first.manifest()) == canonical_bytes(second.manifest())


def test_a_different_seed_changes_the_firm_and_every_event_id():
    first = generate(SEED)
    second = generate(SEED + 1)
    assert first.firm.tenant_id != second.firm.tenant_id
    assert {event["id"] for event in first.events}.isdisjoint(
        event["id"] for event in second.events
    )


def test_every_event_matches_the_activity_event_schema(corpus):
    for event in corpus.events:
        errors = sorted(VALIDATOR.iter_errors(event), key=lambda error: list(error.path))
        assert not errors, f"{event['id']}: {errors[0].message}"


def test_every_event_id_is_the_canonical_id_of_its_facts(corpus):
    for event in corpus.events:
        expected = activity_event_id(
            event["tenantId"],
            event["source"]["system"],
            event["source"]["sourceId"],
            parse_time(event["startedAt"]),
        )
        assert event["id"] == expected


def test_labels_cover_every_event_once_and_in_the_same_order(corpus):
    assert [label["eventId"] for label in corpus.labels] == [event["id"] for event in corpus.events]
    assert len({label["eventId"] for label in corpus.labels}) == len(corpus.events)


def test_mail_and_calendar_events_name_a_signal_that_exists(corpus):
    messages = {m["messageId"] for thread in corpus.threads for m in thread["messages"]}
    meetings = {m["meetingId"] for m in corpus.calendar}
    mail_events = set()
    meeting_events = set()
    for event in corpus.events:
        system = event["source"]["system"]
        if system == "m365_mail":
            mail_events.add(event["source"]["sourceId"])
        elif system == "m365_calendar":
            meeting_events.add(event["source"]["sourceId"])
    assert mail_events == messages
    assert meeting_events == meetings


def test_excluded_domains_are_never_labelled_as_stored(corpus):
    excluded = {e.value for e in corpus.firm.exclusions if e.kind == "domain"}
    excluded_events = 0
    for event, label in zip(corpus.events, corpus.labels, strict=True):
        domains = {
            participant["domain"]
            for participant in event.get("subject", {}).get("participants", [])
        }
        touches_exclusion = bool(domains & excluded)
        if touches_exclusion:
            excluded_events += 1
            assert label["expectedStorage"] == "excluded", event["id"]
            assert label["expectedOutcome"] == "excluded"
            assert label["clientId"] is None and label["matterId"] is None
            assert label["billable"] is False
        else:
            assert label["expectedStorage"] == "stored", event["id"]
    assert excluded_events >= 4


def test_an_ambiguous_event_names_every_open_candidate(corpus):
    matters = {matter.matter_id: matter for matter in corpus.firm.matters}
    ambiguous = [label for label in corpus.labels if label["ambiguous"]]
    assert len(ambiguous) >= 2
    for label in ambiguous:
        candidates = label["candidateMatterIds"]
        assert len(candidates) >= 2
        assert label["matterId"] in candidates
        assert candidates == sorted(candidates)
        assert {matters[m].client_id for m in candidates} == {label["clientId"]}
        assert all(matters[m].status == "open" for m in candidates)
        assert label["expectedOutcome"] == "review"


def test_a_closed_matter_never_expects_an_automatic_post(corpus):
    matters = {matter.matter_id: matter for matter in corpus.firm.matters}
    closed = [
        label
        for label in corpus.labels
        if label["matterId"] and matters[label["matterId"]].status == "closed"
    ]
    assert closed, "the corpus should contain at least one closed-matter signal"
    assert all(label["expectedOutcome"] == "review" for label in closed)


def test_unassigned_signals_are_stored_but_not_billable(corpus):
    unassigned = [
        label
        for label in corpus.labels
        if label["expectedStorage"] == "stored" and label["clientId"] is None
    ]
    assert unassigned, "the corpus should contain internal or newsletter signals"
    assert all(label["billable"] is False for label in unassigned)
    assert all(label["expectedOutcome"] == "not_billable" for label in unassigned)


def test_every_billable_event_has_a_matter(corpus):
    for label in corpus.labels:
        if label["billable"]:
            assert label["matterId"] is not None
            assert label["clientId"] is not None


def test_content_capture_is_off_everywhere(corpus):
    for event in corpus.events:
        assert event["privacy"] == {"contentCaptured": False, "redactionApplied": False}
        assert "contentRef" not in event


def test_every_event_has_a_positive_interval_and_capture_follows_it(corpus):
    for event in corpus.events:
        started = parse_time(event["startedAt"])
        ended = parse_time(event["endedAt"])
        captured = parse_time(event["capturedAt"])
        assert started < ended <= captured, event["id"]
        assert (ended - started).total_seconds() % 60 == 0, event["id"]


def test_candidate_sets_are_honest(corpus):
    for label in corpus.labels:
        candidates = label["candidateMatterIds"]
        if len(candidates) > 1:
            assert label["ambiguous"], label["eventId"]
            assert label["expectedOutcome"] == "review", label["eventId"]
        if label["ambiguous"]:
            assert len(candidates) > 1, label["eventId"]
        if label["matterId"] is not None:
            assert label["matterId"] in candidates or not candidates, label["eventId"]


def test_client_only_signals_list_every_open_matter(corpus):
    matters = {matter.matter_id: matter for matter in corpus.firm.matters}
    for label in corpus.labels:
        matter_id = label["matterId"]
        if (
            matter_id
            and matters[matter_id].status == "open"
            and label["evidence"] in {"counterparty", "content"}
        ):
            expected = sorted(
                matter.matter_id
                for matter in corpus.firm.matters
                if matter.client_id == label["clientId"] and matter.status == "open"
            )
            assert label["candidateMatterIds"] == expected, label["eventId"]


def test_every_address_uses_a_reserved_test_domain(corpus):
    addresses = set()
    for event in corpus.events:
        for participant in event.get("subject", {}).get("participants", []):
            addresses.add(participant["address"])
    for thread in corpus.threads:
        for message in thread["messages"]:
            addresses.add(message["from"])
            addresses.update(message["to"])
            addresses.update(message.get("cc", []))
    for meeting in corpus.calendar:
        addresses.add(meeting["organizer"])
        addresses.update(meeting["attendees"])
    for user in corpus.firm.users:
        addresses.add(user.email)
    assert addresses
    for address in addresses:
        assert address.endswith((".example", ".test")), address


def test_events_are_ordered_by_start_then_id(corpus):
    keys = [(event["startedAt"], event["id"]) for event in corpus.events]
    assert keys == sorted(keys)


def test_the_corpus_is_large_enough_to_measure_something(corpus):
    assert len(corpus.firm.users) >= 4
    assert len(corpus.firm.clients) >= 6
    assert len(corpus.firm.matters) >= 18
    assert len(corpus.threads) >= 10
    assert len(corpus.calendar) >= 10
    assert len(corpus.events) >= 150


def test_every_matter_and_client_appears_in_the_events(corpus):
    labelled_matters = {label["matterId"] for label in corpus.labels if label["matterId"]}
    assert {matter.matter_id for matter in corpus.firm.matters} <= labelled_matters
    per_client = Counter(label["clientId"] for label in corpus.labels if label["clientId"])
    assert set(per_client) == {client.client_id for client in corpus.firm.clients}
    assert min(per_client.values()) >= 3


def test_the_corpus_carries_no_credential_shaped_text(corpus):
    blob = b"".join(canonical_bytes(body) for body in corpus.files().values())
    for forbidden in (b"tlx_", b"sk-ant-", b"BEGIN ", b"PRIVATE KEY"):
        assert forbidden not in blob
