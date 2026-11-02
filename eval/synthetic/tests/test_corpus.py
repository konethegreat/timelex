# Written by Kone Tshivhinda & Claude
import hashlib
import json
from pathlib import Path

from timelex_goldset import check_corpus, generate, write_corpus

SEED = 20261102
CORPUS = Path(__file__).resolve().parents[1] / "corpus"


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def normalized(path: Path) -> bytes:
    return path.read_bytes().replace(b"\r\n", b"\n")


def test_the_committed_corpus_is_exactly_what_the_generator_produces():
    assert check_corpus(CORPUS) == []


def test_the_manifest_digests_and_sizes_match_every_file():
    manifest = read_json(CORPUS / "manifest.json")
    for entry in manifest["files"]:
        body = normalized(CORPUS / entry["name"])
        assert hashlib.sha256(body).hexdigest() == entry["sha256"], entry["name"]
        assert len(body) == entry["bytes"], entry["name"]


def test_the_manifest_counts_match_the_corpus():
    manifest = read_json(CORPUS / "manifest.json")
    counts = manifest["counts"]
    labels = read_json(CORPUS / "labels.json")["labels"]
    assert counts["events"] == len(read_json(CORPUS / "events.json")["events"])
    assert counts["clients"] == len(read_json(CORPUS / "firm.json")["clients"])
    assert counts["matters"] == len(read_json(CORPUS / "firm.json")["matters"])
    assert counts["threads"] == len(read_json(CORPUS / "threads.json")["threads"])
    assert counts["meetings"] == len(read_json(CORPUS / "calendar.json")["meetings"])
    assert counts["excludedEvents"] == sum(
        label["expectedStorage"] == "excluded" for label in labels
    )
    assert counts["ambiguousEvents"] == sum(label["ambiguous"] for label in labels)
    assert counts["reviewEvents"] == sum(label["expectedOutcome"] == "review" for label in labels)


def test_check_reports_a_tampered_file(tmp_path):
    write_corpus(generate(SEED), tmp_path)
    assert check_corpus(tmp_path) == []
    path = tmp_path / "labels.json"
    path.write_bytes(path.read_bytes().replace(b'"billable": true', b'"billable": false', 1))
    problems = check_corpus(tmp_path)
    assert problems
    assert any("labels.json" in problem for problem in problems)


def test_check_reports_a_missing_file(tmp_path):
    write_corpus(generate(SEED), tmp_path)
    (tmp_path / "calendar.json").unlink()
    assert any("calendar.json" in problem for problem in check_corpus(tmp_path))
