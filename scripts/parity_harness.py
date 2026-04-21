#!/usr/bin/env python3
"""Generate/check deterministic HTML preview fixtures for parity baselining."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import render_slides


def render_fixture(ir_path: Path) -> str:
    ir = json.loads(ir_path.read_text(encoding="utf-8"))
    return render_slides.render_html_preview(json.dumps(ir))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixtures-dir",
        default="fixtures/parity",
        help="Directory containing *.ir.json and *.preview.html fixture pairs.",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Write rendered HTML into *.preview.html files.",
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    ir_files = sorted(fixtures_dir.glob("*.ir.json"))
    if not ir_files:
        raise SystemExit(f"No fixture IR files found in {fixtures_dir}")

    mismatches: list[str] = []

    for ir_path in ir_files:
        stem = ir_path.name.removesuffix(".ir.json")
        expected_path = fixtures_dir / f"{stem}.preview.html"
        actual_html = render_fixture(ir_path)

        if args.update:
            expected_path.write_text(actual_html, encoding="utf-8")
            print(f"updated {expected_path}")
            continue

        if not expected_path.exists():
            mismatches.append(f"missing expected fixture: {expected_path}")
            continue

        expected_html = expected_path.read_text(encoding="utf-8")
        if expected_html != actual_html:
            mismatches.append(f"mismatch: {expected_path}")

    if mismatches:
        for message in mismatches:
            print(message)
        return 1

    print(f"checked {len(ir_files)} fixture(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
