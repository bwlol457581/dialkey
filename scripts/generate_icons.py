#!/usr/bin/env python3
"""Generate DialKey icon assets (spec §9: dial-pad motif).

Tray (16/32): simplified 3×3 grid, one accent tile.
App (256): full 3×4 phone keypad, multi-color, no adjacent same color.

Outputs under assets/:
  dialkey.ico          — multi-resolution for winres / Shell
  dialkey-256.png      — app master
  dialkey-tray-16.png  — tray preview
  dialkey-tray-32.png  — tray preview
"""

from __future__ import annotations

import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"

# Dark teal chassis (matches early tray glyph intent)
BG = (15, 42, 61, 255)  # #0F2A3D

# Tray: muted keys + single accent
TRAY_KEY = (238, 238, 238, 255)
TRAY_ACCENT = (240, 144, 64, 255)  # #F09040

# App 3×4: graph-colored so edge-adjacent tiles differ
# Layout (phone keypad):
#   1 2 3
#   4 5 6
#   7 8 9
#   * 0 #
PAL_A = (42, 157, 143, 255)  # teal
PAL_B = (233, 196, 106, 255)  # gold
PAL_C = (244, 162, 97, 255)  # orange
APP_GRID = [
    [PAL_A, PAL_B, PAL_C],
    [PAL_B, PAL_C, PAL_A],
    [PAL_C, PAL_A, PAL_B],
    [PAL_A, PAL_B, PAL_C],
]


def _round_rect(draw: ImageDraw.ImageDraw, box, radius: int, fill) -> None:
    draw.rounded_rectangle(box, radius=radius, fill=fill)


def render_tray(size: int) -> Image.Image:
    """3×3 simplified grid; bottom-right tile is the accent."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    pad = max(1, size // 16)
    radius = max(1, size // 8)
    _round_rect(draw, (0, 0, size - 1, size - 1), radius, BG)

    cols, rows = 3, 3
    inner = size - 2 * pad
    gap = max(1, size // 16)
    cell = (inner - gap * (cols - 1)) // cols
    # Center the grid in the padded area
    used = cell * cols + gap * (cols - 1)
    ox = pad + (inner - used) // 2
    oy = pad + (inner - used) // 2

    for r in range(rows):
        for c in range(cols):
            x0 = ox + c * (cell + gap)
            y0 = oy + r * (cell + gap)
            fill = TRAY_ACCENT if (r == rows - 1 and c == cols - 1) else TRAY_KEY
            cr = max(0, cell // 4)
            _round_rect(draw, (x0, y0, x0 + cell - 1, y0 + cell - 1), cr, fill)

    return img


def render_app(size: int) -> Image.Image:
    """3×4 full dial pad."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    pad = max(8, size // 16)
    radius = max(12, size // 10)
    _round_rect(draw, (0, 0, size - 1, size - 1), radius, BG)

    cols, rows = 3, 4
    gap = max(4, size // 32)
    inner_w = size - 2 * pad
    inner_h = size - 2 * pad
    cell_w = (inner_w - gap * (cols - 1)) // cols
    cell_h = (inner_h - gap * (rows - 1)) // rows
    used_w = cell_w * cols + gap * (cols - 1)
    used_h = cell_h * rows + gap * (rows - 1)
    ox = pad + (inner_w - used_w) // 2
    oy = pad + (inner_h - used_h) // 2
    cr = max(2, min(cell_w, cell_h) // 5)

    for r in range(rows):
        for c in range(cols):
            x0 = ox + c * (cell_w + gap)
            y0 = oy + r * (cell_h + gap)
            fill = APP_GRID[r][c]
            _round_rect(
                draw,
                (x0, y0, x0 + cell_w - 1, y0 + cell_h - 1),
                cr,
                fill,
            )

    return img


def render_for_ico(size: int) -> Image.Image:
    """ICO sizes: use tray motif below 48px (legibility), app pad at 48+."""
    if size < 48:
        return render_tray(size)
    return render_app(size)


def write_ico(path: Path, frames: list[Image.Image]) -> None:
    """Write a Vista+ ICO with PNG-compressed frames (reliable multi-size)."""
    pngs: list[bytes] = []
    for frame in frames:
        buf = io.BytesIO()
        frame.convert("RGBA").save(buf, format="PNG")
        pngs.append(buf.getvalue())

    count = len(frames)
    # ICONDIR + ICONDIRENTRY[count] + image data
    offset = 6 + 16 * count
    entries = bytearray()
    blob = bytearray()
    for frame, png in zip(frames, pngs):
        w, h = frame.size
        # 0 means 256 in the classic ICONDIRENTRY fields
        entry_w = 0 if w >= 256 else w
        entry_h = 0 if h >= 256 else h
        entries += struct.pack(
            "<BBBBHHII",
            entry_w,
            entry_h,
            0,  # color palette
            0,  # reserved
            1,  # planes
            32,  # bit count
            len(png),
            offset + len(blob),
        )
        blob += png

    header = struct.pack("<HHH", 0, 1, count)  # reserved, type=icon, count
    path.write_bytes(header + entries + blob)


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)

    tray16 = render_tray(16)
    tray32 = render_tray(32)
    app256 = render_app(256)

    tray16.save(ASSETS / "dialkey-tray-16.png")
    tray32.save(ASSETS / "dialkey-tray-32.png")
    app256.save(ASSETS / "dialkey-256.png")

    # Multi-size ICO: tray motif at small sizes, full pad at 48+
    sizes = [16, 32, 48, 256]
    frames = [render_for_ico(s) for s in sizes]
    write_ico(ASSETS / "dialkey.ico", frames)

    print(f"Wrote icons under {ASSETS}")
    for name in (
        "dialkey.ico",
        "dialkey-256.png",
        "dialkey-tray-16.png",
        "dialkey-tray-32.png",
    ):
        path = ASSETS / name
        print(f"  {name}: {path.stat().st_size} bytes")

    ico = Image.open(ASSETS / "dialkey.ico")
    print(f"  ICO sizes: {sorted(ico.info.get('sizes', []))}")


if __name__ == "__main__":
    main()

