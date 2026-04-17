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

## PR workflow (to avoid unexpected merge conflicts)

If you are opening multiple PRs in sequence, use a **new branch per PR** from updated `main`.

### Recommended flow

```bash
# 1) Ensure local main includes latest merged work
git checkout main
git pull --ff-only origin main

# 2) Create a fresh branch for the next PR
git checkout -b feature/<short-topic>

# 3) Make changes, commit, push, open PR
git add .
git commit -m "Your change"
git push -u origin feature/<short-topic>
```

### If you already made changes on an older branch

Rebase that branch onto latest `main` before opening/updating the PR:

```bash
git fetch origin
git rebase origin/main
```

If conflicts still appear, it usually means the PR branch still contains commits from an already-merged PR (or a different base branch). In that case, create a fresh branch from `origin/main` and cherry-pick only the new commit(s).
