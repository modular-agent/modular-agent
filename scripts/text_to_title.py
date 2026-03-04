#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# dependencies = ["fonttools"]
# ///
"""Generate a title SVG with project defaults.

Usage:
    uv run scripts/text_to_title.py \
        --font-file path/to/FunnelDisplay-SemiBold.ttf \
        --text "modular-agent-core" \
        -o doc/images/modular_agent_core_title.svg
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Import from sibling module
sys.path.insert(0, str(Path(__file__).parent))
from text_to_svg import text_to_svg


def main() -> None:
    ap = argparse.ArgumentParser(description="Generate a title SVG with project defaults")
    ap.add_argument("--font-file", type=Path, required=True, help="Local font file path (.ttf/.otf)")
    ap.add_argument("--text", required=True, help="Text to render")
    ap.add_argument("--size", type=float, default=30, help="Font size in pt (default: 30)")
    ap.add_argument("--weight", type=int, default=600, help="Font weight (default: 600)")
    ap.add_argument("--stroke-width", type=float, default=1.0, help="Outline stroke width (default: 1.0)")
    ap.add_argument("--output", "-o", required=True, help="Output SVG path")
    args = ap.parse_args()

    svg = text_to_svg(
        font_path=args.font_file,
        text=args.text,
        size=args.size,
        weight=args.weight,
        style="outlined",
        stroke_width=args.stroke_width,
        dark_mode=False,
    )

    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(svg, encoding="utf-8")
    print(f"Written → {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
