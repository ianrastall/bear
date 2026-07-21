"""Generate Bear board palettes.

Faithful port of src/Bear.App/Theming/ColorMath.cs + BoardTheme.FromDarkSquare,
so hex values produced here are identical to what the C# will emit.
"""
import math

# ---------- ColorMath.cs port ----------

def rgb_to_hsl(c):
    r, g, b = c[0] / 255.0, c[1] / 255.0, c[2] / 255.0
    mx, mn = max(r, g, b), min(r, g, b)
    l = (mx + mn) / 2.0
    d = mx - mn
    if d == 0.0:
        return (0.0, 0.0, l)
    s = d / (2.0 - mx - mn) if l > 0.5 else d / (mx + mn)
    if mx == r:
        h = ((g - b) / d) + (6.0 if g < b else 0.0)
    elif mx == g:
        h = ((b - r) / d) + 2.0
    else:
        h = ((r - g) / d) + 4.0
    return (h / 6.0, s, l)


def _channel(p, q, t):
    x = t
    if x < 0.0:
        x += 1.0
    if x > 1.0:
        x -= 1.0
    if x < 1.0 / 6.0:
        v = p + ((q - p) * 6.0 * x)
    elif x < 1.0 / 2.0:
        v = q
    elif x < 2.0 / 3.0:
        v = p + ((q - p) * ((2.0 / 3.0) - x) * 6.0)
    else:
        v = p
    # C# (byte)Math.Round uses banker's rounding; Python round() matches.
    return int(round(min(max(v, 0.0), 1.0) * 255.0))


def hsl_to_rgb(h, s, l):
    if s == 0.0:
        grey = int(round(l * 255.0))
        return (grey, grey, grey)
    q = l * (1.0 + s) if l < 0.5 else l + s - (l * s)
    p = (2.0 * l) - q
    return (_channel(p, q, h + 1.0 / 3.0), _channel(p, q, h), _channel(p, q, h - 1.0 / 3.0))


def _to_linear(ch):
    v = ch / 255.0
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def luminance(c):
    return 0.2126 * _to_linear(c[0]) + 0.7152 * _to_linear(c[1]) + 0.0722 * _to_linear(c[2])


def clamp(v, lo, hi):
    return max(lo, min(hi, v))


def from_dark_square(dark_rgb):
    """Port of BoardTheme.FromDarkSquare. Returns (dark, light, border)."""
    ih, isat, il = rgb_to_hsl(dark_rgb)
    dark = hsl_to_rgb(ih, clamp(isat, 0.0, 0.95), il)
    dh, ds, dl = rgb_to_hsl(dark)
    border_sat = clamp(ds * 0.92, 0.0, 0.88)
    light_sat = clamp(ds * 0.78, 0.0, 0.82)
    border_l = clamp(dl - 0.18, 0.06, max(0.07, dl - 0.06))
    light_l = clamp(dl + 0.22, min(0.92, dl + 0.08), 0.92)
    return dark, hsl_to_rgb(dh, light_sat, light_l), hsl_to_rgb(dh, border_sat, border_l)


def status_ok(dark):
    lum = luminance(dark)
    return 0.11 <= lum <= 0.42


# ---------- CIELAB for perceptual spread ----------

def to_lab(c):
    def f(t):
        return t ** (1 / 3) if t > 0.008856 else (7.787 * t) + (16 / 116)
    r, g, b = (_to_linear(c[0]), _to_linear(c[1]), _to_linear(c[2]))
    x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047
    y = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 1.00000
    z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883
    fx, fy, fz = f(x), f(y), f(z)
    return (116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz))


def delta_e(a, b):
    la, lb = to_lab(a), to_lab(b)
    return math.sqrt(sum((la[i] - lb[i]) ** 2 for i in range(3)))


# ---------- families ----------

FAMILIES = [
    ("Red",     345, 15,  ["Brick", "Garnet", "Terracotta", "Rust", "Barn", "Oxblood", "Ember", "Clay"]),
    ("Orange",  15,  40,  ["Copper", "Sienna", "Ochre", "Amber", "Pumpkin", "Cinnamon", "Apricot", "Tangerine"]),
    ("Gold",    40,  55,  ["Brass", "Mustard", "Honey", "Wheat", "Bronze", "Saffron", "Straw", "Marigold"]),
    ("Olive",   55,  80,  ["Moss", "Khaki", "Fern", "Drab", "Lichen", "Sage", "Pesto", "Chartreuse"]),
    ("Green",   80,  150, ["Forest", "Basil", "Hunter", "Pine", "Jade", "Clover", "Ivy", "Emerald"]),
    ("Teal",    150, 180, ["Pine", "Spruce", "Viridian", "Lagoon", "Juniper", "Eucalyptus", "Deep", "Verdigris"]),
    ("Cyan",    180, 200, ["Slate", "Petrol", "Lagoon", "Glacier", "Aqua", "Cerulean", "Marine", "Mist"]),
    ("Blue",    200, 240, ["Cadet", "Steel", "Denim", "Cobalt", "Sapphire", "Navy", "Azure", "Powder"]),
    ("Indigo",  240, 270, ["Slate", "Twilight", "Ink", "Periwinkle", "Iris", "Midnight", "Lupine", "Dusk"]),
    ("Violet",  270, 300, ["Amethyst", "Heather", "Orchid", "Aubergine", "Lavender", "Thistle", "Grape", "Mauve"]),
    ("Magenta", 300, 330, ["Plum", "Fuchsia", "Damson", "Wine", "Mulberry", "Peony", "Byzantine", "Berry"]),
    ("Rose",    330, 345, ["Dusty", "Blush", "Claret", "Rosewood", "Cerise", "Antique", "Coral", "Ruby"]),
]

NEUTRALS = ["Graphite", "Ash", "Slate", "Pewter", "Charcoal", "Stone"]

PER_FAMILY = 8
MIN_DELTA_E = 9.0


def sweep(h_lo, h_hi):
    """All Ok-band candidates within a hue range."""
    out = []
    hues = [h_lo + i for i in range(0, (h_hi - h_lo) % 360 or 360, 2)]
    for hd in hues:
        h = (hd % 360) / 360.0
        for s10 in range(12, 96, 3):
            for l10 in range(18, 62, 2):
                s, l = s10 / 100.0, l10 / 100.0
                dark = hsl_to_rgb(h, clamp(s, 0.0, 0.95), l)
                if status_ok(dark):
                    out.append(dark)
    # unique by hex
    seen, uniq = set(), []
    for c in out:
        k = (c[0] << 16) | (c[1] << 8) | c[2]
        if k not in seen:
            seen.add(k)
            uniq.append(c)
    return uniq


def farthest_point(cands, n, min_de):
    """Greedy max-spread selection in Lab."""
    if not cands:
        return []
    # seed with the most chromatic mid-lightness candidate
    def chroma(c):
        lab = to_lab(c)
        return math.hypot(lab[1], lab[2])
    picked = [max(cands, key=chroma)]
    while len(picked) < n:
        best, best_d = None, -1.0
        for c in cands:
            d = min(delta_e(c, p) for p in picked)
            if d > best_d:
                best, best_d = c, d
        if best is None or best_d < min_de:
            break
        picked.append(best)
    return picked


def main():
    total_candidates = 0
    rows = []
    for name, h_lo, h_hi, quals in FAMILIES:
        cands = sweep(h_lo, h_hi)
        total_candidates += len(cands)
        chosen = farthest_point(cands, PER_FAMILY, MIN_DELTA_E)
        # order light -> dark for a sane menu
        chosen.sort(key=luminance, reverse=True)
        for i, c in enumerate(chosen):
            rows.append((f"{name}-{quals[i % len(quals)]}", c))

    # neutrals: saturation 0
    ncands = [hsl_to_rgb(0.0, 0.0, l / 100.0) for l in range(18, 62)]
    ncands = [c for c in ncands if status_ok(c)]
    total_candidates += len(ncands)
    nchosen = farthest_point(ncands, len(NEUTRALS), 6.0)
    nchosen.sort(key=luminance, reverse=True)
    for i, c in enumerate(nchosen):
        rows.append((f"Neutral-{NEUTRALS[i % len(NEUTRALS)]}", c))

    print(f"# raw Ok-band candidates: {total_candidates}")
    print(f"# selected palettes:      {len(rows)}")
    print()
    for nm, c in rows:
        dark, light, border = from_dark_square(c)
        hexv = f"0x{c[0]:02X}{c[1]:02X}{c[2]:02X}"
        print(f'        FromDarkSquare("{nm}",'.ljust(46) + f'{hexv}),'
              + f'  // lum {luminance(dark):.3f}  light #{light[0]:02X}{light[1]:02X}{light[2]:02X}'
              + f'  border #{border[0]:02X}{border[1]:02X}{border[2]:02X}')


if __name__ == "__main__":
    main()
