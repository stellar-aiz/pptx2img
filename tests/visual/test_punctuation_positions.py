import unittest
from PIL import Image, ImageDraw
from compare_punctuation_positions import compare_positions, transport_distance


def specimen(positions, tone=0):
    image = Image.new('RGB', (100, 60), 'white')
    draw = ImageDraw.Draw(image)
    for x in positions:
        draw.rectangle((x, 10, x + 9, 29), fill=(tone,) * 3)
    return image


class PositionComparisonTest(unittest.TestCase):
    def test_internal_shift_is_detected_even_with_same_outer_bounds(self):
        result = compare_positions(specimen([5, 35, 65]), specimen([5, 45, 65]), 24)
        self.assertTrue(result['line_breaks_match'])
        self.assertEqual(result['lines'][0]['edge_error_px'], 0)
        self.assertFalse(result['passed'])

    def test_small_raster_offset_and_weight_change_are_allowed(self):
        self.assertTrue(compare_positions(specimen([5, 35, 65]), specimen([6, 36, 66], 80), 24)['passed'])

    def test_empty_image_and_wrong_image_size_fail(self):
        empty = Image.new('RGB', (100, 60), 'white')
        self.assertFalse(compare_positions(empty, empty, 24)['passed'])
        self.assertFalse(compare_positions(empty, empty.resize((50, 60)), 24)['passed'])

    def test_transport_distance_has_pixel_units(self):
        self.assertEqual(transport_distance([1, 0, 0, 0], [0, 0, 0, 1]), 3)
        self.assertIsNone(transport_distance([], []))


if __name__ == '__main__':
    unittest.main()
