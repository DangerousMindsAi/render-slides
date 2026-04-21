#!/usr/bin/env python3
"""Migrate renderer parity fixtures from binary files to Base64 text snapshots."""

from __future__ import annotations

import argparse
import base64
from pathlib import Path


SUPPORTED_SUFFIXES = (".render.png", ".render.pptx")


def candidate_files(fixtures_dir: Path) -> list[Path]:
    candidates: list[Path] = []
    for suffix in SUPPORTED_SUFFIXES:
        candidates.extend(sorted(fixtures_dir.glob(f"*{suffix}")))
    return sorted(candidates)


def migrate_file(path: Path, *, delete_original: bool, dry_run: bool) -> tuple[Path, Path, bool]:
    target = path.with_name(path.name + ".base64")
    should_write = (not target.exists()) or (target.read_text(encoding="utf-8").strip() != base64.b64encode(path.read_bytes()).decode("ascii"))

    if not dry_run and should_write:
        encoded = base64.b64encode(path.read_bytes()).decode("ascii") + "\n"
        target.write_text(encoded, encoding="utf-8")

    deleted = False
    if delete_original and not dry_run:
        path.unlink()
        deleted = True

    return path, target, deleted


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixtures-dir",
        default="fixtures/parity",
        help="Directory containing renderer parity fixture files.",
    )
    parser.add_argument(
        "--keep-originals",
        action="store_true",
        help="Do not delete original binary files after writing Base64 fixtures.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show planned migrations without writing/deleting files.",
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    files = candidate_files(fixtures_dir)
    if not files:
        print(f"No binary renderer fixtures found in {fixtures_dir}")
        return 0

    migrated = 0
    for path in files:
        source, target, deleted = migrate_file(
            path,
            delete_original=not args.keep_originals,
            dry_run=args.dry_run,
        )
        action = "would migrate" if args.dry_run else "migrated"
        delete_note = " (source deleted)" if deleted else ""
        print(f"{action} {source} -> {target}{delete_note}")
        migrated += 1

    print(f"processed {migrated} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
