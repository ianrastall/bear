"""Palettes on a regular Hue x Saturation x Luminance grid.

Instead of scattering samples and naming them afterwards, solve for the cell:
for each (hue, saturation tier, luminance tier), binary-search the HSL lightness
that lands the dark square's WCAG luminance in the target tier. Luminance rises
monotonically with L at fixed H/S, so the search always converges if the tier is
reachable at all -- and "is it reachable" is exactly what we need to find out.
"""


from _colormath import hsl_to_rgb, luminance, rgb_to_hsl, from_dark_square, clamp

FAMILIES = [
    ("Red", 5), ("Orange", 25), ("Amber", 45), ("Olive", 68),
    ("Green", 115), ("Teal", 165), ("Cyan", 190), ("Blue", 215),
    ("Indigo", 250), ("Violet", 285), ("Magenta", 315), ("Rose", 338),
]

SATS = [("Muted", 0.14), ("Soft", 0.30), ("Rich", 0.46)]

# Three tiers spanning the Ok band (0.11-0.42), by target luminance.
LUMS = [("Dark", 0.155), ("Mid", 0.245), ("Light", 0.350)]

TOLERANCE = 0.006


def solve_l(hue_deg, sat, target_lum):
    """Binary-search HSL lightness for a dark square at the target luminance."""
    lo, hi = 0.05, 0.95
    h = hue_deg / 360.0
    for _ in range(60):
        mid = (lo + hi) / 2.0
        lum = luminance(hsl_to_rgb(h, clamp(sat, 0.0, 0.95), mid))
        if lum < target_lum:
            lo = mid
        else:
            hi = mid
    l = (lo + hi) / 2.0
    c = hsl_to_rgb(h, clamp(sat, 0.0, 0.95), l)
    return c, abs(luminance(c) - target_lum)


def main():
    rows, misses = [], []
    for fam, hue in FAMILIES:
        for sname, sat in SATS:
            for lname, target in LUMS:
                c, err = solve_l(hue, sat, target)
                if err > TOLERANCE:
                    misses.append((fam, sname, lname, err, luminance(c)))
                    continue
                rows.append((fam, sname, lname, c))

    neutrals = []
    for lname, target in LUMS:
        c, err = solve_l(0, 0.0, target)
        neutrals.append(("Neutral", "-", lname, c))

    print(f"# grid cells requested: {len(FAMILIES) * len(SATS) * len(LUMS)}")
    print(f"# cells solved:         {len(rows)}")
    print(f"# unreachable cells:    {len(misses)}")
    for m in misses:
        print(f"    MISS {m[0]}/{m[1]}/{m[2]}  best lum {m[4]:.3f} (target off by {m[3]:.3f})")
    print()

    hexes = set()
    dupes = 0
    for fam, s, l, c in rows + neutrals:
        k = (c[0] << 16) | (c[1] << 8) | c[2]
        if k in hexes:
            dupes += 1
        hexes.add(k)
    print(f"# total palettes: {len(rows) + len(neutrals)}   duplicate hexes: {dupes}")
    print()

    for fam, s, l, c in rows + neutrals:
        dark, *_ = from_dark_square(c)
        chroma = "None" if s == "-" else s
        call = f'FromDarkSquare("{fam}", Chroma.{chroma}, Shade.{l}, 0x{c[0]:02X}{c[1]:02X}{c[2]:02X}),'
        print(f"        {call}".ljust(74) + f"// lum {luminance(dark):.3f}")


if __name__ == "__main__":
    main()
