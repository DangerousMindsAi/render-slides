#!/usr/bin/env python3
"""Generate/check deterministic parity fixtures for HTML, PNG, and PPTX renderer outputs."""

from __future__ import annotations

import argparse
import base64
import difflib
import json
import math
import shutil
import subprocess
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import render_slides


@dataclass(frozen=True)
class PngDiffMetrics:
    width: int
    height: int
    diff_pixels: int
    total_pixels: int
    max_channel_delta: int
    rmse: float

    @property
    def diff_ratio(self) -> float:
        if self.total_pixels == 0:
            return 0.0
        return self.diff_pixels / self.total_pixels


@dataclass(frozen=True)
class HarnessConfig:
    checks: set[str]
    update: bool
    artifacts_dir: Path | None
    png_rmse_threshold: float
    png_diff_ratio_threshold: float


def parse_checks(raw_checks: str) -> set[str]:
    checks = {item.strip().lower() for item in raw_checks.split(",") if item.strip()}
    valid = {"html", "png", "pptx", "pptx_png"}
    unknown = checks - valid
    if unknown:
        raise SystemExit(f"Unknown parity check(s): {', '.join(sorted(unknown))}")
    if not checks:
        raise SystemExit("At least one parity check must be requested")
    return checks


def render_fixture_html(ir_path: Path) -> str:
    ir = json.loads(ir_path.read_text(encoding="utf-8"))
    return render_slides.render_html_preview(json.dumps(ir))


def should_skip_png_check(ir: dict) -> str | None:
    slides = ir.get("slides")
    if not isinstance(slides, list):
        return "IR is missing a slides array"
    for slide in slides:
        if not isinstance(slide, dict):
            continue
        slots = slide.get("slots")
        if not isinstance(slots, dict):
            continue
        image_ref = slots.get("image")
        if not isinstance(image_ref, str):
            continue
        if image_ref.startswith("http://") or image_ref.startswith("https://"):
            continue
        return (
            "image slot uses a non-http(s) source, which is not supported by "
            "the PNG renderer parity path yet"
        )
    return None


def write_artifact(path: Path, payload: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(payload, bytes):
        path.write_bytes(payload)
    else:
        path.write_text(payload, encoding="utf-8")


def decode_png_bytes(raw: bytes, label: str) -> tuple[int, int, bytes]:
    signature = b"\x89PNG\r\n\x1a\n"
    if not raw.startswith(signature):
        raise ValueError(f"Not a PNG file: {label}")

    pos = len(signature)
    width = height = None
    bit_depth = color_type = None
    compressed = bytearray()

    while pos + 8 <= len(raw):
        length = int.from_bytes(raw[pos : pos + 4], "big")
        chunk_type = raw[pos + 4 : pos + 8]
        chunk_data_start = pos + 8
        chunk_data_end = chunk_data_start + length
        chunk_data = raw[chunk_data_start:chunk_data_end]
        pos = chunk_data_end + 4  # skip crc

        if chunk_type == b"IHDR":
            width = int.from_bytes(chunk_data[0:4], "big")
            height = int.from_bytes(chunk_data[4:8], "big")
            bit_depth = chunk_data[8]
            color_type = chunk_data[9]
            interlace = chunk_data[12]
            if bit_depth != 8:
                raise ValueError(f"Unsupported PNG bit depth {bit_depth} in {label}")
            if interlace != 0:
                raise ValueError(f"Unsupported interlaced PNG in {label}")
            if color_type not in {2, 6}:
                raise ValueError(f"Unsupported PNG color type {color_type} in {label}")
        elif chunk_type == b"IDAT":
            compressed.extend(chunk_data)
        elif chunk_type == b"IEND":
            break

    if width is None or height is None or color_type is None:
        raise ValueError(f"Malformed PNG missing IHDR: {label}")

    bytes_per_px = 4 if color_type == 6 else 3
    stride = width * bytes_per_px
    decompressed = zlib.decompress(bytes(compressed))

    expected_size = (stride + 1) * height
    if len(decompressed) != expected_size:
        raise ValueError(
            f"Unexpected PNG payload size in {label}: got {len(decompressed)} expected {expected_size}"
        )

    rows = []
    prev = bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_type = decompressed[cursor]
        cursor += 1
        row = bytearray(decompressed[cursor : cursor + stride])
        cursor += stride

        if filter_type == 0:
            pass
        elif filter_type == 1:
            for i in range(stride):
                left = row[i - bytes_per_px] if i >= bytes_per_px else 0
                row[i] = (row[i] + left) & 0xFF
        elif filter_type == 2:
            for i in range(stride):
                row[i] = (row[i] + prev[i]) & 0xFF
        elif filter_type == 3:
            for i in range(stride):
                left = row[i - bytes_per_px] if i >= bytes_per_px else 0
                up = prev[i]
                row[i] = (row[i] + ((left + up) // 2)) & 0xFF
        elif filter_type == 4:
            for i in range(stride):
                a = row[i - bytes_per_px] if i >= bytes_per_px else 0
                b = prev[i]
                c = prev[i - bytes_per_px] if i >= bytes_per_px else 0
                p = a + b - c
                pa = abs(p - a)
                pb = abs(p - b)
                pc = abs(p - c)
                if pa <= pb and pa <= pc:
                    pr = a
                elif pb <= pc:
                    pr = b
                else:
                    pr = c
                row[i] = (row[i] + pr) & 0xFF
        else:
            raise ValueError(f"Unsupported PNG filter type {filter_type} in {label}")

        rows.append(bytes(row))
        prev = row

    if color_type == 6:
        return width, height, b"".join(rows)

    # RGB -> RGBA
    rgba = bytearray(width * height * 4)
    src = 0
    dst = 0
    data = b"".join(rows)
    while src < len(data):
        rgba[dst] = data[src]
        rgba[dst + 1] = data[src + 1]
        rgba[dst + 2] = data[src + 2]
        rgba[dst + 3] = 255
        src += 3
        dst += 4
    return width, height, bytes(rgba)


def diff_png(expected_bytes: bytes, actual_bytes: bytes) -> PngDiffMetrics:
    ew, eh, expected = decode_png_bytes(expected_bytes, "expected")
    aw, ah, actual = decode_png_bytes(actual_bytes, "actual")
    if (ew, eh) != (aw, ah):
        raise ValueError(f"PNG dimension mismatch expected {ew}x{eh}, got {aw}x{ah}")

    diff_pixels = 0
    max_channel_delta = 0
    total_squared_error = 0
    for i in range(0, len(expected), 4):
        channels_differ = False
        for channel in range(4):
            delta = abs(expected[i + channel] - actual[i + channel])
            total_squared_error += delta * delta
            if delta:
                channels_differ = True
            if delta > max_channel_delta:
                max_channel_delta = delta
        if channels_differ:
            diff_pixels += 1

    channel_count = len(expected)
    rmse = math.sqrt(total_squared_error / channel_count) if channel_count else 0.0
    return PngDiffMetrics(
        width=ew,
        height=eh,
        diff_pixels=diff_pixels,
        total_pixels=ew * eh,
        max_channel_delta=max_channel_delta,
        rmse=rmse,
    )


def compare_html(stem: str, ir_path: Path, fixtures_dir: Path, config: HarnessConfig) -> list[str]:
    mismatches: list[str] = []
    expected_path = fixtures_dir / f"{stem}.preview.html"
    actual_html = render_fixture_html(ir_path)

    if config.update:
        expected_path.write_text(actual_html, encoding="utf-8")
        print(f"updated {expected_path}")
        return mismatches

    if not expected_path.exists():
        return [f"missing expected HTML fixture: {expected_path}"]

    expected_html = expected_path.read_text(encoding="utf-8")
    if expected_html != actual_html:
        mismatches.append(f"html mismatch: {expected_path}")
        if config.artifacts_dir:
            write_artifact(config.artifacts_dir / f"html/{stem}.expected.html", expected_html)
            write_artifact(config.artifacts_dir / f"html/{stem}.actual.html", actual_html)
            unified_diff = "\n".join(
                difflib.unified_diff(
                    expected_html.splitlines(),
                    actual_html.splitlines(),
                    fromfile=f"{stem}.expected.html",
                    tofile=f"{stem}.actual.html",
                    lineterm="",
                )
            )
            write_artifact(config.artifacts_dir / f"html/{stem}.diff.txt", f"{unified_diff}\n")
    return mismatches


def compare_png(stem: str, ir_json: str, fixtures_dir: Path, config: HarnessConfig) -> list[str]:
    mismatches: list[str] = []
    expected_path = fixtures_dir / f"{stem}.render.png.base64"

    with tempfile.TemporaryDirectory(prefix=f"parity-{stem}-png-") as tmp:
        out_dir = Path(tmp) / "png"
        render_slides.render_pngs(ir_json, str(out_dir))
        actual_path = out_dir / "slide-001.png"
        actual_bytes = actual_path.read_bytes()
        if config.update:
            expected_path.write_text(base64.b64encode(actual_bytes).decode("ascii") + "\n", encoding="utf-8")
            print(f"updated {expected_path}")
            return mismatches

        if not expected_path.exists():
            return [f"missing expected PNG fixture: {expected_path}"]

        expected_bytes = base64.b64decode(expected_path.read_text(encoding="utf-8"))
        metrics = diff_png(expected_bytes, actual_bytes)

    over_rmse = metrics.rmse > config.png_rmse_threshold
    over_ratio = metrics.diff_ratio > config.png_diff_ratio_threshold
    if over_rmse or over_ratio:
        mismatches.append(
            "png mismatch: "
            f"{expected_path} "
            f"(rmse={metrics.rmse:.4f} threshold={config.png_rmse_threshold:.4f}, "
            f"diff_ratio={metrics.diff_ratio:.6f} threshold={config.png_diff_ratio_threshold:.6f}, "
            f"diff_pixels={metrics.diff_pixels}/{metrics.total_pixels}, "
            f"max_delta={metrics.max_channel_delta})"
        )
        if config.artifacts_dir:
            write_artifact(config.artifacts_dir / f"png/{stem}.expected.png", expected_bytes)
            write_artifact(config.artifacts_dir / f"png/{stem}.actual.png", actual_bytes)
            write_artifact(
                config.artifacts_dir / f"png/{stem}.metrics.json",
                json.dumps(
                    {
                        "width": metrics.width,
                        "height": metrics.height,
                        "diff_pixels": metrics.diff_pixels,
                        "total_pixels": metrics.total_pixels,
                        "diff_ratio": metrics.diff_ratio,
                        "max_channel_delta": metrics.max_channel_delta,
                        "rmse": metrics.rmse,
                        "rmse_threshold": config.png_rmse_threshold,
                        "diff_ratio_threshold": config.png_diff_ratio_threshold,
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
            )
    return mismatches


def compare_pptx(stem: str, ir_json: str, fixtures_dir: Path, config: HarnessConfig) -> list[str]:
    mismatches: list[str] = []
    expected_path = fixtures_dir / f"{stem}.render.pptx.base64"

    with tempfile.TemporaryDirectory(prefix=f"parity-{stem}-pptx-") as tmp:
        actual_path = Path(tmp) / "deck.pptx"
        render_slides.render_pptx(ir_json, str(actual_path))
        actual_bytes = actual_path.read_bytes()

    if config.update:
        expected_path.write_text(base64.b64encode(actual_bytes).decode("ascii") + "\n", encoding="utf-8")
        print(f"updated {expected_path}")
        return mismatches

    if not expected_path.exists():
        return [f"missing expected PPTX fixture: {expected_path}"]

    expected_bytes = base64.b64decode(expected_path.read_text(encoding="utf-8"))
    if expected_bytes != actual_bytes:
        mismatches.append(f"pptx mismatch: {expected_path}")
        if config.artifacts_dir:
            write_artifact(config.artifacts_dir / f"pptx/{stem}.expected.pptx", expected_bytes)
            write_artifact(config.artifacts_dir / f"pptx/{stem}.actual.pptx", actual_bytes)
    return mismatches


def convert_pptx_to_pngs(pptx_path: Path, output_dir: Path) -> list[Path]:
    soffice = shutil.which("soffice")
    if soffice is None:
        return []

    output_dir.mkdir(parents=True, exist_ok=True)
    command = [
        soffice,
        "--headless",
        "--convert-to",
        "png",
        "--outdir",
        str(output_dir),
        str(pptx_path),
    ]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise RuntimeError(
            "PPTX-to-PNG conversion failed via soffice: "
            f"exit={result.returncode}, stderr={result.stderr.strip()}"
        )

    deck_stem = pptx_path.stem
    converted = sorted(output_dir.glob(f"{deck_stem}*.png"))
    return converted


def compare_pptx_png(
    stem: str,
    ir_json: str,
    fixtures_dir: Path,
    config: HarnessConfig,
) -> list[str]:
    mismatches: list[str] = []
    expected_png_path = fixtures_dir / f"{stem}.render.png.base64"

    if not expected_png_path.exists():
        return [
            "missing expected PNG fixture required for pptx_png check: "
            f"{expected_png_path}"
        ]

    expected_png = base64.b64decode(expected_png_path.read_text(encoding="utf-8"))

    with tempfile.TemporaryDirectory(prefix=f"parity-{stem}-pptx-png-") as tmp:
        tmp_path = Path(tmp)
        pptx_path = tmp_path / "deck.pptx"
        render_slides.render_pptx(ir_json, str(pptx_path))

        converted_pngs = convert_pptx_to_pngs(pptx_path, tmp_path / "png")
        if not converted_pngs:
            print(
                "skipped pptx_png check for "
                f"{stem}: libreoffice/soffice not available in environment"
            )
            return mismatches

        actual_png = converted_pngs[0].read_bytes()
        metrics = diff_png(expected_png, actual_png)

    over_rmse = metrics.rmse > config.png_rmse_threshold
    over_ratio = metrics.diff_ratio > config.png_diff_ratio_threshold
    if over_rmse or over_ratio:
        mismatches.append(
            "pptx_png mismatch: "
            f"{stem} "
            f"(rmse={metrics.rmse:.4f} threshold={config.png_rmse_threshold:.4f}, "
            f"diff_ratio={metrics.diff_ratio:.6f} threshold={config.png_diff_ratio_threshold:.6f}, "
            f"diff_pixels={metrics.diff_pixels}/{metrics.total_pixels}, "
            f"max_delta={metrics.max_channel_delta})"
        )
        if config.artifacts_dir:
            write_artifact(config.artifacts_dir / f"pptx_png/{stem}.expected.png", expected_png)
            write_artifact(config.artifacts_dir / f"pptx_png/{stem}.actual.png", actual_png)
            write_artifact(
                config.artifacts_dir / f"pptx_png/{stem}.metrics.json",
                json.dumps(
                    {
                        "width": metrics.width,
                        "height": metrics.height,
                        "diff_pixels": metrics.diff_pixels,
                        "total_pixels": metrics.total_pixels,
                        "diff_ratio": metrics.diff_ratio,
                        "max_channel_delta": metrics.max_channel_delta,
                        "rmse": metrics.rmse,
                        "rmse_threshold": config.png_rmse_threshold,
                        "diff_ratio_threshold": config.png_diff_ratio_threshold,
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
            )
    return mismatches


def run_checks(ir_files: Iterable[Path], fixtures_dir: Path, config: HarnessConfig) -> list[str]:
    mismatches: list[str] = []
    for ir_path in ir_files:
        stem = ir_path.name.removesuffix(".ir.json")
        ir_json = ir_path.read_text(encoding="utf-8")
        ir = json.loads(ir_json)

        if "html" in config.checks:
            mismatches.extend(compare_html(stem, ir_path, fixtures_dir, config))
        png_skip_reason = should_skip_png_check(ir)
        if "png" in config.checks:
            if png_skip_reason:
                print(f"skipped png check for {stem}: {png_skip_reason}")
            else:
                mismatches.extend(compare_png(stem, ir_json, fixtures_dir, config))
        if "pptx" in config.checks:
            mismatches.extend(compare_pptx(stem, ir_json, fixtures_dir, config))
        if "pptx_png" in config.checks:
            mismatches.extend(compare_pptx_png(stem, ir_json, fixtures_dir, config))

    return mismatches


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixtures-dir",
        default="fixtures/parity",
        help="Directory containing *.ir.json and parity fixture files.",
    )
    parser.add_argument(
        "--checks",
        default="html,png,pptx",
        help="Comma-separated checks to run: html,png,pptx,pptx_png",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Rewrite expected parity fixtures from current renderer outputs.",
    )
    parser.add_argument(
        "--artifacts-dir",
        help=(
            "Optional output directory for mismatch artifacts. "
            "When set, expected/actual outputs and diffs are written per check."
        ),
    )
    parser.add_argument(
        "--png-rmse-threshold",
        type=float,
        default=0.0,
        help="Maximum allowed PNG RMSE (0-255 scale).",
    )
    parser.add_argument(
        "--png-diff-ratio-threshold",
        type=float,
        default=0.0,
        help="Maximum allowed ratio of differing PNG pixels (0-1).",
    )
    args = parser.parse_args()

    fixtures_dir = Path(args.fixtures_dir)
    ir_files = sorted(fixtures_dir.glob("*.ir.json"))
    if not ir_files:
        raise SystemExit(f"No fixture IR files found in {fixtures_dir}")

    artifacts_dir = Path(args.artifacts_dir) if args.artifacts_dir else None
    if artifacts_dir:
        artifacts_dir.mkdir(parents=True, exist_ok=True)

    config = HarnessConfig(
        checks=parse_checks(args.checks),
        update=args.update,
        artifacts_dir=artifacts_dir,
        png_rmse_threshold=args.png_rmse_threshold,
        png_diff_ratio_threshold=args.png_diff_ratio_threshold,
    )

    mismatches = run_checks(ir_files, fixtures_dir, config)

    if mismatches:
        for message in mismatches:
            print(message)
        if artifacts_dir:
            print(f"wrote mismatch artifacts to {artifacts_dir}")
        return 1

    print(
        f"checked {len(ir_files)} fixture(s) with checks: {', '.join(sorted(config.checks))}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
