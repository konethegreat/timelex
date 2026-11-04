# Written by Kone Tshivhinda & Claude
"""Retrieval stage for attribution: local embeddings in a namespace per tenant.

Design rules:
- One namespace per tenant ([ADR-0007](../../../docs/adr/0007-tenancy-and-data-residency.md)). A
  request names its tenant; documents never carry their own tenant, so text cannot drift into
  another tenant's index.
- Embeddings are deterministic local feature hashes. No provider, no network, no model weights, so
  the same corpus produces the same vectors everywhere and no client text leaves the worker.
- The store keeps vectors only. The text is used once to compute a vector and is never persisted.
- This is lexical retrieval, not a semantic model: it finds shared wording, not paraphrases. That
  is enough to shortlist candidates for later stages, which still need a confidence policy.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

SCHEMA_VERSION = "1.0"
EMBEDDING_NAME = "blake2b-token-hash-v1"
DEFAULT_TOP_K = 5
MAX_TOP_K = 50
MAX_DOCUMENTS = 5000
MAX_TEXT_LENGTH = 4000

_TENANT_RE = re.compile(r"tn_[0-9A-Za-z]{8,64}")
_DOCUMENT_RE = re.compile(r"[0-9A-Za-z._:-]{1,64}")
_TOKEN_RE = re.compile(r"[^\W_]+", re.UNICODE)
_BUCKET_RE = re.compile(r"[0-9a-f]{16}")

_REQUEST_KEYS = {"schemaVersion", "tenantId", "operation", "documents", "query", "topK"}
_DOCUMENT_KEYS = {"documentId", "text"}

# A sparse unit vector: token-hash bucket -> normalised weight. Only buckets that occur are stored.
Vector = dict[str, float]


class RetrievalError(ValueError):
    """A request or namespace file does not satisfy the retrieval contract."""


@dataclass(frozen=True)
class Match:
    """One document from the tenant's namespace and its cosine similarity, rounded to 6 decimals."""

    document_id: str
    score: float


def embed(text: str) -> Vector:
    """Embed `text` as a sparse unit vector of blake2b token hashes.

    Words are case-folded and hashed to 64-bit buckets. Because the bucket is the full hash, two
    different words practically never share one, so a match means the same word: there is no
    collision noise to inflate an unrelated document. The vector is scaled to unit length, which
    makes cosine similarity a dot product over the buckets the two vectors share.
    """
    counts: dict[str, float] = {}
    for token in _TOKEN_RE.findall(text.casefold()):
        digest = hashlib.blake2b(token.encode("utf-8"), digest_size=8, person=b"timelex1")
        bucket = digest.hexdigest()
        counts[bucket] = counts.get(bucket, 0.0) + 1.0
    norm = math.sqrt(sum(weight * weight for weight in counts.values()))
    if norm == 0.0:
        return {}
    return {bucket: weight / norm for bucket, weight in sorted(counts.items())}


class TenantStore(Protocol):
    """Persistence for namespaces. Each tenant is read and written independently."""

    def read(self, tenant_id: str) -> dict[str, Vector]: ...

    def write(self, tenant_id: str, vectors: Mapping[str, Vector]) -> None: ...


class InMemoryTenantStore:
    """Volatile namespaces, for tests and for a worker that has no disk."""

    def __init__(self) -> None:
        self._namespaces: dict[str, dict[str, Vector]] = {}

    def read(self, tenant_id: str) -> dict[str, Vector]:
        validate_tenant_id(tenant_id)
        return dict(self._namespaces.get(tenant_id, {}))

    def write(self, tenant_id: str, vectors: Mapping[str, Vector]) -> None:
        validate_tenant_id(tenant_id)
        self._namespaces[tenant_id] = {
            document_id: dict(vector) for document_id, vector in vectors.items()
        }


class DirectoryTenantStore:
    """One ``<root>/<tenant>/index.json`` file per namespace.

    The tenant directory comes from the validated tenant id, never from caller text, so a request
    cannot traverse out of the root. A file whose own ``tenantId`` does not match its directory is
    refused rather than read, which catches a namespace copied into the wrong place.
    """

    def __init__(self, root: str | Path) -> None:
        self.root = Path(root)

    def _path(self, tenant_id: str) -> Path:
        validate_tenant_id(tenant_id)
        return self.root / tenant_id / "index.json"

    def read(self, tenant_id: str) -> dict[str, Vector]:
        path = self._path(tenant_id)
        if not path.is_file():
            return {}
        try:
            stored = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise RetrievalError("namespace file is unreadable") from error
        return _stored_vectors(stored, tenant_id)

    def write(self, tenant_id: str, vectors: Mapping[str, Vector]) -> None:
        path = self._path(tenant_id)
        for document_id, vector in vectors.items():
            validate_document_id(document_id)
            _validate_vector(vector)
        stored = {
            "schemaVersion": SCHEMA_VERSION,
            "tenantId": tenant_id,
            "embedding": EMBEDDING_NAME,
            "documents": {
                document_id: {
                    bucket: round(weight, 12) for bucket, weight in sorted(vector.items())
                }
                for document_id, vector in sorted(vectors.items())
            },
        }
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(".json.tmp")
        try:
            temporary.write_text(
                json.dumps(stored, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            os.replace(temporary, path)
        except OSError as error:
            temporary.unlink(missing_ok=True)
            raise RetrievalError("namespace file is not writable") from error


class RetrievalIndex:
    """Attribute text against the caller's own tenant namespace."""

    def __init__(self, store: TenantStore) -> None:
        self._store = store

    def index(self, tenant_id: str, documents: Mapping[str, str]) -> int:
        """Add or replace documents and return the namespace's distinct document count."""
        validate_tenant_id(tenant_id)
        if not isinstance(documents, Mapping):
            raise RetrievalError("documents must be a mapping of id to text")
        vectors = dict(self._store.read(tenant_id))
        for document_id, text in documents.items():
            validate_document_id(document_id)
            validate_text(text)
            vectors[document_id] = embed(text)
        self._store.write(tenant_id, vectors)
        return len(vectors)

    def search(self, tenant_id: str, query: str, top_k: int = DEFAULT_TOP_K) -> list[Match]:
        """Return up to `top_k` documents from this tenant only, best first."""
        validate_tenant_id(tenant_id)
        validate_text(query)
        validate_top_k(top_k)
        query_vector = embed(query)
        if not query_vector:
            return []
        scored: list[tuple[float, str]] = []
        for document_id, vector in self._store.read(tenant_id).items():
            score = sum(weight * vector.get(bucket, 0.0) for bucket, weight in query_vector.items())
            if score > 0.0:
                scored.append((min(1.0, score), document_id))
        scored.sort(key=lambda entry: (-entry[0], entry[1]))
        return [
            Match(document_id=document_id, score=round(score, 6))
            for score, document_id in scored[:top_k]
        ]


def handle_request(payload: object, index: RetrievalIndex) -> dict:
    """Validate one retrieval request and return its response.

    The worker is stateless: every call names its tenant and carries its whole payload. The shape
    and limits mirror ``contracts/attribution/retrieval-*.schema.json``.
    """
    if not isinstance(payload, dict):
        raise RetrievalError("request must be an object")
    if set(payload) - _REQUEST_KEYS:
        raise RetrievalError("request has unknown fields")
    if payload.get("schemaVersion") != SCHEMA_VERSION:
        raise RetrievalError("unsupported schemaVersion")
    tenant_id = payload.get("tenantId")
    validate_tenant_id(tenant_id)
    operation = payload.get("operation")

    if operation == "index":
        if "query" in payload or "topK" in payload:
            raise RetrievalError("an index request cannot carry a query")
        documents = _request_documents(payload.get("documents"))
        indexed = index.index(tenant_id, documents)
        return {
            "schemaVersion": SCHEMA_VERSION,
            "tenantId": tenant_id,
            "operation": "index",
            "indexed": indexed,
        }

    if operation == "search":
        if "documents" in payload:
            raise RetrievalError("a search request cannot carry documents")
        query = payload.get("query")
        validate_text(query)
        top_k = payload.get("topK", DEFAULT_TOP_K)
        validate_top_k(top_k)
        matches = index.search(tenant_id, query, top_k)
        return {
            "schemaVersion": SCHEMA_VERSION,
            "tenantId": tenant_id,
            "operation": "search",
            "matches": [
                {"documentId": match.document_id, "score": match.score} for match in matches
            ],
        }

    raise RetrievalError("operation must be index or search")


def validate_tenant_id(tenant_id: object) -> None:
    if not isinstance(tenant_id, str) or _TENANT_RE.fullmatch(tenant_id) is None:
        raise RetrievalError("tenantId must match ^tn_[0-9A-Za-z]{8,64}$")


def validate_document_id(document_id: object) -> None:
    if not isinstance(document_id, str) or _DOCUMENT_RE.fullmatch(document_id) is None:
        raise RetrievalError("documentId must match ^[0-9A-Za-z._:-]{1,64}$")


def validate_text(text: object) -> None:
    if not isinstance(text, str) or not 1 <= len(text) <= MAX_TEXT_LENGTH:
        raise RetrievalError(f"text must be a string of 1 to {MAX_TEXT_LENGTH} characters")


def validate_top_k(top_k: object) -> None:
    if type(top_k) is not int or not 1 <= top_k <= MAX_TOP_K:
        raise RetrievalError(f"topK must be an integer from 1 to {MAX_TOP_K}")


def _request_documents(documents: object) -> dict[str, str]:
    if not isinstance(documents, list) or not 1 <= len(documents) <= MAX_DOCUMENTS:
        raise RetrievalError(f"documents must hold 1 to {MAX_DOCUMENTS} items")
    by_id: dict[str, str] = {}
    for document in documents:
        if not isinstance(document, dict) or set(document) != _DOCUMENT_KEYS:
            raise RetrievalError("each document must hold exactly documentId and text")
        document_id = document["documentId"]
        text = document["text"]
        validate_document_id(document_id)
        validate_text(text)
        by_id[document_id] = text
    return by_id


def _stored_vectors(stored: object, tenant_id: str) -> dict[str, Vector]:
    if not isinstance(stored, dict):
        raise RetrievalError("namespace file is not an object")
    if stored.get("schemaVersion") != SCHEMA_VERSION:
        raise RetrievalError("namespace file has an unsupported schemaVersion")
    if stored.get("tenantId") != tenant_id:
        raise RetrievalError("namespace file belongs to another tenant")
    if stored.get("embedding") != EMBEDDING_NAME:
        raise RetrievalError("namespace file has an unknown embedding")
    documents = stored.get("documents")
    if not isinstance(documents, dict):
        raise RetrievalError("namespace file has no documents")
    vectors: dict[str, Vector] = {}
    for document_id, value in documents.items():
        validate_document_id(document_id)
        vectors[document_id] = _vector_from_json(value)
    return vectors


def _vector_from_json(value: object) -> Vector:
    if not isinstance(value, dict):
        raise RetrievalError("namespace file holds an invalid vector")
    vector: Vector = {}
    for bucket, weight in value.items():
        if not isinstance(bucket, str) or _BUCKET_RE.fullmatch(bucket) is None:
            raise RetrievalError("namespace file holds an invalid bucket")
        if isinstance(weight, bool) or not isinstance(weight, (int, float)):
            raise RetrievalError("namespace file holds a non-numeric weight")
        vector[bucket] = float(weight)
    _validate_vector(vector)
    return vector


def _validate_vector(vector: Mapping[str, float]) -> None:
    for bucket, weight in vector.items():
        if _BUCKET_RE.fullmatch(bucket) is None:
            raise RetrievalError("vector holds an invalid bucket")
        if not math.isfinite(weight) or weight <= 0.0:
            raise RetrievalError("vector holds an invalid weight")
