# timelex-worker (Python)

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Stateless Python worker for jobs where the Python ecosystem wins. **It holds no business rules** ([ADR-0002](../../docs/adr/0002-language-allocation.md)); Rust services own policy, money and state.

| Module | Status | Purpose |
|---|---|---|
| `timelex_worker.redaction` | ✅ | Remove South African ID numbers, phone numbers, emails, payment-card numbers and labelled bank-account numbers **before** any text is sent to a model provider (POPIA minimisation, [research 03](../../docs/research/03-south-africa-compliance.md)) |
| `timelex_worker.retrieval` | ✅ | Deterministic local token-hash vectors and cosine search in one namespace per tenant ([ADR-0029](../../docs/adr/0029-retrieval-local-embeddings-per-tenant-namespace.md)); no provider, network or model weights, and the store keeps vectors, never text |
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

```python
from timelex_worker.retrieval import DirectoryTenantStore, RetrievalIndex, handle_request

index = RetrievalIndex(DirectoryTenantStore("worker-data"))
handle_request(
    {
        "schemaVersion": "1.0",
        "tenantId": "tn_examplefirm",
        "operation": "index",
        "documents": [{"documentId": "mt_1", "text": "Lease termination notice"}],
    },
    index,
)
handle_request(
    {
        "schemaVersion": "1.0",
        "tenantId": "tn_examplefirm",
        "operation": "search",
        "query": "lease termination",
        "topK": 3,
    },
    index,
)
# {'schemaVersion': '1.0', 'tenantId': 'tn_examplefirm', 'operation': 'search',
#  'matches': [{'documentId': 'mt_1', 'score': 0.816497}]}
```

Each tenant is stored in `<root>/<tenantId>/index.json`. A request names its tenant, and a namespace
file carrying another tenant's id is refused rather than read.

## Limits (be honest about them)
- Redaction is pattern-based: it will miss free-text names, addresses and unlabelled account numbers. It is a **minimisation layer, not anonymisation**. Names and matter content need the privacy filters and exclusions planned in P2-07.
- Bank-account detection only fires on a label ("account number", "acc no") followed by digits, to avoid redacting every long number.
- Luhn and date checks reduce false positives but do not eliminate them; over-redaction is the safe failure.
- Retrieval is lexical, not semantic: it matches shared words, not paraphrases or synonyms. Token hashes in a namespace are not encryption, and the file is tenant data even though it holds no source text.
