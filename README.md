# render-slides

`render-slides` is a Python package with a Rust core for rendering LLM-friendly slideshow IR into PNG slides and PPTX files.

## Current status

This repository currently contains:
- an implementation plan in `docs/implementation-plan.md`
- a project scaffold for Python + Rust packaging via `maturin`

## Local development

```bash
maturin develop
python -c "import render_slides; print(render_slides.describe_schema())"
```
