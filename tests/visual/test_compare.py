"""比較器自体が一致・差分を正しく扱うことを、Office 不要で検証する。"""
import argparse
import unittest
from PIL import Image
from compare import compare_images, passes, fraction


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


if __name__ == "__main__":
    unittest.main()
