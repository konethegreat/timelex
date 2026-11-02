# Written by Kone Tshivhinda & Claude
"""The canonical activity-event id, implemented from contracts/events/README.md.

The Rust reference implementation is `timelex_types::activity_event_id`; the pinned cross-language
value in the tests keeps the two from drifting.
"""

from __future__ import annotations

import hashlib
import struct
from datetime import UTC, datetime

_ID_DOMAIN = b"timelex/activity-event/id/v1\0"
_EPOCH = datetime(1970, 1, 1, tzinfo=UTC)


def activity_event_id(tenant_id: str, system: str, source_id: str, started_at: datetime) -> str:
    """Return the `ae_...` id of one observed signal.

    Whole seconds only, by integer arithmetic: `datetime.timestamp()` returns a float and can round
    a sub-second value up, which would disagree with the Rust reference. A naive datetime is refused
    because the host zone would otherwise change the id.
    """
    if started_at.tzinfo is None or started_at.utcoffset() is None:
        raise ValueError("started_at must be timezone-aware")
    delta = started_at.astimezone(UTC) - _EPOCH
    seconds = delta.days * 86_400 + delta.seconds
    digest = hashlib.sha256()
    digest.update(_ID_DOMAIN)
    for part in (tenant_id, system, source_id):
        raw = part.encode("utf-8")
        digest.update(struct.pack(">I", len(raw)))
        digest.update(raw)
    digest.update(struct.pack(">q", seconds))
    return "ae_" + digest.digest()[:16].hex()
