#!/usr/bin/env python3

import unittest

from benchmark_capsules import component_scores, parse_raw, performance_score


class PerformanceScoringTests(unittest.TestCase):
    def row(self, large_ns: int, peak: int = 4_194_304) -> dict:
        return {
            "small_p50_ns": 20_000,
            "large_input_bytes": 4_194_304,
            "large_p50_ns": large_ns,
            "large_p95_ns": large_ns * 11 // 10,
            "reported_peak_memory_bytes": peak,
        }

    def test_faster_capability_scores_higher(self) -> None:
        faster = performance_score(component_scores(self.row(2_000_000), 2_000.0))
        slower = performance_score(component_scores(self.row(20_000_000), 2_000.0))
        self.assertGreater(faster, slower)

    def test_memory_amplification_reduces_score(self) -> None:
        compact = performance_score(component_scores(self.row(4_000_000), 1_000.0))
        amplified = performance_score(component_scores(self.row(4_000_000, 33_554_432), 1_000.0))
        self.assertGreater(compact, amplified)

    def test_parser_requires_unique_capabilities(self) -> None:
        row = "EXBENCH\tCAPABILITY\tcapsule:x\tcapability:x\ts\tb\tf\t1\t1\t1\t1\t2\t2\t2\t2\t2"
        raw = "EXBENCH\tCALIBRATION\t2\t1\t1\n" + row + "\n" + row + "\nEXBENCH\tSUMMARY\t2\t1\n"
        with self.assertRaisesRegex(ValueError, "duplicate"):
            parse_raw(raw)


if __name__ == "__main__":
    unittest.main()
