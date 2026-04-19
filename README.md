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
- Rust and Python test coverage for validation, transport behaviors, and manifest/introspection path stability checks
- a one-command build/test script at `scripts/test-python-build.sh`
- a one-command Rustdoc generation script at `scripts/generate-docs.sh`

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

## Next steps

- Expand snapshot-oriented tests from paths to full operation-spec surface (path + op + params + bounds) to lock introspection contracts.
- Begin wiring template body usage into upcoming HTML/PNG rendering path while preserving deterministic output.
- Add layout-aware semantic validation to enforce required slot sets per layout before render-time.

## Implementation plan status

- ✅ Template manifest migration now covers the full v1 layout set (`title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, `comparison`).
- ✅ Introspection path coverage now includes `section` and `image_focus` slot paths via tests.
- ⏭️ Next: implement operation-spec snapshot testing and start consuming template bodies in the renderer pipeline.
