# Internal Architecture Overview

This document explains how the `render-slides` system is organized internally across Python entrypoints, Rust core logic, templates, and transport adapters.

## 1) System context

```mermaid
flowchart LR
  A[Python caller\nrender_slides.*] --> B[pyo3 bindings\nsrc/lib.rs]
  B --> C[IR parse + schema validation]
  C --> D[Template manifest\nbuild.rs + templates/layouts]
  D --> E[HTML preview renderer]
  E --> F[PNG rasterization\nhyper_render]
  C --> G[ILM slide model]
  G --> H[PPTX OpenXML emitter]
  B --> I[Transport router\nsrc/transport.rs]
  I --> J[file:// + local paths]
  I --> K[http(s)://]
  I --> L[s3:// (filesystem-backed root)]
```

## 2) Module map and responsibilities

| Area | Primary files | Responsibility |
|---|---|---|
| Python public API surface | `python/render_slides/__init__.py` | Re-export extension symbols and define stable `__all__`. |
| Rust extension entrypoints | `src/lib.rs` | `#[pyfunction]` methods: validate/introspect/render/copy + Python error mapping. |
| Transport routing + adapters | `src/transport.rs` | Scheme detection, handler registry, local/http/s3 read-write streams, byte copy. |
| Template ingestion/build-time metadata | `build.rs`, `templates/layouts/*.slide.jinja` | Parse front matter, generate manifest with editable paths + operation specs + template bodies. |
| Schema contract | `schemas/v1/ir.schema.json` | Structural validation for IR payloads and refinement configuration. |
| Deterministic fixture baselines | `fixtures/parity/*` | Golden HTML, PNG (base64), PPTX (base64) expected outputs. |
| Runtime parity tooling | `scripts/parity_harness.py` | Compare generated artifacts to fixtures with strict/thresholded checks. |

## 3) Data flow walkthrough

### IR parse + validation
1. Python passes `ir_json: str` into Rust API (e.g., `validate`, `render_*`).
2. Rust parses JSON (`serde_json`) and validates against `schemas/v1/ir.schema.json`.
3. Semantic layout validation verifies required slot sets for each layout.
4. Failures are surfaced as `ValueError` with structured path-specific hints.

### Template + layout resolution
1. `build.rs` scans `templates/layouts/*.slide.jinja`.
2. It emits a generated manifest consumed in `src/lib.rs` (`generated::...`).
3. Render paths and operation specs derive from manifest metadata.
4. Runtime lookup maps slide `layout` to the corresponding template body.

### Preview HTML pipeline
1. Parse and validate IR.
2. Resolve theme tokens (defaults + optional overrides).
3. For each slide: map `slots` into HTML template placeholders.
4. Escape unsafe text and emit deterministic HTML document.

### PNG pipeline
1. Build per-slide preview HTML snippets from the same validated IR.
2. Rasterize each snippet with `hyper_render` at 1366x768.
3. Write `slide-XXX.png` files through transport sink routing (local/http/s3).

### PPTX pipeline
1. Map validated slide content into a shared ILM-style intermediate model.
2. Emit deterministic OpenXML package parts (`[Content_Types].xml`, `ppt/...`).
3. Materialize text runs and optional media references (notably `image_focus`).

### Transport copy pipeline
1. `copy_source_to_sink` resolves source and sink handlers by URI scheme.
2. Source opens `Read`, sink opens `Write`.
3. Stream copy occurs with flush/finalization semantics (HTTP uses buffered upload).

## 4) Error model and debugging tips

| Error family | Typical source | Debug tip |
|---|---|---|
| `Invalid JSON` | Input parse failures | Re-run with minimal payload and validate string encoding/quoting first. |
| `ValidationError: ...` | Schema or semantic slot checks | Start at reported JSONPath, verify required slots for the chosen layout. |
| `RenderError: no template registered ...` | Unknown layout/template mismatch | Confirm template exists in `templates/layouts` and regenerate build artifacts. |
| `Unsupported URI scheme` | Transport router | Ensure URI starts with `file://`, `http(s)://`, `s3://`, or registered alias. |
| HTTP flush/status failures | Remote sink rejects PUT/POST | Inspect endpoint behavior; router attempts PUT then POST fallback. |

## 5) Where to change what

| Desired change | Primary location(s) |
|---|---|
| Add or tighten IR validation rule | `schemas/v1/ir.schema.json`, `src/lib.rs` semantic checks |
| Add new slide layout | `templates/layouts/*.slide.jinja`, `build.rs`, fixtures in `fixtures/parity` |
| Add/adjust editable path operation metadata | Layout front matter in templates + manifest generation in `build.rs` |
| Modify HTML output structure/theme tokens | `src/lib.rs` preview rendering helpers |
| Modify PNG dimensions/raster settings | `src/lib.rs` raster config (`hyper_render::Config`) |
| Modify PPTX structure/content mapping | `src/lib.rs` ILM and OpenXML emit helpers |
| Add transport scheme or alter behavior | `src/transport.rs` adapter and registration logic |
| Update parity expectations | `scripts/parity_harness.py` and `fixtures/parity/*` |

## 6) Contributor workflow

1. Run Rust checks:
   - `cargo test`
2. Build/install extension and run Python tests:
   - `./scripts/test-python-build.sh`
3. Validate renderer parity:
   - `python scripts/parity_harness.py --checks html,png,pptx --png-rmse-threshold 0.0 --png-diff-ratio-threshold 0.0 --artifacts-dir artifacts/parity`
4. If rendering changed intentionally, refresh fixtures:
   - `python scripts/parity_harness.py --checks html,png,pptx --update`
5. Regenerate Rustdocs when inline docs changed:
   - `./scripts/generate-docs.sh`

## 7) Future architecture notes

- Current plugin hooks register handler aliases onto built-in adapters; callback-defined external adapters are a logical next extension.
- Documentation quality gates should move into CI to guarantee docs and code evolve together.
