# Written by Kone Tshivhinda & Claude
from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from .emit import check_corpus, write_corpus
from .generate import DEFAULT_SEED, generate


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="timelex-goldset")
    commands = parser.add_subparsers(dest="command", required=True)

    build = commands.add_parser("generate", help="write the corpus for a seed")
    build.add_argument("--seed", type=int, default=DEFAULT_SEED)
    build.add_argument("--out", type=Path, default=Path("corpus"))

    check = commands.add_parser("check", help="fail if the corpus no longer matches")
    check.add_argument("--corpus", type=Path, default=Path("corpus"))

    args = parser.parse_args(argv)
    if args.command == "generate":
        corpus = generate(args.seed)
        write_corpus(corpus, args.out)
        print(f"wrote {len(corpus.events)} events to {args.out}")
        return 0

    problems = check_corpus(args.corpus)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    print("corpus is current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
