#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# dependencies = ["fonttools"]
# ///
"""Convert text to SVG paths using a font file.

Usage:
    uv run scripts/text_to_svg.py \
        --font-file path/to/FunnelDisplay-SemiBold.ttf \
        --text "modular-agent-core" \
        --size 30 --weight 600 --style outline \
        -o doc/images/modular_agent_core_title.svg
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont


# -- text → SVG --------------------------------------------------------------


def text_to_svg(
    font_path: Path,
    text: str,
    size: float,
    weight: int = 400,
    style: str = "outline",
    stroke_width: float = 1.5,
    dark_mode: bool = True,
) -> str:
    """Render *text* as an SVG using glyph outlines from *font_path*."""
    font = TTFont(str(font_path))

    # For variable fonts, pass the weight as a location to getGlyphSet
    location = {"wght": weight} if "fvar" in font else None
    gs = font.getGlyphSet(location=location)
    cmap = font.getBestCmap()
    upem = font["head"].unitsPerEm
    scale = size / upem

    ascender = font["OS/2"].sTypoAscender
    descender = font["OS/2"].sTypoDescender
    height = (ascender - descender) * scale

    # Convert each character to an SVG path
    paths: list[str] = []
    x = 0.0
    for ch in text:
        gname = cmap.get(ord(ch))
        if gname is None:
            x += upem * 0.25
            continue

        pen = SVGPathPen(gs)
        t = TransformPen(pen, (scale, 0, 0, -scale, x * scale, ascender * scale))
        gs[gname].draw(t)
        d = pen.getCommands()
        if d:
            paths.append(d)

        x += gs[gname].width

    w = x * scale

    # Assemble SVG
    lines: list[str] = [
        f'<svg xmlns="http://www.w3.org/2000/svg"'
        f' viewBox="0 0 {w:.1f} {height:.1f}"'
        f' width="{w:.1f}" height="{height:.1f}">',
    ]

    # Style
    if style == "outline":
        base = f"fill: none; stroke: #000; stroke-width: {stroke_width};"
        dark = "stroke: #fff;"
    elif style == "outlined":
        base = f"fill: #fff; stroke: #000; stroke-width: {stroke_width}; paint-order: stroke fill;"
        dark = "fill: #000; stroke: #fff;"
    else:
        base = "fill: #000;"
        dark = "fill: #fff;"

    lines.append("  <style>")
    lines.append(f"    .t {{ {base} }}")
    if dark_mode:
        lines.append("    @media (prefers-color-scheme: dark) {")
        lines.append(f"      .t {{ {dark} }}")
        lines.append("    }")
    lines.append("  </style>")

    for d in paths:
        lines.append(f'  <path class="t" d="{d}"/>')

    lines.append("</svg>")
    return "\n".join(lines)


# -- CLI ----------------------------------------------------------------------


def main() -> None:
    ap = argparse.ArgumentParser(description="Render text to SVG using a font file")
    ap.add_argument("--font-file", type=Path, required=True, help="Local font file path (.ttf/.otf)")
    ap.add_argument("--text", required=True, help="Text to render")
    ap.add_argument("--size", type=float, default=30, help="Font size in pt (default: 30)")
    ap.add_argument("--weight", type=int, default=400, help="Font weight (default: 400)")
    ap.add_argument("--style", choices=["outline", "fill", "outlined"], default="outline")
    ap.add_argument("--stroke-width", type=float, default=1.5, help="Outline stroke width (default: 1.5)")
    ap.add_argument("--dark-mode", action=argparse.BooleanOptionalAction, default=True)
    ap.add_argument("--output", "-o", required=True, help="Output SVG path")
    args = ap.parse_args()

    svg = text_to_svg(
        font_path=args.font_file,
        text=args.text,
        size=args.size,
        weight=args.weight,
        style=args.style,
        stroke_width=args.stroke_width,
        dark_mode=args.dark_mode,
    )

    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(svg, encoding="utf-8")
    print(f"Written → {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
