# Written by Kone Tshivhinda & Claude
"""Write the corpus to disk and prove a committed copy still matches the generator."""

from __future__ import annotations

import json
from pathlib import Path

from .generate import generate
from .jsonio import canonical_bytes
from .model import Corpus


def write_corpus(corpus: Corpus, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    files = corpus.files()
    for name, body in sorted(files.items()):
        (out / name).write_bytes(canonical_bytes(body))
    (out / "manifest.json").write_bytes(canonical_bytes(corpus.manifest()))


def _normalized(path: Path) -> bytes:
    return path.read_bytes().replace(b"\r\n", b"\n")


def check_corpus(corpus_dir: Path) -> list[str]:
    """Return one problem per drifted file, or an empty list when the corpus is current."""
    manifest_path = corpus_dir / "manifest.json"
    if not manifest_path.exists():
        return ["manifest.json: missing"]
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"manifest.json: cannot read ({error})"]
    seed = manifest.get("seed")
    if not isinstance(seed, int):
        return ["manifest.json: seed is missing or not an integer"]

    corpus = generate(seed)
    expected = {name: canonical_bytes(body) for name, body in corpus.files().items()}
    expected["manifest.json"] = canonical_bytes(corpus.manifest())

    problems: list[str] = []
    for name, body in sorted(expected.items()):
        path = corpus_dir / name
        if not path.exists():
            problems.append(f"{name}: missing")
        elif _normalized(path) != body:
            problems.append(f"{name}: differs from the generator")
    for path in sorted(corpus_dir.glob("*.json")):
        if path.name not in expected:
            problems.append(f"{path.name}: unexpected file")
    return problems
