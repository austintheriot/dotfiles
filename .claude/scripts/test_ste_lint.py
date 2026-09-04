#!/usr/bin/env python3
"""Unit tests for ste_lint.

Stdlib unittest, no third-party runner, because nothing else in the dotfiles
repo installs one.

    python3 -m unittest discover -s ~/.claude/scripts -p 'test_*.py'
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from ste_lint import (  # noqa: E402
    _count_words,
    _excerpt,
    _mask,
    _prose_lines,
    _sentences,
    lint,
)


def rules(text: str, mode: str = "prose") -> set[str]:
    return {finding.rule for finding in lint(text, mode)}


def messages(text: str, mode: str = "prose") -> str:
    return " ".join(finding.message for finding in lint(text, mode))


class MaskTests(unittest.TestCase):
    def test_inline_code_is_blanked_but_length_is_kept(self):
        masked = _mask("Run `git status` now.")
        self.assertNotIn("git status", masked)
        self.assertEqual(len(masked), len("Run `git status` now."))

    def test_urls_are_blanked(self):
        self.assertNotIn("example.com", _mask("See https://example.com/a for more."))

    def test_paths_are_blanked(self):
        self.assertNotIn("tmux-split", _mask("Edit ~/.scripts/tmux-split.sh today."))

    def test_snake_and_camel_identifiers_are_blanked(self):
        masked = _mask("Call update_window and then selectPane.")
        self.assertNotIn("update_window", masked)
        self.assertNotIn("selectPane", masked)

    def test_quoted_strings_are_blanked(self):
        self.assertNotIn("hello there", _mask('It prints "hello there" once.'))

    def test_markdown_link_keeps_its_text_and_drops_the_target(self):
        masked = _mask("See [the guide](https://example.com/guide) first.")
        self.assertIn("the guide", masked)
        self.assertNotIn("example.com", masked)


class ProseLineTests(unittest.TestCase):
    def test_fenced_blocks_are_skipped(self):
        text = "Prose here.\n```\nshould may might\n```\nMore prose.\n"
        numbers = [number for number, _ in _prose_lines(text)]
        self.assertEqual(numbers, [1, 5])

    def test_headings_are_skipped(self):
        self.assertEqual(_prose_lines("## A heading\n"), [])

    def test_table_rows_are_skipped(self):
        self.assertEqual(_prose_lines("| a | b |\n"), [])

    def test_indented_code_is_skipped_but_indented_list_items_are_not(self):
        self.assertEqual(_prose_lines("    literal code line\n"), [])
        self.assertEqual(len(_prose_lines("    - a list item\n")), 1)

    def test_blank_and_masked_out_lines_are_dropped(self):
        self.assertEqual(_prose_lines("\n`only_code`\n"), [])


class SentenceTests(unittest.TestCase):
    def test_splits_on_terminal_punctuation(self):
        self.assertEqual(_sentences("One. Two! Three?"), ["One.", "Two!", "Three?"])

    def test_splits_on_a_colon(self):
        self.assertEqual(_sentences("Do this: then that."), ["Do this:", "then that."])

    def test_a_single_clause_stays_whole(self):
        self.assertEqual(_sentences("Just one clause"), ["Just one clause"])


class WordCountTests(unittest.TestCase):
    def test_plain_words_are_counted(self):
        self.assertEqual(_count_words("one two three four"), 4)

    def test_a_parenthetical_counts_as_one_word(self):
        # "Set the flag PAREN now" is five tokens.
        self.assertEqual(_count_words("Set the flag (the one on the left) now"), 5)

    def test_a_number_with_a_unit_counts_as_one_word(self):
        self.assertEqual(_count_words("Wait 30 seconds"), 2)

    def test_a_hyphenated_group_counts_as_one_word(self):
        self.assertEqual(_count_words("Use the well-known method"), 4)


class ExcerptTests(unittest.TestCase):
    def test_whitespace_is_collapsed(self):
        self.assertEqual(_excerpt("a   b\n c"), "a b c")

    def test_long_text_is_truncated_with_an_ellipsis(self):
        result = _excerpt("word " * 40, limit=20)
        self.assertEqual(len(result), 20)
        self.assertTrue(result.endswith("…"))


class LintRuleTests(unittest.TestCase):
    def test_a_sentence_over_the_cap_is_a_major_finding(self):
        long_sentence = " ".join(f"word{index}" for index in range(30)) + "."
        findings = [f for f in lint(long_sentence) if f.rule == "5.1/6.3"]
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0].severity, "major")

    def test_the_word_cap_is_tighter_in_procedural_mode(self):
        sentence = " ".join(f"word{index}" for index in range(22)) + "."
        self.assertNotIn("5.1/6.3", rules(sentence, "prose"))
        self.assertIn("5.1/6.3", rules(sentence, "procedural"))

    def test_contractions_are_flagged(self):
        self.assertIn("4.2", rules("It doesn't work."))

    def test_banned_modals_are_flagged(self):
        self.assertIn("3.2", rules("You should restart it."))

    def test_voice_mode_allows_modals_and_hedges(self):
        self.assertNotIn("3.2", rules("You should restart it.", "voice"))
        self.assertNotIn("9.2", rules("It is probably fine.", "voice"))

    def test_vague_hedges_are_flagged(self):
        self.assertIn("9.2", rules("It is probably fine."))

    def test_perfect_tense_is_flagged(self):
        self.assertIn("3.4", rules("The task has been done."))

    def test_progressive_tense_is_flagged(self):
        self.assertIn("3.2", rules("The server is running out of room."))

    def test_a_trailing_ing_clause_is_flagged(self):
        self.assertIn("3.5", rules("It restarts, causing a delay."))

    def test_semicolons_are_flagged(self):
        self.assertIn("8.1", rules("It starts; then it stops."))

    def test_latin_abbreviations_are_flagged(self):
        self.assertIn("GR-6", rules("Use a flag, e.g. --force."))

    def test_em_dashes_are_flagged(self):
        self.assertIn("house", rules("This is the reason — it failed."))

    def test_a_trailing_condition_is_flagged(self):
        self.assertIn("5.4", rules("Restart the server if the light is red."))

    def test_a_leading_condition_is_accepted(self):
        self.assertNotIn("5.4", rules("If the light is red, restart the server."))

    def test_unapproved_words_are_swapped(self):
        self.assertIn("1.1", rules("Utilize the tool."))
        self.assertIn("use", messages("Utilize the tool."))

    def test_rewrite_words_are_flagged_separately(self):
        self.assertIn("9.1", rules("Ensure the light is off."))

    def test_software_vocabulary_is_never_flagged(self):
        clean = "Read the file. Read the log. Read the branch."
        self.assertEqual(rules(clean), set())

    def test_synonym_rotation_is_reported_once_for_the_document(self):
        findings = [f for f in lint("Check the value.\nVerify the value.\n") if f.rule == "9.4"]
        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0].line, 0)

    def test_code_fences_are_exempt(self):
        self.assertEqual(rules("```\nYou should utilize it; e.g. now.\n```\n"), set())

    def test_inline_code_is_exempt(self):
        self.assertEqual(rules("Call `utilize_helper` first."), set())

    def test_clean_prose_produces_no_findings(self):
        self.assertEqual(rules("Start the server. The light comes on."), set())

    def test_findings_are_sorted_by_line(self):
        text = "It starts; then stops.\nYou should go.\nIt doesn't work.\n"
        lines = [finding.line for finding in lint(text)]
        self.assertEqual(lines, sorted(lines))


if __name__ == "__main__":
    unittest.main()
