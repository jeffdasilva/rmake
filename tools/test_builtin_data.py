# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Protect extraction boundaries and observable recipe bytes."""
import unittest

import builtin_data


class ExtractionTests(unittest.TestCase):
    def test_defaults_exclude_automatic_environment_and_runtime_metadata(self):
        text = ("# default\nCC = cc\n# environment\nCFLAGS = injected\n"
                "# automatic\n@D = formula\n# default\nMAKE_HOST := host\n"
                "# default\n.INCLUDE_DIRS := /path\n# Implicit Rules\n"
                "(%): %\n\tarchive\n# Files\n")
        data = builtin_data.extract(text)
        self.assertEqual(data['default-variables.mk'], builtin_data.HEADER + 'CC = cc\n')

    def test_tabs_spaces_empty_lines_and_terminal_colons_are_preserved(self):
        text = ("# Implicit Rules\n%.src:\n\n(%): %\n# builtin\n\tarchive \n\n"
                "%:: %,v\n\t+checkout\n\n# Files\n# builtin\n"
                ".src.out:\n# recipe\n\tfirst \n\t second\n\n.SUFFIXES: .src .out\n")
        data = builtin_data.extract(text)
        self.assertEqual(data['default-suffix-rules.mk'],
                         builtin_data.HEADER + '.src.out:\n\tfirst \n\t second\n')
        self.assertEqual(data['default-pattern-rules.mk'],
                         builtin_data.HEADER + '(%): %\n\tarchive \n\n%:: %,v\n\t+checkout\n')

    def test_missing_sections_nul_and_oversized_database_are_rejected(self):
        for text in ('', '# Implicit Rules\n# Files\n', '\0', 'x' * (512 * 1024 + 1)):
            with self.subTest(size=len(text)), self.assertRaises((ValueError, IndexError)):
                builtin_data.extract(text)
