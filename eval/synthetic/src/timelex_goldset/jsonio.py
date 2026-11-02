# Written by Kone Tshivhinda & Claude
"""One canonical JSON spelling, so the corpus can be compared byte for byte."""

from __future__ import annotations

import json
from typing import Any


def canonical_bytes(body: Any) -> bytes:
    """Stable UTF-8 JSON: sorted keys, two-space indent, one trailing LF, no CRLF."""
    text = json.dumps(body, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    return text.encode("utf-8")
