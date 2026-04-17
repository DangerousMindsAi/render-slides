# render-slides

`render-slides` is a Python package with a Rust core for rendering LLM-friendly slideshow IR into PNG slides and PPTX files.

## Current status

This repository currently contains:
- an implementation plan in `docs/implementation-plan.md`
- a project scaffold for Python + Rust packaging via `maturin`

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
python -c "import render_slides; print(render_slides.validate_detailed('{\"slides\": [{\"layout\": \"title_body\", \"slots\": {\"title\": \"Hello\"}}]}'))"
```

## Optional: fast edit/build loop

If you prefer an in-place development install, run:

```bash
python -m maturin develop
```

This requires an active virtual environment.
