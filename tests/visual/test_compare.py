"""Verify the comparator handles matches and differences correctly without Office."""
import argparse
import unittest
import json
from pathlib import Path
import tempfile
from PIL import Image
from compare import compare_images, compare_regions, diagnostic_summary, load_regions, passes, fraction


class ComparisonTests(unittest.TestCase):
    def test_identical_images_pass(self):
        image = Image.new("RGB", (10, 10), "white")
        metrics, diff = compare_images(image, image, 16)
        self.assertEqual(metrics["mae"], 0)
        self.assertEqual(metrics["changed_ratio"], 0)
        self.assertIsNone(diff.getbbox())
        self.assertTrue(passes(metrics, 0, 0))

    def test_missing_content_fails_and_has_heatmap(self):
        actual = Image.new("RGB", (10, 10), "white")
        reference = Image.new("RGB", (10, 10), "black")
        metrics, diff = compare_images(actual, reference, 16)
        self.assertEqual(metrics["mae"], 1)
        self.assertEqual(metrics["changed_ratio"], 1)
        self.assertFalse(passes(metrics, 0.02, 0.05))
        self.assertIsNotNone(diff.getbbox())

    def test_pixel_tolerance_does_not_hide_mae(self):
        actual = Image.new("RGB", (10, 10), (255, 255, 255))
        reference = Image.new("RGB", (10, 10), (240, 240, 240))
        metrics, _ = compare_images(actual, reference, 16)
        self.assertEqual(metrics["changed_ratio"], 0)
        self.assertGreater(metrics["mae"], 0.02)
        self.assertFalse(passes(metrics, 0.02, 0.05))

    def test_dimension_mismatch_is_error(self):
        with self.assertRaises(ValueError):
            compare_images(Image.new("RGB", (10, 10)), Image.new("RGB", (11, 10)), 16)

    def test_nan_threshold_is_rejected(self):
        with self.assertRaises(argparse.ArgumentTypeError):
            fraction("nan")


class LocalComparisonTests(unittest.TestCase):
    def test_small_deleted_shape_fails_locally_while_global_passes(self):
        actual = Image.new("RGB", (1000, 1000), "white")
        reference = actual.copy()
        reference.paste("black", (100, 100, 110, 110))
        whole, _ = compare_images(actual, reference, 16)
        regions = compare_regions(actual, reference, [
            {"slide": 1, "name": "小図形", "rect": [.1, .1, .01, .01]}
        ], 16, .02, .05)
        self.assertTrue(passes(whole, .02, .05))
        self.assertFalse(regions[0]["passed"])
        self.assertEqual(regions[0]["mae"], 1)
        self.assertEqual(regions[0]["changed_ratio"], 1)
        self.assertEqual(regions[0]["pixel_rect"], [100, 100, 110, 110])

    def test_no_regions_and_identical_regions(self):
        image = Image.new("RGB", (100, 100), "white")
        self.assertEqual(compare_regions(image, image, [], 16, .02, .05), [])
        result = compare_regions(image, image, [
            {"slide": 1, "name": "全体", "rect": [0, 0, 1, 1]}
        ], 16, .02, .05)
        self.assertTrue(result[0]["passed"])
        self.assertEqual(result[0]["width"], 100)

    def test_fractional_bounds_include_edge_pixels(self):
        image = Image.new("RGB", (10, 10), "white")
        result = compare_regions(image, image, [
            {"slide": 1, "name": "端", "rect": [.01, .01, .01, .01]}
        ], 16, .02, .05)
        self.assertEqual(result[0]["pixel_rect"], [0, 0, 1, 1])

    def test_diagnostics_count_objects_by_code_without_losing_omissions(self):
        summary = diagnostic_summary([
            {"code": "omitted", "object_id": "1"},
            {"code": "omitted", "object_id": "2"},
            {"code": "approximation", "object_id": "3"},
        ])
        self.assertEqual(summary["by_code"], {"approximation": 1, "omitted": 2})
        self.assertTrue(summary["has_omissions"])
        self.assertFalse(diagnostic_summary([])["has_omissions"])
        self.assertIsNone(diagnostic_summary(None)["has_omissions"])
        self.assertFalse(diagnostic_summary(None)["available"])

    def test_regions_validation_rejects_invalid_bounds_and_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "regions.json"
            valid = {"slide": 1, "name": "小図形", "rect": [.1, .1, .2, .2]}
            path.write_text(json.dumps({"regions": [valid]}))
            self.assertEqual(load_regions(path), [valid])
            cases = [
                [], [valid, valid],
                [{**valid, "slide": 0}], [{**valid, "slide": True}],
                [{**valid, "name": ""}],
                *[[{**valid, "rect": rect}] for rect in (
                    [0, 0, 0, .1], [-.1, 0, .1, .1], [.9, 0, .2, .1],
                    [0, .9, .1, .2], [0, 0, float("nan"), .1],
                    [0, 0, True, .1], [0, 0, .1],
                )],
            ]
            for regions in cases:
                with self.subTest(regions=regions):
                    path.write_text(json.dumps({"regions": regions}))
                    with self.assertRaises(ValueError):
                        load_regions(path)
        self.assertEqual(load_regions(None), [])

    def test_threshold_boundaries_are_unchanged(self):
        self.assertTrue(passes({"mae": .02, "changed_ratio": .05}, .02, .05))
        self.assertFalse(passes({"mae": .020001, "changed_ratio": .05}, .02, .05))
        self.assertFalse(passes({"mae": .02, "changed_ratio": .050001}, .02, .05))


if __name__ == "__main__":
    unittest.main()
