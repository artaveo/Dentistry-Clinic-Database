"""Generate the Tauri app icon set (app/icons/) from the transparent icon.png.

Also builds icon-small.png: the same mark with its separator gap
morphologically thickened, used only for <=32px frames where the original's
thin lines blur into noise (see ../README.md).

The .ico is assembled by hand (not via Pillow's single-source ICO writer,
which can't mix two source images) so that 16/24/32px frames come from the
simplified mark and 48/256px frames come from the full-detail one.
"""
import io
import struct
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import binary_dilation

BRAND = Path(__file__).resolve().parent.parent
ICONS = BRAND.parent.parent / "app" / "icons"


def make_small_variant(full: Image.Image) -> Image.Image:
    arr = np.array(full)
    fg = arr[:, :, 3] > 0
    white = (arr[:, :, 0] > 200) & (arr[:, :, 1] > 200) & (arr[:, :, 2] > 200) & fg
    dilated = binary_dilation(white, iterations=35) & fg
    out = arr.copy()
    newly = dilated & ~white
    out[newly, 0] = 255
    out[newly, 1] = 255
    out[newly, 2] = 255
    out[newly, 3] = 255
    return Image.fromarray(out, "RGBA")


def resize(src: Image.Image, size: int) -> Image.Image:
    return src.resize((size, size), Image.LANCZOS)


def write_ico(path: Path, images: list[tuple[int, Image.Image]]) -> None:
    """images: [(size, RGBA image)], each embedded as its own PNG (Vista+ ICO format)."""
    header = struct.pack("<HHH", 0, 1, len(images))
    entries = b""
    data = b""
    offset = 6 + 16 * len(images)
    for size, im in images:
        buf = io.BytesIO()
        im.save(buf, format="PNG")
        png_bytes = buf.getvalue()
        wh = size if size < 256 else 0
        entries += struct.pack("<BBBBHHII", wh, wh, 0, 0, 1, 32, len(png_bytes), offset)
        data += png_bytes
        offset += len(png_bytes)
    path.write_bytes(header + entries + data)


if __name__ == "__main__":
    full = Image.open(BRAND / "icon.png").convert("RGBA")
    small = make_small_variant(full)
    small.save(BRAND / "icon-small.png")

    resize(small, 32).save(ICONS / "32x32.png")
    resize(full, 128).save(ICONS / "128x128.png")
    resize(full, 256).save(ICONS / "128x128@2x.png")
    resize(full, 512).save(ICONS / "icon.png")

    ico_sizes = [16, 24, 32, 48, 256]
    frames = [(s, resize(small if s <= 32 else full, s)) for s in ico_sizes]
    write_ico(ICONS / "icon.ico", frames)

    print("done:", ", ".join(f"{s}px" for s, _ in frames))
