"""Make the Artaveo Dental source logos transparent.

Only removes background connected to the image edges (flood fill from the
border), so thin internal separation lines of the same color survive.
Anti-aliased edge pixels get a soft alpha ramp instead of a hard cutoff.
"""
from pathlib import Path
import numpy as np
from PIL import Image
from scipy.ndimage import label, binary_dilation

def process(path_in, path_out, bg="white", strict=10, soft=70, dilate=3, icon_boundary=None):
    img = Image.open(path_in).convert("RGB")
    arr = np.array(img).astype(np.int16)
    bg_color = np.array([255, 255, 255] if bg == "white" else [0, 0, 0])
    dist = np.sqrt(((arr - bg_color) ** 2).sum(axis=2))

    strict_mask = dist < strict
    labeled, n = label(strict_mask)
    border_labels = set(labeled[0, :]) | set(labeled[-1, :]) | set(labeled[:, 0]) | set(labeled[:, -1])
    border_labels.discard(0)
    core_transparent = np.isin(labeled, list(border_labels))

    # Enclosed (non-border-connected) background islands: keep opaque only inside the
    # icon-mark region (its deliberate thin white separator lines). Everywhere else
    # (wordmark letter counters, AA noise) is removed like ordinary background.
    if icon_boundary is not None:
        from scipy.ndimage import find_objects
        enclosed_labels = set(range(1, n + 1)) - border_labels
        slices = find_objects(labeled)
        outside_icon = set()
        for lbl in enclosed_labels:
            sl = slices[lbl - 1]
            if sl is None:
                continue
            if sl[1].start >= icon_boundary:
                outside_icon.add(lbl)
        if outside_icon:
            core_transparent |= np.isin(labeled, list(outside_icon))

    band = binary_dilation(core_transparent, iterations=dilate) & ~core_transparent

    alpha = np.full(dist.shape, 255, dtype=np.float64)
    alpha[core_transparent] = 0
    band_alpha = np.clip((dist[band] / soft) * 255, 0, 255)
    alpha[band] = band_alpha

    out = np.dstack([np.array(img), alpha.astype(np.uint8)])
    Image.fromarray(out, mode="RGBA").save(path_out)
    print(f"{Path(path_in).name} -> {path_out}  (removed bg, kept {int((alpha>0).sum())}px opaque+)")

if __name__ == "__main__":
    here = Path(__file__).resolve().parent.parent
    src = here / "source"
    dst = here
    jobs = [
        (src / "icon-flat.png", dst / "icon.png", "white", None),
        (src / "lockup-horizontal-light.png", dst / "lockup-light.png", "white", 620),
        (src / "lockup-horizontal-dark.png", dst / "lockup-dark.png", "black", 620),
        (src / "mono-black.png", dst / "mono-black.png", "white", None),
        (src / "mono-white.png", dst / "mono-white.png", "black", None),
    ]
    for i, o, bg, icon_boundary in jobs:
        strict = 24 if "lockup-horizontal-dark" in i.name else 10
        process(str(i), str(o), bg, strict=strict, icon_boundary=icon_boundary)
