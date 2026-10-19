"""Oversized JPEG for downscale tests: 8400x64, left half red, right half blue (wider than 8192)."""
from pathlib import Path
from PIL import Image

image = Image.new('RGB', (8400, 64), (255, 0, 0))
image.paste((0, 0, 255), (4200, 0, 8400, 64))
image.save(Path(__file__).with_name('oversized.jpg'), quality=90)
