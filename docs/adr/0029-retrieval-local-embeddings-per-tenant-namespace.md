# ADR-0029: Retrieval embeds locally, and every tenant gets its own namespace

- **Status:** Accepted
- **Date:** 2026-11-04
- **Task:** P3-06 (retrieval stage: worker embeddings, per-tenant namespace)
- **Relates to:** ADR-0002 (Python for embeddings behind a typed contract), ADR-0007 (tenancy and data residency), ADR-0028 (attribution stages 0–1), [research 04 §4](../research/04-autonomous-capture-design.md)

## Context

Stages 0–1 assign 102 blocks on the synthetic gold set and leave 91 for later stages: 71 reviews
with a client but no single matter, and 20 blocks with no key or history at all. Stage 2 is retrieval:
given a block's text, find the firm's matter records and prior work that read most like it, and hand a
shortlist to the model stages.

The contract has four hard constraints. Client text must not leave the platform without an approved
provider and a disclosure entry ([P2-10](0024-data-flow-register-and-disclosure.md)). One tenant's
text must never influence another tenant's results (ADR-0007). A worker is stateless and may restart
at any time, so the index needs to survive a restart without a broker. And the same corpus must
produce the same scores on every machine, or the gold set cannot measure the stage.

An external embeddings API is the obvious quality answer and the wrong first answer here. It sends
client text to a third party, needs a provider agreement and a key, costs money per call, and hides
its model version behind a moving endpoint. None of that is approved for P3-06.

## Decision

### 1. Embeddings are local, deterministic and lexical

The worker computes a sparse unit vector from the text with blake2b token hashing. Words are
case-folded, split on Unicode word boundaries and hashed to 64-bit buckets; repeated words raise a
bucket's weight, and the vector is scaled to unit length. Cosine similarity is then a dot product over
shared buckets. The full hash is the bucket, so two different words practically never collide and a
match means the same word. There is no model, no provider, no network and no floating-point model
version to pin: the embedding name (`blake2b-token-hash-v1`) is stored beside the vectors, and a
change to the tokenizer or hash needs a new name and a rebuild.

This is lexical retrieval. It finds shared wording and names, not paraphrases or synonyms. That is
honest for a first retrieval stage and enough to shortlist candidates for the model, which must still
decide whether a candidate is good enough to post.

### 2. A namespace belongs to exactly one tenant

A request names its `tenantId`; documents never carry their own tenant. Each tenant is persisted in
its own file, `<root>/<tenantId>/index.json`, inside a directory derived only from a validated tenant
id matching `^tn_[0-9A-Za-z]{8,64}$`. A request cannot traverse out of the root because no caller text
reaches the path. A file whose own `tenantId` does not match the directory it was read from is refused
rather than read, so a namespace copied into the wrong place cannot leak. One tenant's documents are
never loaded while answering another tenant, so there is no shared index to filter and no filter to
get wrong. Tenants are isolated by construction, not by a query parameter.

### 3. The store keeps vectors, never text

The text is used once to compute a vector and is not persisted. The namespace file holds the tenant
id, the embedding name, the schema version and document id → sparse vector. A worker restart reloads
vectors; it never needs the original wording. The namespace file is tenant data and inherits the
tenant's storage protections, but it contains no message body or subject line.

Sparse vectors also keep the file small: one entry per distinct word rather than a fixed-width dense
array. The store writes through a temporary file and an atomic replace, so a crash cannot leave half a
namespace on disk. Two writers to the same tenant would race; the MVP expects one worker per tenant
and revisits this when concurrency exists.

### 4. The contract is versioned and symmetrical

`contracts/attribution/retrieval-request.schema.json` and `retrieval-response.schema.json` define one
`index` operation and one `search` operation at `schemaVersion` 1.0. An index request carries 1–5000
documents and cannot carry a query; a search request carries a query, an optional `topK` of 1–50 and
no documents. Responses echo the tenant and operation, return `indexed` for an index or `matches` for
a search, and never mix the two. A match holds a document id and a cosine score rounded to six
decimals; only positive similarity is reported. Unknown fields are rejected on both sides.

The worker enforces the same limits in `handle_request`, so a caller that skips schema validation
still cannot write a foreign tenant or an oversized document. Frozen request/response examples live in
`contracts/examples/`; the Rust contract test validates them against the schemas and the Python test
replays them, so the two languages cannot drift apart silently.

### 5. The acceptance test is isolation, not recall

The task's acceptance is a namespace isolation test. With the same document id written under two
tenants, each search returns its own vector. Each tenant has its own file. A file copied from another
tenant is refused. A tenant id containing a path separator or `..` is rejected before any write. A
separate test reads the persisted file and asserts the source text does not appear in it. Scores are
deterministic, ties break on document id, and only positive matches are returned.

Retrieval quality on the gold set is not claimed here. The gold set has no retrieval-specific labels,
and stage 2's precision depends on the candidate policy and threshold that P3-07 will own. Measuring
it now would either invent a threshold or report a number with no gate.

## Alternatives considered

- **Call an external embeddings provider.** Rejected for this increment: unapproved data flow,
  provider credentials, per-call cost and a model version the platform cannot pin. Revisit when a
  provider agreement and a disclosure entry exist. The contract does not name the embedding, so a
  later provider can replace the local one behind the same operations.
- **Use a bundled sentence-transformer model.** Rejected: model weights and a runtime are a large
  dependency, and an embedding model's version becomes part of every stored vector. The current
  vectors are reproducible from a hash and rebuildable in seconds.
- **Dense feature hashing into a fixed small dimension.** Rejected after measuring it: collisions let
  unrelated or partly matching documents outrank the document that shared more words. Sparse 64-bit
  token hashes remove that noise; the cost is that the vector is larger in entry count but smaller
  than a dense array of comparable quality.
- **One shared index with a tenant filter on every query.** Rejected by ADR-0007. A filter is a
  policy that can be forgotten, and a bug in it leaks across tenants. Separate namespaces make the
  isolation structural.
- **Store the source text beside the vector.** Rejected: the vector is all retrieval needs, and the
  text is the sensitive part. Not storing it keeps the namespace useful after a restart without
  retaining message bodies.
- **Build the HTTP service and mcp-host workflow route now.** Deferred. The worker contract and index
  are the acceptance-bearing part; routing, tenancy context and workflow wiring belong with the
  service work that also owns persistence and review. ADR-0018 already routes those endpoints to
  P3-06 follow-up, and they will call this contract unchanged.

## Consequences

- **Namespace isolation is structural.** A missed tenant parameter cannot search another tenant's
  namespace because there is no other tenant's namespace in memory for that request.
- **Recall is lexical.** Synonyms, paraphrases and a matter named only in a message body that shares
  no words with the matter record will not match. Those blocks stay for the model stages or a person.
- **Token hashes are not encryption.** A namespace file holds hashes of ordinary words, and a
  dictionary can guess common ones. It contains no text, but it is still tenant data and must be
  protected as such. The vector is a minimisation, not a secret-sharing scheme.
- **Changing the embedding means a rebuild.** The embedding name is stored with the namespace; a
  worker that does not recognize it refuses to read. That is deliberate: silently mixing vectors from
  two schemes would produce meaningless scores.
- **The index is per worker root, not replicated.** Restart survives because the file is on disk;
  machine loss does not. Replication, backups and tenancy-aware storage belong to the service.
- **Scores are not confidence.** A cosine score says two texts share words, not that the document is
  the right matter. It must not be compared to a posting threshold without calibration.
- **Revisit** when a provider and disclosure exist, when privacy filtering of bodies is ready for
  content retrieval, when the gold set gains retrieval labels, and when more than one worker writes a
  tenant.
