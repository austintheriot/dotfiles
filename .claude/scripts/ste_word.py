#!/usr/bin/env python3
"""Query the ASD-STE100 not-approved word list.

Lets an agent check specific words without loading the 1,198-entry table into
context. Data lives in ~/.claude/data/ste100-words.json.

Usage:
    ste_word.py utilize ensure prior          # look up words
    ste_word.py --scan draft.md               # every not-approved word in a file
    ste_word.py --scan draft.md --include-exempt
    ste_word.py --json utilize                # machine-readable

Exit status: 0 if every queried word is approved or absent, 1 if any is not
approved, 2 on usage error.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DATA = Path.home() / ".claude" / "data" / "ste100-words.json"

KIND_HINT = {
    "swap": "drop-in replacement",
    "rewrite": "needs a construction change",
    "technical-term": "restructure to use it as a technical noun/verb",
}


def _load() -> dict:
    if not DATA.is_file():
        print(f"ste_word: missing data file: {DATA}", file=sys.stderr)
        sys.exit(2)
    return json.loads(DATA.read_text())["words"]


def _describe(word: str, entry: dict) -> str:
    flag = "  [domain-exempt: approved in software context]" if entry["domain_exempt"] else ""
    pos = f" ({entry['pos']})" if entry["pos"] else ""
    return f"{word} -> {entry['use']}{pos}  [{KIND_HINT[entry['kind']]}]{flag}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Query STE not-approved words.")
    parser.add_argument("words", nargs="*", help="words to look up")
    parser.add_argument("--scan", metavar="FILE", help="report every not-approved word in FILE")
    parser.add_argument("--include-exempt", action="store_true",
                        help="include words that software usage protects")
    parser.add_argument("--json", action="store_true", dest="as_json")
    args = parser.parse_args(argv)

    if not args.words and not args.scan:
        parser.error("give at least one word or --scan FILE")

    table = _load()
    results: dict[str, dict] = {}

    if args.scan:
        path = Path(args.scan)
        if not path.is_file():
            print(f"ste_word: no such file: {args.scan}", file=sys.stderr)
            return 2
        text = path.read_text(errors="replace").lower()
        text = re.sub(r"```.*?```|`[^`\n]+`", " ", text, flags=re.S)
        seen = set(re.findall(r"[a-z][a-z'-]+", text))
        for word, entry in table.items():
            if " " in word:
                if word in text:
                    results[word] = entry
            elif word in seen:
                results[word] = entry

    for word in args.words:
        key = word.lower()
        if key in table:
            results[key] = table[key]
        elif not args.as_json:
            print(f"{word}: not in the not-approved list "
                  "(either approved, or outside the dictionary)")

    if not args.include_exempt:
        results = {w: e for w, e in results.items() if not e["domain_exempt"]}

    if args.as_json:
        print(json.dumps(results, indent=2))
    elif results:
        for word in sorted(results):
            print(_describe(word, results[word]))
        swaps = sum(1 for e in results.values() if e["kind"] == "swap")
        print(f"\n{len(results)} not approved ({swaps} simple swaps)")
    elif args.scan:
        print(f"{args.scan}: no not-approved words found")

    return 1 if results else 0


if __name__ == "__main__":
    sys.exit(main())
