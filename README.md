# render-slides

`render-slides` is a Python package with a Rust core for rendering LLM-friendly slideshow IR into PNG slides and PPTX files.

## Current status

This repository currently contains:
- an implementation plan in `docs/implementation-plan.md`
- Python + Rust (`maturin`/`pyo3`) project scaffolding
- initial IR validation, schema summary, and introspection APIs (`validate`, `describe_schema`, `list_paths`, `list_operations`, `explain_operation`, `get_examples`)
- compile-time template manifest generation from `.slide.jinja` files with YAML front matter
- migrated layout templates for `title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, and `comparison` with metadata-derived refinement paths
- JSON Schema definition for the v1 IR (including `refinement_config` paths/operations/aliases) at `schemas/v1/ir.schema.json`
- transport layer scaffolding for local files, HTTP(S), and AWS S3 URIs
- a Python copy helper API (`copy_source_to_sink`) backed by the Rust transport router
- a deterministic HTML preview API (`render_html_preview`) that consumes layout template bodies and materializes slide slot values
- preview HTML theme-token emission with deterministic CSS custom properties (default tokens + optional IR theme overrides)
- layout-aware validation errors with required/optional/provided slot summaries and deterministic `suggested_fix` guidance
- deterministic renderer entrypoint scaffolding for artifact output:
  - `render_pngs` now rasterizes HTML slide snapshots into real 1366x768 PNG files (one per slide) using `hyper_render` (Chromium-free)
  - `render_pptx` now emits a real standards-compliant OpenXML `.pptx` package with deterministic slide/text mapping and `image_focus` media embedding
- expanded parity fixtures + harness checks across all v1 layouts at `fixtures/parity/` and `scripts/parity_harness.py`
- Rust and Python test coverage for validation, transport behaviors, and manifest/introspection path stability checks
- a one-command build/test script at `scripts/test-python-build.sh`
- a one-command Rustdoc generation script at `scripts/generate-docs.sh`
- CI parity harness workflow with mismatch artifact uploads at `.github/workflows/parity-harness.yml`
- renderer-backed parity fixtures and thresholds for HTML preview + PNG + PPTX outputs via `scripts/parity_harness.py`

## Prerequisites

Before building locally, install:
- Python 3.9+ (including 3.13)
- Rust toolchain (`rustup`, including `cargo`)
- `pip`

## Step-by-step: build and test

### 1) Clone and enter the repo

```bash
git clone <your-fork-or-repo-url>
cd render-slides
```

### 2) Create and activate a virtual environment

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
```

### 3) Install development tools

```bash
python -m pip install -r requirements-dev.txt
```

### 4) Run Rust unit tests

```bash
cargo test
```

### 5) Build the Python wheel from Rust extension code

```bash
python -m maturin build --release -o dist
```

### 6) Install the built wheel

```bash
python -m pip install --force-reinstall dist/render_slides-*.whl
```

### 7) Run Python tests

```bash
pytest -q
```

### 8) Quick sanity check

```bash
python -c "import render_slides; print(render_slides.describe_schema())"
python -c "print(__import__('render_slides').validate('{\"slides\": []}'))"
```


## One-command Python build + test

If you just want to rebuild the extension and run Python tests, use:

```bash
./scripts/test-python-build.sh
```

This script will create `.venv` if needed, install dev dependencies, build a fresh wheel with `maturin`, install it, and run `pytest -q`.

## Optional: fast edit/build loop

If you prefer an in-place development install, run:

```bash
python -m maturin develop
```

This requires an active virtual environment.

## Generate Rust documentation

To generate project Rustdocs (including private items), run:

```bash
./scripts/generate-docs.sh
```

The generated docs entry point will be:

```text
target/doc/render_slides/index.html
```

## Run the parity harness

Validate golden fixtures (HTML + renderer artifacts) locally:

```bash
python scripts/parity_harness.py \
  --checks html,png,pptx \
  --png-rmse-threshold 0.0 \
  --png-diff-ratio-threshold 0.0 \
  --artifacts-dir artifacts/parity
```

Refresh fixtures after intentional rendering changes:

```bash
python scripts/parity_harness.py --checks html,png,pptx --update
```

> Note: renderer golden fixtures are stored as text-safe Base64 files (`*.render.png.base64`, `*.render.pptx.base64`) so PR tooling that rejects binary files can still create PRs cleanly.
>
> If someone accidentally generates binary renderer fixtures, run:
>
> ```bash
> python scripts/migrate_renderer_fixtures_to_base64.py
> ```

## Remaining gaps

- PPTX output currently uses deterministic layout mapping for v1 templates; full ILM-shared geometry parity with HTML output is still in progress.
- Runtime-extensible Python registration hooks (`register_source_handler`, `register_sink_handler`) are still planned but not yet exposed.
- ILM-first dual-emitter architecture (shared absolute geometry consumed by both HTML and PPTX emitters) remains to be implemented.

## Implementation plan status

- ✅ Template manifest migration now covers the full v1 layout set (`title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, `comparison`).
- ✅ Introspection path coverage now includes `section` and `image_focus` slot paths via tests.
- ✅ Operation-spec snapshot testing now locks path + operation + params + bounds contracts for introspection.
- ✅ Layout-aware semantic validation now enforces required slot combinations per layout before render-time.
- ✅ Validation errors now include layout-specific required/optional/provided slot details with deterministic `suggested_fix` hints.
- ✅ Template bodies are now consumed by a deterministic HTML preview pipeline (`render_html_preview`) with HTML escaping and slot substitution.
- ✅ HTML preview now emits shared theme tokens (with deterministic defaults and optional IR theme overrides).
- ✅ Golden parity fixtures now cover all v1 layouts with deterministic preview snapshots (`fixtures/parity`, `scripts/parity_harness.py`).
- ✅ Parity harness now validates HTML + renderer-backed PNG/PPTX outputs with configurable PNG diff thresholds and CI artifact uploads.
- ✅ `render_pngs` now emits real HTML-to-image slide PNG snapshots (1366x768) via `hyper_render` instead of placeholder 1x1 bytes.
- ✅ Renderer entrypoint APIs now emit deterministic output artifacts (`render_pngs`, `render_pptx`) rather than raising `NotImplementedError`.
- ⏭️ Next: calibrate geometry tokens for tighter HTML/PPTX visual parity and add optional PPTX-to-image parity export checks in CI.
