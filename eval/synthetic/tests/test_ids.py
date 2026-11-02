# Written by Kone Tshivhinda & Claude
from datetime import UTC, datetime, timedelta, timezone

import pytest

from timelex_goldset import activity_event_id


def test_the_canonical_id_matches_the_pinned_cross_language_value():
    # Pinned in timelex-types as well. If Python and Rust disagree, the corpus stops being a
    # rehearsal for the real capture path.
    assert (
        activity_event_id(
            "tn_demo0001",
            "m365_mail",
            "AAMkAGI2synthetic-message-id",
            datetime(2026, 10, 20, 8, 12, tzinfo=UTC),
        )
        == "ae_819918eaf3953eb60573f889187a7b92"
    )


def test_the_length_prefix_stops_a_boundary_shift():
    at = datetime(2026, 10, 20, 8, 12, tzinfo=UTC)
    left = activity_event_id("ab", "m365_mail", "c", at)
    right = activity_event_id("a", "m365_mail", "bc", at)
    assert left != right


def test_sub_seconds_and_offsets_name_the_same_instant():
    base = datetime(2026, 10, 20, 8, 12, tzinfo=UTC)
    with_micros = base.replace(microsecond=999_999)
    same_instant = datetime(2026, 10, 20, 10, 12, tzinfo=timezone(timedelta(hours=2)))
    assert activity_event_id("tn_x", "m365_mail", "m", with_micros) == activity_event_id(
        "tn_x", "m365_mail", "m", base
    )
    assert activity_event_id("tn_x", "m365_mail", "m", same_instant) == activity_event_id(
        "tn_x", "m365_mail", "m", base
    )


def test_a_naive_datetime_is_refused():
    with pytest.raises(ValueError, match="timezone-aware"):
        activity_event_id("tn_x", "m365_mail", "m", datetime(2026, 10, 20, 8, 12))
