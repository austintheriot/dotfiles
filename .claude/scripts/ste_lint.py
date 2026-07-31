#!/usr/bin/env python3
"""Deterministic linter for STE-shaped prose.

Checks only what a regex can decide. It cannot detect passive voice, restricted
meanings, or whether a technical noun fits an approved category, so a clean run
does not mean the text is STE-compliant. It means the mechanical violations are
gone. Rule numbers refer to ASD-STE100 Issue 9; see
~/.claude/rules/simplified-technical-english.md.

Usage:
    ste_lint.py FILE...
    ste_lint.py -                      # read stdin
    echo "text" | ste_lint.py - --mode procedural
    ste_lint.py draft.md --json        # machine-readable
    ste_lint.py draft.md --quiet       # exit status only

Modes:
    prose (default)  25-word cap. Docs, chat replies, PR bodies, commit bodies.
                     Banned modals and vague qualifiers are enforced.
    procedural       20-word cap. Steps, runbooks, safety text.
    strict           Same as prose. Kept as an explicit name for agent-facing
                     text, error messages, and API reference.
    voice            25-word cap, modal and hedge checks OFF. The only carve-out
                     mode: teammate-facing drafts where
                     ~/.claude/rules/write-like-austin.md outranks STE, because
                     the social work of the hedges is the point.

Exit status: 0 clean, 1 violations found, 2 usage error.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, asdict
from pathlib import Path

WORD_CAP = {"prose": 25, "procedural": 20, "strict": 25, "voice": 25}

# Rule 3.2 / 3.4. STE approves can, will, must. Everything else in this set
# encodes unmarked uncertainty, which is what STE exists to remove.
BANNED_MODALS = ("should", "would", "may", "might", "could", "ought")

# Vague qualifiers. These smooth over a confidence signal instead of stating it.
# STE bans the mush, not the information: replace with the number, or with a
# plain declarative sentence about what you did not check.
VAGUE_HEDGES = (
    "probably", "roughly", "somewhat", "fairly", "pretty much", "a bit",
    "kind of", "sort of", "more or less", "i'd say", "i would say",
    "arguably", "presumably", "seemingly", "relatively",
)

# Rule 9.4. Rotating synonyms for one concept is a defect even when every
# variant is individually approved. Each set collapses to its first member.
SYNONYM_SETS = (
    ("make sure", "check", "verify", "confirm", "validate", "ensure", "ascertain"),
    ("config", "configuration", "settings", "preferences"),
    ("start", "begin", "commence", "initiate", "kick off"),
    ("stop", "cease", "terminate", "discontinue", "halt"),
    ("do", "perform", "execute", "conduct", "carry out"),
    ("show", "indicate", "display", "reveal", "surface"),
    ("use", "utilize", "employ", "leverage"),
    ("change", "modify", "alter", "adjust", "update"),
    ("remove", "delete", "eliminate", "drop", "strip"),
    ("find", "locate", "determine", "discover", "identify"),
)

# Marketing register. STE has no rule for these; they violate the spirit of
# rule 9.2 (a word must carry a definite meaning) and Austin's voice spec.
SLOP = (
    "simply", "seamlessly", "seamless", "robust", "powerful", "comprehensive",
    "leverage", "leveraging", "pivotal", "crucial", "vital", "cutting-edge",
    "state-of-the-art", "best-in-class", "world-class", "game-changing",
    "effortlessly", "delightful", "elegant", "unlock", "supercharge",
    "excited", "thrilled", "revolutionary", "seamlessly integrate",
)

# Claudese tics named in ~/.claude/rules/write-like-austin.md.
CLAUDESE = (
    "belt-and-suspenders", "a sharp edge", "the load-bearing", "footgun",
    "non-trivial", "first-class", "out of the box", "to be fair", "that said",
    "it's worth noting", "worth noting", "dive in", "deep dive", "let's unpack",
)

# Single-word swaps extracted from the ASD-STE100 Part 2 dictionary, filtered to
# entries that improve any register and that software vocabulary does not claim.
# Not the full 1,198-entry list; see ~/.claude/rules/ste100-word-index.md.
SWAP = {
    "acceptable": "permitted", "achieve": "get", "acquire": "get",
    "additional": "more", "adequate": "sufficient", "advise": "tell",
    "aggravate": "increase", "allow": "let", "alter": "change",
    "appropriate": "applicable", "assist": "help", "attempt": "try",
    "avoid": "prevent", "cease": "stop", "commence": "start",
    "comprise": "have", "conduct": "do", "considerable": "large",
    "determine": "find", "diminish": "decrease", "discontinue": "stop",
    "eliminate": "remove", "employ": "use", "evaluate": "examine",
    "facilitate": "help", "feasible": "possible", "fundamental": "important",
    "however": "but", "identical": "same", "implement": "do",
    "incorporate": "include", "indicate": "show", "inform": "tell",
    "initiate": "start", "locate": "find", "maintain": "keep",
    "major": "primary", "minor": "small", "modify": "change",
    "notify": "tell", "obtain": "get", "perform": "do",
    "permissible": "permitted", "permit": "let", "portion": "piece",
    "principal": "primary", "proceed": "continue", "prohibit": "prevent",
    "provide": "give", "rapid": "fast", "represent": "show",
    "respective": "related", "retain": "keep", "reveal": "show",
    "several": "some", "significant": "important", "similar": "equivalent",
    "suitable": "applicable", "terminate": "stop", "therefore": "thus",
    "undertake": "do", "utilize": "use", "various": "different",
    "via": "through", "whether": "if", "within": "in",
    "prior to": "before", "in order to": "to", "with the exception of": "except",
    "in the event that": "if", "a number of": "some", "at this point in time": "now",
    "for the purpose of": "to", "in spite of": "although",
}

# Construction changes: the fix is a rewrite, not a token swap.
REWRITE = {
    "ascertain": "make sure (that)", "assure": "make sure (that)",
    "ensure": "make sure (that)", "establish": "make sure (that)",
    "verify": "make sure (that)", "complicated": "not easy",
    "difficult": "not easy", "eventually": "some time",
    "exceed": "more than", "excessive": "too much",
    "impossible": "not possible", "insufficient": "not sufficient",
    "unnecessary": "not necessary", "particular": "only applicable",
    "analyze": "do an analysis of", "investigate": "do an investigation of",
    "require": "is necessary", "depend on": "if ...",
}
# Deliberately absent: check, review, test, log, monitor. STE would force
# "do a check of" over "check", which trades fluency for one-part-of-speech
# purity and is banned by ~/.claude/CLAUDE.md.

# Rule 1.5 category 19 and rule 1.12 category 2 approve this vocabulary as
# technical nouns and technical verbs. The aerospace dictionary restricts these
# words to other parts of speech, which would make every software document a
# violation. Never flag them.
DOMAIN_EXEMPT = frozenset("""
code codebase file files test tests function functions log logs loop loops
branch branches build builds load loads store stores process processes design
designs service services route routes trace traces level levels render returns
return pass passes state states event events call calls list lists key keys
focus filter filters field fields content interface interfaces memory network
networks screen screens token tokens update updates install format debug enable
disable open close copy delete drag save sort validate abort boot download
upload upgrade reboot click enter press type tap swipe scroll highlight encrypt
erase paste zoom navigate manage shape match reference case cases result results
work time order use commit commits merge merges deploy deploys rollback cache
caches queue queues thread threads schema schemas index indexes query queries
mock mocks stub stubs hook hooks flag flags patch patches diff diffs
""".split())

FENCE = re.compile(r"```.*?```|~~~.*?~~~", re.S)
INLINE_CODE = re.compile(r"`[^`\n]+`")
URL = re.compile(r"https?://\S+|\b[\w.-]+/[\w./-]+\b")
PATH_LIKE = re.compile(r"[~./]?\b[\w-]+(?:/[\w.-]+)+\b|\b\w+\.(?:py|ts|tsx|js|rs|md|json|toml|yaml|yml|sh)\b")
IDENTIFIER = re.compile(r"\b\w+(?:_\w+)+\b|\b[a-z]+[A-Z]\w*\b|\b[A-Z]{2,}\b")
QUOTED = re.compile(r'"[^"\n]{0,200}"|“[^”\n]{0,200}”')
MD_LINK = re.compile(r"\[([^\]]*)\]\([^)]*\)")
HEADING = re.compile(r"^\s{0,3}#{1,6}\s")
TABLE_ROW = re.compile(r"^\s*\|.*\|\s*$")
LIST_ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s")


@dataclass
class Finding:
    line: int
    rule: str
    severity: str
    message: str
    excerpt: str


def _mask(text: str) -> str:
    """Blank out spans that STE word counts and rewrites must not touch.

    Code fences, inline code, URLs, paths, identifiers, and quoted strings are
    exempt. Rule 8.6 already counts quoted text and alphanumeric identifiers as
    one word; the rest is a necessary extension for software prose.
    """
    for pattern in (FENCE, INLINE_CODE, URL, PATH_LIKE, IDENTIFIER, QUOTED):
        text = pattern.sub(lambda m: " " * len(m.group(0)), text)
    return MD_LINK.sub(lambda m: m.group(1), text)


def _prose_lines(text: str) -> list[tuple[int, str]]:
    """Yield (line_number, masked_text) for lines that carry prose."""
    out: list[tuple[int, str]] = []
    in_fence = False
    for number, raw in enumerate(text.splitlines(), start=1):
        if raw.lstrip().startswith(("```", "~~~")):
            in_fence = not in_fence
            continue
        if in_fence or HEADING.match(raw) or TABLE_ROW.match(raw):
            continue
        if raw.startswith("    ") and not LIST_ITEM.match(raw):
            continue
        masked = _mask(raw)
        if masked.strip():
            out.append((number, masked))
    return out


def _sentences(line: str) -> list[str]:
    """Split on terminal punctuation and on a colon, per rule 8.4."""
    parts = re.split(r"(?<=[.!?:])\s+", line.strip())
    return [p for p in (s.strip() for s in parts) if p]


def _count_words(sentence: str) -> int:
    """Rule 8.6 / 8.7: numbers with units, abbreviations, and hyphenated
    groups each count as one word. Parentheticals count as one (rule 8.5)."""
    text = re.sub(r"\([^)]*\)", " PAREN ", sentence)
    text = re.sub(r"\b\d+(?:\.\d+)?\s*[A-Za-z%°]+\b", " UNIT ", text)
    tokens = re.findall(r"[A-Za-z0-9][A-Za-z0-9'’-]*", text)
    return len([t for t in tokens if not LIST_ITEM.match(t)])


def _excerpt(text: str, limit: int = 78) -> str:
    flat = " ".join(text.split())
    return flat if len(flat) <= limit else flat[: limit - 1] + "…"


def lint(text: str, mode: str = "prose") -> list[Finding]:
    cap = WORD_CAP[mode]
    voice_pass = mode == "voice"
    findings: list[Finding] = []
    lines = _prose_lines(text)
    whole = " ".join(masked for _, masked in lines).lower()

    for number, line in lines:
        low = line.lower()

        for sentence in _sentences(line):
            count = _count_words(sentence)
            if count > cap:
                findings.append(Finding(
                    number, "5.1/6.3", "major",
                    f"Sentence is {count} words; cap is {cap}. Split it.",
                    _excerpt(sentence)))

        for match in re.finditer(r"\b\w+(?:n't|'ll|'re|'ve|'d)\b|\b(?:it's|that's|there's|let's|we're|you're|don't|doesn't|isn't|aren't|can't|won't)\b", low):
            findings.append(Finding(
                number, "4.2", "minor",
                f"Contraction “{match.group(0)}”. Write it in full.",
                _excerpt(line)))

        if not voice_pass:
            for modal in BANNED_MODALS:
                for match in re.finditer(rf"\b{modal}\b", low):
                    findings.append(Finding(
                        number, "3.2", "major",
                        f"Modal “{modal}” is not approved. Use can, will, or must. "
                        "If it marks real uncertainty, state that as a fact instead "
                        "(\"I did not verify this\").",
                        _excerpt(line)))

            for hedge in VAGUE_HEDGES:
                for match in re.finditer(rf"\b{re.escape(hedge)}\b", low):
                    findings.append(Finding(
                        number, "9.2", "minor",
                        f"Vague qualifier “{match.group(0)}”. Give the number, or "
                        "say plainly what you did not check.",
                        _excerpt(line)))

        for match in re.finditer(r"\b(?:has|have|had)\s+been\b|\b(?:has|have|had)\s+(?:just\s+|already\s+|never\s+|not\s+)?\w+ed\b", low):
            findings.append(Finding(
                number, "3.4", "major",
                f"Perfect tense “{match.group(0)}”. Use the simple past.",
                _excerpt(line)))

        for match in re.finditer(r"\b(?:is|are|was|were|be|been|being)\s+\w+ing\b", low):
            findings.append(Finding(
                number, "3.2", "major",
                f"Progressive tense “{match.group(0)}”. Use the simple present.",
                _excerpt(line)))

        for match in re.finditer(r",\s+(making|allowing|enabling|ensuring|highlighting|creating|providing|offering|helping|reducing|improving|leading|causing|resulting|meaning|giving|showing)\b", low):
            findings.append(Finding(
                number, "3.5", "major",
                f"Trailing “-ing” clause “, {match.group(1)}”. Start a new sentence.",
                _excerpt(line)))

        if ";" in line:
            findings.append(Finding(
                number, "8.1", "major",
                "Semicolon is not permitted. Write two sentences.",
                _excerpt(line)))

        for match in re.finditer(r"\b(?:e\.g\.|i\.e\.|etc\.|viz\.|cf\.)", low):
            findings.append(Finding(
                number, "GR-6", "minor",
                f"Latin abbreviation “{match.group(0)}”. Write it in English.",
                _excerpt(line)))

        if "—" in line:
            findings.append(Finding(
                number, "house", "major",
                "Em dash is forbidden by CLAUDE.md. Use a comma, colon, parentheses, or --.",
                _excerpt(line)))

        for term in SLOP:
            for match in re.finditer(rf"\b{re.escape(term)}\b", low):
                findings.append(Finding(
                    number, "9.2", "minor",
                    f"Marketing word “{match.group(0)}”. Delete it or say the concrete thing.",
                    _excerpt(line)))

        for term in CLAUDESE:
            if term in low:
                findings.append(Finding(
                    number, "house", "minor",
                    f"Claudese tic “{term}”. Say it plainly.",
                    _excerpt(line)))

        for phrase, replacement in SWAP.items():
            if " " not in phrase and phrase in DOMAIN_EXEMPT:
                continue
            for match in re.finditer(rf"\b{re.escape(phrase)}\b", low):
                findings.append(Finding(
                    number, "1.1", "minor",
                    f"“{match.group(0)}” is not approved. Use “{replacement}”.",
                    _excerpt(line)))

        for phrase, replacement in REWRITE.items():
            if phrase in DOMAIN_EXEMPT:
                continue
            for match in re.finditer(rf"\b{re.escape(phrase)}\b", low):
                findings.append(Finding(
                    number, "9.1", "minor",
                    f"“{match.group(0)}” needs a construction change: “{replacement}”.",
                    _excerpt(line)))

        for sentence in _sentences(line):
            body = sentence.lower().lstrip("-*+0123456789.) ")
            if re.match(r"^(if|when|unless|once|after|before)\b", body):
                continue
            trailing = re.search(
                r"\b(if|when|unless|once)\b[^.?!]*[.?!]?$", body)
            if trailing:
                findings.append(Finding(
                    number, "5.4", "minor",
                    f"Condition trails the command (“{trailing.group(1)} …”). "
                    "Put the condition first, then a comma.",
                    _excerpt(sentence)))

    for group in SYNONYM_SETS:
        present = [term for term in group if re.search(rf"\b{re.escape(term)}\b", whole)]
        if len(present) > 1:
            findings.append(Finding(
                0, "9.4", "minor",
                f"Synonym rotation: {', '.join(present)}. Pick one ({present[0]}) and repeat it.",
                ""))

    findings.sort(key=lambda f: (f.line, f.rule))
    return findings


def _render(findings: list[Finding], label: str, mode: str) -> str:
    if not findings:
        return f"{label}: clean ({mode} mode, mechanical checks only)"
    order = {"major": 0, "minor": 1}
    findings = sorted(findings, key=lambda f: (order[f.severity], f.line))
    out = [f"{label}: {len(findings)} finding(s) [{mode} mode]"]
    for f in findings:
        where = f"line {f.line}" if f.line else "document"
        out.append(f"  {f.severity:5}  {where:>11}  rule {f.rule:9} {f.message}")
        if f.excerpt:
            out.append(f"                            → {f.excerpt}")
    majors = sum(1 for f in findings if f.severity == "major")
    out.append(f"  {majors} major, {len(findings) - majors} minor")
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Lint prose for mechanical STE violations.")
    parser.add_argument("files", nargs="+", help="files to lint, or - for stdin")
    parser.add_argument("--mode", choices=sorted(WORD_CAP), default="prose")
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--quiet", action="store_true", help="exit status only")
    args = parser.parse_args(argv)

    total: list[dict] = []
    rendered: list[str] = []
    for name in args.files:
        if name == "-":
            text, label = sys.stdin.read(), "<stdin>"
        else:
            path = Path(name)
            if not path.is_file():
                print(f"ste_lint: no such file: {name}", file=sys.stderr)
                return 2
            text, label = path.read_text(errors="replace"), str(path)
        findings = lint(text, args.mode)
        total.extend({"file": label, **asdict(f)} for f in findings)
        rendered.append(_render(findings, label, args.mode))

    if not args.quiet:
        if args.as_json:
            print(json.dumps(total, indent=2))
        else:
            print("\n".join(rendered))
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
