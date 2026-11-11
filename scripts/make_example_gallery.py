#!/usr/bin/env python3
"""Make the visual review contact sheet from docs/visuals/examples/*.png.

Requires Pillow. Run after scripts/capture_examples.py.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "docs" / "visuals" / "examples"
OUTPUT = ROOT / "docs" / "visuals" / "example_gallery.png"
CELL_WIDTH = 360
CELL_HEIGHT = 260
COLUMNS = 4


def main() -> None:
    paths = sorted(SOURCE.glob("*.png"))
    paths = [path for path in paths if path.stem not in {
        "menus_object_actions", "menus_transform_pie", "fps_world_ui_range"
    } and not path.stem.startswith("properties_editor_")]
    if not paths:
        raise SystemExit("No example screenshots found; run scripts/capture_examples.py first")
    rows = (len(paths) + COLUMNS - 1) // COLUMNS
    sheet = Image.new("RGB", (CELL_WIDTH * COLUMNS, CELL_HEIGHT * rows), "#161616")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.truetype(str(ROOT / "assets" / "fonts" / "Inter[opsz,wght].ttf"), 18)
    for index, path in enumerate(paths):
        x = (index % COLUMNS) * CELL_WIDTH
        y = (index // COLUMNS) * CELL_HEIGHT
        draw.rounded_rectangle(
            (x + 5, y + 5, x + CELL_WIDTH - 5, y + CELL_HEIGHT - 5),
            radius=7,
            fill="#303030",
        )
        with Image.open(path) as image:
            preview = image.convert("RGB")
            preview.thumbnail((CELL_WIDTH - 22, CELL_HEIGHT - 45), Image.Resampling.LANCZOS)
        px = x + (CELL_WIDTH - preview.width) // 2
        py = y + 12 + (CELL_HEIGHT - 48 - preview.height) // 2
        sheet.paste(preview, (px, py))
        draw.text((x + 13, y + CELL_HEIGHT - 31), path.stem.replace("_", " ").title(), font=font, fill="#f0f0f0")
    sheet.save(OUTPUT, optimize=True)
    print(f"Wrote {OUTPUT} from {len(paths)} example screenshots")


if __name__ == "__main__":
    main()
