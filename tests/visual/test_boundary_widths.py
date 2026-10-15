import unittest
from PIL import Image, ImageDraw
from compare_boundary_widths import line_ink, same_breaks


def ink(widths, color=0):
    image = Image.new('RGB', (100, 80), 'white')
    draw = ImageDraw.Draw(image)
    for index, width in enumerate(widths):
        draw.rectangle((5, 5 + index * 30, 4 + width, 14 + index * 30), fill=(color,) * 3)
    return line_ink(image, 2)


class BoundaryComparisonTest(unittest.TestCase):
    def test_empty_images_are_not_a_success(self):
        self.assertFalse(same_breaks(ink([]), ink([])))

    def test_changed_line_count_is_detected(self):
        self.assertFalse(same_breaks(ink([60]), ink([50, 10])))

    def test_one_character_moved_between_two_lines_is_detected(self):
        self.assertFalse(same_breaks(ink([50, 10]), ink([40, 20])))

    def test_uniform_rasterization_weight_does_not_change_line_shares(self):
        self.assertTrue(same_breaks(ink([50, 10]), ink([50, 10], 80)))

    def test_small_glyph_gaps_merge_but_line_gaps_remain(self):
        image = Image.new('RGB', (100, 80), 'white')
        draw = ImageDraw.Draw(image)
        for y in (5, 9, 35):
            draw.rectangle((5, y, 20, y + 1), fill='black')
        self.assertEqual(len(line_ink(image, 2)['bounds']), 2)


if __name__ == '__main__':
    unittest.main()
