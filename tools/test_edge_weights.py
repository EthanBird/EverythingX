#!/usr/bin/env python3
"""Unit tests for Capsule-local performance edge weights."""

from __future__ import annotations

import unittest

from sync_edge_weights import build_documents


class EdgeWeightTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.documents = build_documents()

    def test_all_current_production_capsules_and_capabilities_are_materialized(self) -> None:
        self.assertEqual(len(self.documents), 104)
        self.assertEqual(
            sum(len(document["capabilities"]) for document in self.documents.values()),
            105,
        )

    def test_derived_load_is_inverse_performance_score(self) -> None:
        for document in self.documents.values():
            for capability in document["capabilities"]:
                weight = capability["edge_weight"]
                self.assertAlmostEqual(
                    weight["load_0_to_100"],
                    round(100.0 - weight["performance_score_0_to_100"], 3),
                    places=3,
                )
                self.assertEqual(weight["load_direction"], "higher-is-more-expensive")

    def test_multi_capability_capsule_keeps_separate_edge_weights(self) -> None:
        utf16 = next(
            document
            for document in self.documents.values()
            if document["capsule"]["id"] == "capsule:utf16-to-utf8"
        )
        self.assertEqual(
            {item["strategy"] for item in utf16["capabilities"]},
            {"strict", "replace-invalid"},
        )

    def test_weight_is_environment_bound_and_formula_driven(self) -> None:
        for document in self.documents.values():
            profile = document["profile"]
            self.assertEqual(profile["profile_id"], "exbench:ci-default-v1")
            self.assertEqual(len(profile["harness_sha256"]), 64)
            self.assertEqual(len(profile["environment_fingerprint_sha256"]), 64)
            for capability in document["capabilities"]:
                weight = capability["edge_weight"]
                self.assertIn("N", weight["estimated_latency_micros"])
                self.assertIn("N", weight["estimated_peak_memory_bytes"])
                self.assertIn("N", weight["estimated_output_bytes"])


if __name__ == "__main__":
    unittest.main()
