#!/usr/bin/env python3
"""Generate/check deterministic HTML preview fixtures for parity baselining."""

from __future__ import annotations

import argparse
import difflib
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
    parser.add_argument(
        "--artifacts-dir",
        help=(
            "Optional output directory for mismatch artifacts. "
            "When set, expected/actual HTML and unified diffs are written per fixture."
        ),
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    ir_files = sorted(fixtures_dir.glob("*.ir.json"))
    if not ir_files:
        raise SystemExit(f"No fixture IR files found in {fixtures_dir}")

    mismatches: list[str] = []
    artifacts_dir = Path(args.artifacts_dir) if args.artifacts_dir else None
    if artifacts_dir:
        artifacts_dir.mkdir(parents=True, exist_ok=True)

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
            if artifacts_dir:
                expected_artifact = artifacts_dir / f"{stem}.expected.html"
                actual_artifact = artifacts_dir / f"{stem}.actual.html"
                diff_artifact = artifacts_dir / f"{stem}.diff.txt"

                expected_artifact.write_text(expected_html, encoding="utf-8")
                actual_artifact.write_text(actual_html, encoding="utf-8")
                unified_diff = "\n".join(
                    difflib.unified_diff(
                        expected_html.splitlines(),
                        actual_html.splitlines(),
                        fromfile=f"{stem}.expected.html",
                        tofile=f"{stem}.actual.html",
                        lineterm="",
                    )
                )
                diff_artifact.write_text(f"{unified_diff}\n", encoding="utf-8")

    if mismatches:
        for message in mismatches:
            print(message)
        if artifacts_dir:
            print(f"wrote mismatch artifacts to {artifacts_dir}")
        return 1

    print(f"checked {len(ir_files)} fixture(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
