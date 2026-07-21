"""Board palettes, resolved from a name.

`bear/palettes.py` stores only the dark square of each board. The light square and the
border come from `bear.colormath.from_dark_square`, so a palette is always internally
consistent and there is only ever one colour to get right.

Deliberately free of Qt: a palette is three RGB triples, and the widget turns them into
whatever it needs.
"""

from __future__ import annotations

from dataclasses import dataclass

from bear.colormath import RGB, from_dark_square, luminance, to_hex
from bear.palettes import DEFAULT, PALETTES


@dataclass(frozen=True)
class Palette:
    family: str
    chroma: str
    shade: str
    dark: RGB
    light: RGB
    border: RGB

    @property
    def key(self) -> str:
        return f"{self.family}-{self.chroma}-{self.shade}"

    @property
    def label(self) -> str:
        """How it reads in a menu, under its family: "Soft Mid"."""
        return f"{self.chroma} {self.shade}"

    @property
    def luminance(self) -> float:
        return luminance(self.dark)

    def hex(self) -> tuple[str, str, str]:
        return to_hex(self.dark), to_hex(self.light), to_hex(self.border)


def _build() -> dict[str, Palette]:
    out: dict[str, Palette] = {}
    for family, chroma, shade, dark_hex in PALETTES:
        red, green, blue = (int(dark_hex[1:3], 16), int(dark_hex[3:5], 16),
                            int(dark_hex[5:7], 16))
        dark, light, border = from_dark_square((red, green, blue))
        palette = Palette(family, chroma, shade, dark, light, border)
        out[palette.key] = palette
    return out


ALL: dict[str, Palette] = _build()


def families() -> list[str]:
    """Family names, in the order the generator emitted them."""
    seen: list[str] = []
    for palette in ALL.values():
        if palette.family not in seen:
            seen.append(palette.family)
    return seen


def in_family(family: str) -> list[Palette]:
    return [p for p in ALL.values() if p.family == family]


def get(key: str | None) -> Palette:
    """A palette by key, falling back to the default rather than failing."""
    if key and key in ALL:
        return ALL[key]
    return ALL.get(DEFAULT) or next(iter(ALL.values()))
