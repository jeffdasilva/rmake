# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Keep comparison methodology and frozen workload identities reviewable."""

import hashlib
import unittest

import benchmark


class BenchmarkTests(unittest.TestCase):
    def test_pair_order_alternates_without_changing_sample_membership(self):
        self.assertEqual(benchmark.paired_order(0, True), ["oracle", "candidate"])
        self.assertEqual(benchmark.paired_order(1, True), ["candidate", "oracle"])
        self.assertEqual(benchmark.paired_order(1, False), ["oracle"])

    def test_workloads_are_stable_and_cover_distinct_costs(self):
        first = list(benchmark.workloads())
        self.assertEqual(first, list(benchmark.workloads()))
        self.assertEqual([name for name, _ in first], ["tiny-noop", "large-noop", "expansion"])
        self.assertEqual(len({hashlib.sha256(source).digest() for _, source in first}), 3)
        self.assertEqual(first[1][1].count(b": ;"), 10000)
        self.assertEqual(first[2][1].count(b"$(subst"), 5000)

    def test_raw_samples_and_extremes_are_retained(self):
        self.assertEqual(benchmark.summary([5, 1, 3]), {
            "samples_ns": [5, 1, 3], "median_ns": 3, "min_ns": 1, "max_ns": 5})


if __name__ == "__main__":
    unittest.main()
