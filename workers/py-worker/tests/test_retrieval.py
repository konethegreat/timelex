# Written by Kone Tshivhinda & Claude
"""Per-tenant retrieval: deterministic local embeddings, vector-only persistence, isolation."""

from __future__ import annotations

import json
import math
from pathlib import Path

import pytest

from timelex_worker.retrieval import (
    DEFAULT_TOP_K,
    EMBEDDING_NAME,
    DirectoryTenantStore,
    InMemoryTenantStore,
    Match,
    RetrievalError,
    RetrievalIndex,
    embed,
    handle_request,
)

TENANT = "tn_examplefirm"
OTHER = "tn_otherfirm"
REPO_ROOT = Path(__file__).resolve().parents[3]
EXAMPLES = REPO_ROOT / "contracts" / "examples"


def index_request(documents: dict[str, str], tenant: str = TENANT) -> dict:
    return {
        "schemaVersion": "1.0",
        "tenantId": tenant,
        "operation": "index",
        "documents": [
            {"documentId": document_id, "text": text} for document_id, text in documents.items()
        ],
    }


def search_request(query: str, tenant: str = TENANT, top_k: int | None = None) -> dict:
    request = {
        "schemaVersion": "1.0",
        "tenantId": tenant,
        "operation": "search",
        "query": query,
    }
    if top_k is not None:
        request["topK"] = top_k
    return request


# Embedding


def test_embedding_is_deterministic():
    assert embed("Lease agreement termination") == embed("Lease agreement termination")


def test_embedding_is_case_insensitive():
    assert embed("Lease Agreement") == embed("lease   agreement")


def test_embedding_is_l2_normalised():
    vector = embed("lease agreement termination notice")
    assert vector
    assert math.isclose(
        math.sqrt(sum(value * value for value in vector.values())), 1.0, abs_tol=1e-12
    )


def test_empty_text_embeds_to_zero_vector():
    assert embed("  ,;:  ") == {}


def test_similar_text_scores_higher_than_unrelated_text(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(
        TENANT,
        {
            "mt_lease": "lease agreement termination notice",
            "mt_tax": "income tax return and invoice",
        },
    )
    matches = index.search(TENANT, "lease dispute")
    assert matches[0].document_id == "mt_lease"
    assert matches[0].score > 0.0
    assert all(match.document_id != "mt_tax" for match in matches)


def test_identical_text_scores_one(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {"mt_1": "lease agreement termination"})
    matches = index.search(TENANT, "lease agreement termination")
    assert [match.document_id for match in matches] == ["mt_1"]
    assert matches[0].score == 1.0


# Search behaviour


def test_search_returns_best_first(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(
        TENANT,
        {
            "mt_all": "lease agreement termination notice",
            "mt_some": "lease agreement",
            "mt_other": "tax invoice",
        },
    )
    matches = index.search(TENANT, "lease agreement termination")
    assert [match.document_id for match in matches] == ["mt_all", "mt_some"]
    assert matches[0].score >= matches[1].score


def test_ties_break_by_document_id(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {"mt_b": "lease", "mt_a": "lease"})
    matches = index.search(TENANT, "lease")
    assert [match.document_id for match in matches] == ["mt_a", "mt_b"]


def test_search_in_unknown_tenant_is_empty(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    assert index.search(TENANT, "lease agreement") == []


def test_search_respects_top_k_and_default(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {f"mt_{position}": "lease agreement" for position in range(8)})
    assert len(index.search(TENANT, "lease agreement", top_k=2)) == 2
    assert len(index.search(TENANT, "lease agreement")) == DEFAULT_TOP_K


@pytest.mark.parametrize("top_k", [0, -1, 51, True, 2.5])
def test_search_rejects_invalid_top_k(tmp_path, top_k):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    with pytest.raises(RetrievalError):
        index.search(TENANT, "lease", top_k=top_k)


def test_only_positive_similarity_is_reported(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {"mt_1": "employment contract", "mt_2": "property transfer"})
    matches = index.search(TENANT, "employment contract")
    assert all(match.score > 0.0 for match in matches)


def test_index_upserts_by_document_id(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    assert index.index(TENANT, {"mt_1": "lease agreement"}) == 1
    assert index.index(TENANT, {"mt_1": "income tax return"}) == 1
    assert index.search(TENANT, "income tax return")[0].score == 1.0
    assert index.search(TENANT, "lease agreement") == []


@pytest.mark.parametrize("document_id", ["", "../escape", "a" * 65, "space id", "slash/id"])
def test_index_rejects_bad_document_ids(tmp_path, document_id):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    with pytest.raises(RetrievalError):
        index.index(TENANT, {document_id: "lease agreement"})


# Namespace isolation


def test_same_document_id_in_two_tenants_does_not_mix(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {"mt_1": "lease agreement termination"})
    index.index(OTHER, {"mt_1": "income tax return"})

    tenant_matches = index.search(TENANT, "lease agreement termination")
    other_matches = index.search(OTHER, "income tax return")
    assert tenant_matches == [Match(document_id="mt_1", score=1.0)]
    assert other_matches == [Match(document_id="mt_1", score=1.0)]


def test_each_tenant_has_its_own_file(tmp_path):
    store = DirectoryTenantStore(tmp_path)
    index = RetrievalIndex(store)
    index.index(TENANT, {"mt_1": "lease agreement"})
    index.index(OTHER, {"mt_1": "income tax return"})
    files = sorted(path.relative_to(tmp_path).as_posix() for path in tmp_path.rglob("index.json"))
    assert files == [f"{TENANT}/index.json", f"{OTHER}/index.json"]


def test_persisted_namespace_holds_vectors_not_text(tmp_path):
    index = RetrievalIndex(DirectoryTenantStore(tmp_path))
    index.index(TENANT, {"mt_1": "settlement agreement for the tenant dispute"})
    raw = (tmp_path / TENANT / "index.json").read_text(encoding="utf-8")
    stored = json.loads(raw)
    assert "settlement" not in raw
    assert stored["tenantId"] == TENANT
    assert stored["embedding"] == EMBEDDING_NAME
    assert sorted(stored["documents"]) == ["mt_1"]
    assert all(
        len(bucket) == 16 and set(bucket) <= set("0123456789abcdef")
        for bucket in stored["documents"]["mt_1"]
    )


def test_reload_keeps_tenant_data(tmp_path):
    RetrievalIndex(DirectoryTenantStore(tmp_path)).index(TENANT, {"mt_1": "lease agreement"})
    reloaded = RetrievalIndex(DirectoryTenantStore(tmp_path))
    assert reloaded.search(TENANT, "lease agreement")[0].score == 1.0
    assert reloaded.search(OTHER, "lease agreement") == []


def test_store_refuses_a_file_copied_from_another_tenant(tmp_path):
    store = DirectoryTenantStore(tmp_path)
    RetrievalIndex(store).index(TENANT, {"mt_1": "lease agreement"})
    destination = tmp_path / OTHER / "index.json"
    destination.parent.mkdir(parents=True)
    destination.write_text(
        (tmp_path / TENANT / "index.json").read_text(encoding="utf-8"), encoding="utf-8"
    )
    with pytest.raises(RetrievalError, match="namespace"):
        store.read(OTHER)


def test_store_refuses_vectors_from_another_embedding(tmp_path):
    store = DirectoryTenantStore(tmp_path)
    RetrievalIndex(store).index(TENANT, {"mt_1": "lease agreement"})
    path = tmp_path / TENANT / "index.json"
    stored = json.loads(path.read_text(encoding="utf-8"))
    stored["embedding"] = "some-other-model-v9"
    path.write_text(json.dumps(stored), encoding="utf-8")
    with pytest.raises(RetrievalError, match="embedding"):
        store.read(TENANT)


@pytest.mark.parametrize(
    "tenant", ["", "firm", "tn_short", "../escape", "tn_../escape", "/tmp/escape", "tn_abc/def"]
)
def test_store_rejects_bad_tenant_ids(tmp_path, tenant):
    root = tmp_path / "root"
    store = DirectoryTenantStore(root)
    with pytest.raises(RetrievalError):
        store.read(tenant)
    with pytest.raises(RetrievalError):
        store.write(tenant, {})
    assert not root.exists() or not any(root.rglob("*.json"))
    assert not (tmp_path / "escape").exists()


def test_tenant_id_cannot_escape_the_root(tmp_path):
    root = tmp_path / "root"
    index = RetrievalIndex(DirectoryTenantStore(root))
    with pytest.raises(RetrievalError):
        index.index("../../outside", {"mt_1": "lease agreement"})
    assert not (tmp_path / "outside").exists()


# Contract handling


def test_handle_index_request_returns_contract_shape():
    index = RetrievalIndex(InMemoryTenantStore())
    request = index_request({"mt_1": "lease agreement", "mt_2": "tax invoice"})
    assert handle_request(request, index) == {
        "schemaVersion": "1.0",
        "tenantId": TENANT,
        "operation": "index",
        "indexed": 2,
    }


def test_handle_search_request_returns_contract_shape():
    index = RetrievalIndex(InMemoryTenantStore())
    handle_request(index_request({"mt_1": "lease agreement"}), index)
    response = handle_request(search_request("lease agreement", top_k=3), index)
    assert response == {
        "schemaVersion": "1.0",
        "tenantId": TENANT,
        "operation": "search",
        "matches": [{"documentId": "mt_1", "score": 1.0}],
    }


@pytest.mark.parametrize(
    "payload",
    [
        {},
        {"schemaVersion": "2.0", "tenantId": TENANT, "operation": "search", "query": "lease"},
        {"schemaVersion": "1.0", "tenantId": "", "operation": "search", "query": "lease"},
        {"schemaVersion": "1.0", "tenantId": TENANT, "operation": "delete"},
        {"schemaVersion": "1.0", "tenantId": TENANT, "operation": "index"},
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "index",
            "documents": [],
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "index",
            "documents": [{"documentId": "mt_1", "text": "lease"}],
            "query": "lease",
        },
        {"schemaVersion": "1.0", "tenantId": TENANT, "operation": "search"},
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "search",
            "query": "lease",
            "documents": [{"documentId": "mt_1", "text": "lease"}],
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "search",
            "query": "lease",
            "topK": 51,
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "search",
            "query": "lease",
            "office": "johannesburg",
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "index",
            "documents": [{"documentId": "mt_1", "text": ""}],
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "index",
            "documents": [{"documentId": "mt_1", "text": "lease", "tenantId": OTHER}],
        },
        {
            "schemaVersion": "1.0",
            "tenantId": TENANT,
            "operation": "search",
            "query": "lease" * 1000,
        },
    ],
)
def test_handle_request_rejects_invalid_payloads(payload):
    index = RetrievalIndex(InMemoryTenantStore())
    with pytest.raises(RetrievalError):
        handle_request(payload, index)


# Frozen examples


def test_contract_examples_replay_index():
    request = json.loads((EXAMPLES / "retrieval-index-request.v1.0.json").read_text("utf-8"))
    expected = json.loads((EXAMPLES / "retrieval-index-response.v1.0.json").read_text("utf-8"))
    index = RetrievalIndex(InMemoryTenantStore())
    assert handle_request(request, index) == expected


def test_contract_examples_replay_search():
    seed = json.loads((EXAMPLES / "retrieval-index-request.v1.0.json").read_text("utf-8"))
    request = json.loads((EXAMPLES / "retrieval-search-request.v1.0.json").read_text("utf-8"))
    expected = json.loads((EXAMPLES / "retrieval-search-response.v1.0.json").read_text("utf-8"))
    index = RetrievalIndex(InMemoryTenantStore())
    handle_request(seed, index)
    assert handle_request(request, index) == expected
