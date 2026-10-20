# timelex-worker (Python)

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Stateless Python worker for jobs where the Python ecosystem wins. **It holds no business rules** ([ADR-0002](../../docs/adr/0002-language-allocation.md)); Rust services own policy, money and state.

| Module | Status | Purpose |
|---|---|---|
| `timelex_worker.redaction` | ✅ | Remove South African ID numbers, phone numbers, emails, payment-card numbers and labelled bank-account numbers **before** any text is sent to a model provider (POPIA minimisation, [research 03](../../docs/research/03-south-africa-compliance.md)) |
| embeddings / retrieval | planned (P3-06) | Per-tenant namespaces for matter attribution |
| calibration | planned (P3-08) | Turn raw confidence into calibrated probabilities |
| OCR | planned (Phase 6) | Scanned documents |

## Use

```bash
uv sync
uv run pytest
uv run ruff check . && uv run ruff format --check .
```

```python
from timelex_worker.redaction import redact

result = redact("Call me on 082 123 4567 or mail a@b.co.za")
print(result.text)  # Call me on [PHONE] or mail [EMAIL]
print(result.counts)  # {'phone': 1, 'email': 1}  (matched values are never returned)
```

## Limits (be honest about them)
- Pattern-based: it will miss free-text names, addresses and unlabelled account numbers. It is a **minimisation layer, not anonymisation**. Names and matter content need the privacy filters and exclusions planned in P2-07.
- Bank-account detection only fires on a label ("account number", "acc no") followed by digits, to avoid redacting every long number.
- Luhn and date checks reduce false positives but do not eliminate them; over-redaction is the safe failure.
