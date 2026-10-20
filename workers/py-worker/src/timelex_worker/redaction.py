# Written by Kone Tshivhinda & Claude
"""Redact personal identifiers before text leaves the platform (POPIA data minimisation).

Design rules:
- Detect on the original text, resolve overlaps once, then rebuild the output, so offsets never
  drift and one token never swallows another.
- Return **counts only**, never the matched values, so results are safe to log.
- Over-redaction is the safe failure; under-redaction is the bug.

This is a minimisation layer, not anonymisation. See README "Limits".
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

# Priority: lower number wins when spans overlap.
_PRIORITY = {"sa_id": 0, "card": 1, "account": 2, "email": 3, "phone": 4}
_TOKENS = {
    "sa_id": "[SA_ID]",
    "card": "[CARD]",
    "account": "[ACCOUNT]",
    "email": "[EMAIL]",
    "phone": "[PHONE]",
}


@dataclass(frozen=True)
class RedactionResult:
    """Redacted text plus per-kind counts (never the matched values)."""

    text: str
    counts: dict[str, int] = field(default_factory=dict)


def luhn_valid(digits: str) -> bool:
    """Luhn checksum over a string of digits."""
    if not digits.isdigit():
        return False
    total = 0
    for i, ch in enumerate(reversed(digits)):
        d = int(ch)
        if i % 2 == 1:
            d *= 2
            if d > 9:
                d -= 9
        total += d
    return total % 10 == 0


def _valid_sa_id(candidate: str) -> bool:
    """13 digits, plausible YYMMDD, citizenship digit 0/1, Luhn check."""
    if len(candidate) != 13 or not candidate.isdigit():
        return False
    month, day = int(candidate[2:4]), int(candidate[4:6])
    if not (1 <= month <= 12 and 1 <= day <= 31):
        return False
    if candidate[10] not in "01":
        return False
    return luhn_valid(candidate)


_SA_ID_RE = re.compile(r"(?<!\d)\d{13}(?!\d)")
_CARD_RE = re.compile(r"(?<!\d)(?:\d[ -]?){12,18}\d(?!\d)")
_EMAIL_RE = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+")
_PHONE_RE = re.compile(r"(?<![\w+])(?:\+27|0027|0)[\s-]?\(?0?\)?[1-9](?:[\s-]?\d){8,9}(?!\d)")
_ACCOUNT_RE = re.compile(
    r"(?i)\b(?:account|acc)\.?\s*(?:no|number|nr|num)?\.?\s*[:#]?\s*(\d(?:[\s-]?\d){6,15})(?!\d)"
)

Span = tuple[int, int, str]


def _spans(text: str) -> list[Span]:
    found: list[Span] = []
    for m in _SA_ID_RE.finditer(text):
        if _valid_sa_id(m.group()):
            found.append((m.start(), m.end(), "sa_id"))
    for m in _CARD_RE.finditer(text):
        digits = re.sub(r"[ -]", "", m.group())
        # Major card networks start with 2-6; this keeps 0027... phone numbers out of "card".
        if 13 <= len(digits) <= 19 and digits[0] in "23456" and luhn_valid(digits):
            found.append((m.start(), m.end(), "card"))
    for m in _ACCOUNT_RE.finditer(text):
        found.append((m.start(1), m.end(1), "account"))
    for m in _EMAIL_RE.finditer(text):
        found.append((m.start(), m.end(), "email"))
    for m in _PHONE_RE.finditer(text):
        digits = re.sub(r"\D", "", m.group())
        national = digits[2:] if digits.startswith("27") else digits.lstrip("0")
        if digits.startswith("0027"):
            national = digits[4:]
        # South African numbers have 9 digits after the leading 0 / country code.
        if len(national.lstrip("0")) == 9 or len(national) == 9:
            found.append((m.start(), m.end(), "phone"))
    return found


def _resolve(spans: list[Span]) -> list[Span]:
    """Greedy, non-overlapping: earliest start first, then highest priority, then longest."""
    ordered = sorted(spans, key=lambda s: (s[0], _PRIORITY[s[2]], -(s[1] - s[0])))
    chosen: list[Span] = []
    last_end = -1
    for span in ordered:
        if span[0] >= last_end:
            chosen.append(span)
            last_end = span[1]
    return chosen


def redact(text: str) -> RedactionResult:
    """Replace identifiers with tokens and report how many of each were found."""
    chosen = _resolve(_spans(text))
    out: list[str] = []
    counts: dict[str, int] = {}
    cursor = 0
    for start, end, kind in chosen:
        out.append(text[cursor:start])
        out.append(_TOKENS[kind])
        counts[kind] = counts.get(kind, 0) + 1
        cursor = end
    out.append(text[cursor:])
    return RedactionResult(text="".join(out), counts=counts)
