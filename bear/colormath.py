"""Board colour derivation.

Only the dark square is ever authored. The light square and the border are *derived* from
it, which is what keeps a palette coherent: one hue throughout, and a fixed relationship
between the three tones.

The derivation is a port of the Board Colors tool at <https://chessnerd.net/board-colors.html>.
Lightness steps by -0.18 and +0.22, and saturation is *reduced* moving away from the dark
square (border x0.92, light x0.78). That desaturation is the part that stops light squares
going lurid, and it is the reason this works in HSL rather than in linear light.

Relative luminance is used as a diagnostic: a dark square below 0.11 or above 0.42 makes a
board that is unpleasant to look at for an hour.
"""

from __future__ import annotations

RGB = tuple[int, int, int]

#: The band a dark square must sit in to be usable.
OK_BAND = (0.11, 0.42)


def clamp(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def rgb_to_hsl(colour: RGB) -> tuple[float, float, float]:
    r, g, b = colour[0] / 255.0, colour[1] / 255.0, colour[2] / 255.0
    high, low = max(r, g, b), min(r, g, b)
    lightness = (high + low) / 2.0
    span = high - low
    if span == 0.0:
        return 0.0, 0.0, lightness

    saturation = span / (2.0 - high - low) if lightness > 0.5 else span / (high + low)
    if high == r:
        hue = ((g - b) / span) + (6.0 if g < b else 0.0)
    elif high == g:
        hue = ((b - r) / span) + 2.0
    else:
        hue = ((r - g) / span) + 4.0
    return hue / 6.0, saturation, lightness


def _channel(p: float, q: float, t: float) -> int:
    if t < 0.0:
        t += 1.0
    if t > 1.0:
        t -= 1.0
    if t < 1.0 / 6.0:
        value = p + ((q - p) * 6.0 * t)
    elif t < 1.0 / 2.0:
        value = q
    elif t < 2.0 / 3.0:
        value = p + ((q - p) * ((2.0 / 3.0) - t) * 6.0)
    else:
        value = p
    return int(round(clamp(value, 0.0, 1.0) * 255.0))


def hsl_to_rgb(hue: float, saturation: float, lightness: float) -> RGB:
    if saturation == 0.0:
        grey = int(round(lightness * 255.0))
        return grey, grey, grey
    q = (lightness * (1.0 + saturation) if lightness < 0.5
         else lightness + saturation - (lightness * saturation))
    p = (2.0 * lightness) - q
    return (_channel(p, q, hue + 1.0 / 3.0),
            _channel(p, q, hue),
            _channel(p, q, hue - 1.0 / 3.0))


def _to_linear(channel: int) -> float:
    v = channel / 255.0
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def luminance(colour: RGB) -> float:
    """WCAG relative luminance."""
    return (0.2126 * _to_linear(colour[0])
            + 0.7152 * _to_linear(colour[1])
            + 0.0722 * _to_linear(colour[2]))


def in_ok_band(dark: RGB) -> bool:
    return OK_BAND[0] <= luminance(dark) <= OK_BAND[1]


def from_dark_square(dark_rgb: RGB) -> tuple[RGB, RGB, RGB]:
    """Derive the full board from its dark square. Returns (dark, light, border)."""
    hue, saturation, lightness = rgb_to_hsl(dark_rgb)
    dark = hsl_to_rgb(hue, clamp(saturation, 0.0, 0.95), lightness)

    hue, saturation, lightness = rgb_to_hsl(dark)
    border_saturation = clamp(saturation * 0.92, 0.0, 0.88)
    light_saturation = clamp(saturation * 0.78, 0.0, 0.82)
    border_lightness = clamp(lightness - 0.18, 0.06, max(0.07, lightness - 0.06))
    light_lightness = clamp(lightness + 0.22, min(0.92, lightness + 0.08), 0.92)

    return (dark,
            hsl_to_rgb(hue, light_saturation, light_lightness),
            hsl_to_rgb(hue, border_saturation, border_lightness))


def to_hex(colour: RGB) -> str:
    return f"#{colour[0]:02X}{colour[1]:02X}{colour[2]:02X}"


def from_hex(value: str) -> RGB:
    value = value.lstrip("#")
    return int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16)
