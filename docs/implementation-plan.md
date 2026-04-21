# Rust-backed Python Slide Renderer: Implementation Plan

## 1) Goals and Non-Goals

### Goals
- Build a Python library with performance-critical implementation in Rust.
- Accept an LLM-friendly intermediate representation (IR) for a slideshow.
- Produce either:
  - one PNG per slide (for iterative visual refinement), or
  - one PowerPoint file (`.pptx`) for final output.
- Keep rendered output deterministic and template-driven.

### Non-Goals
- No in-library LLM orchestration, prompting, or agent loops.
- No arbitrary free-form slide composition in v1.
- No full HTML/CSS compatibility in the PowerPoint path.

## 2) Product Constraints and Principles

- **Deterministic rendering:** same IR + assets = same output.
- **Constrained vocabulary:** small set of layouts and style tokens.
- **Round-trip iteration:** PNG rendering should reflect PowerPoint output closely enough for iterative correction.
- **Clear validation:** fail fast with explicit errors and line/field references.
- **Streaming I/O first:** all input/output paths should support chunked/streamed reads and writes (avoid whole-file buffering by default).
- **Pluggable transport layer:** source/destination handlers must be runtime-extensible while shipping built-in local, HTTP, and S3 support.

## 3) High-Level Architecture

### Python API Layer
- User-facing package (`render_slides`) with:
  - `render_pngs(ir, output_target, options)`
  - `render_pptx(ir, output_target, options)`
  - `validate(ir)`
- Input can be Python dicts or JSON/YAML strings.
- Input/output targets can be URIs or adapter objects (e.g., `file://`, `https://`, `s3://`).

### Rust Core (via `pyo3` + `maturin`)
- Core domains:
  - IR parser + schema validation
  - Template/layout resolver
  - Measurement/layout engine
  - Stream I/O abstraction (`Source`/`Sink` traits + async/streaming adapters)
  - Dual emitters:
    - HTML emitter (for PNG snapshots)
    - PPTX emitter (`ppt-rs`)

### Rasterization/PNG Path
- Generate deterministic HTML + CSS from resolved slide model.
- Snapshot HTML to PNG using `hyper_render` (or equivalent headless rendering wrapper).
- Emit one image per slide.
- Write slides through streaming sinks so outputs can go directly to local files, HTTP uploads, or S3 objects.

### PPTX Path
- Convert resolved slide model into standards-compliant OpenXML slide parts.
- Apply equivalent geometry, text styles, and assets (including image embedding and relationships).
- Emit one `.pptx` bundle via a sink writer (local file / HTTP / S3 sink).

### Source/Destination Adapter Layer
- Define a stable transport interface:
  - `Source::open_read(uri, options) -> ReadStream`
  - `Sink::open_write(uri, options) -> WriteStream`
- Built-in adapters in v1:
  1. Local filesystem (`file://` and plain paths)
  2. HTTP(S):
     - Source: `GET`
     - Sink: `PUT` and `POST`
  3. AWS S3 (`s3://bucket/key`) for both read and write
- Runtime extension:
  - Python-side registration hook (e.g., `register_source_handler`, `register_sink_handler`)
  - Rust-side plugin registry used by both render pipelines
- Shared capabilities across adapters:
  - Chunked streaming, retries, deadlines/timeouts, auth config, and structured error mapping.

## 4) IR Design (LLM-Friendly but Constrained)

### IR Principles
- Human- and LLM-writable JSON.
- Low ambiguity field names.
- Strong defaults to minimize token usage.
- Template-based slide declaration.

### Suggested v1 IR Shape
- `meta`: title, author, theme id, slide size.
- `theme`: optional overrides of design tokens.
- `refinement_config`: optional operation schema exposed to LLM callers for iterative edits.
- `slides[]`:
  - `layout`: enum (`title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, `comparison`)
  - `slots`: keyed content (`title`, `subtitle`, `body`, `left`, `right`, `image`, `caption`, etc.)
  - `notes`: optional speaker notes.

### Refinement Config Object (for Qualitative Feedback Loops)
- Add a first-class config object describing:
  - addressable paths (e.g., `slides[2].slots.title`, `slides[1].style.body.font_size`)
  - operation vocabulary per path (e.g., `increase`, `decrease`, `align_left`, `align_center`)
  - operation bounds/step sizes and safety guards.
- Support qualitative aliases that map to concrete operations:
  - `"smaller"` -> `decrease(font_size, step=1)` (or adaptive step)
  - `"larger"` -> `increase(font_size, step=1)`
  - `"left justify"` -> `set_alignment(left)`
- Keep this mapping deterministic and discoverable at runtime.

### Validation Rules
- JSON schema validation + semantic checks.
- Per-layout required/optional slots.
- Max character lengths and line-count heuristics.
- Asset existence and format checks.
- URI scheme validation and adapter capability checks (e.g., sink must support write + multipart/chunking where required).
- LLM-friendly diagnostics as first-class output:
  - include machine-readable error codes, JSONPath/field pointers, and human-readable fixes
  - include "what you sent vs. expected shape" snippets
  - provide concrete retry hints that can be fed directly into the next model turn.
- Validate refinement operations against path/type/operator compatibility and bounds.
- Return operation-level correction hints (e.g., nearest valid path, allowed ops, valid ranges).

## 5) Template System Strategy

### Canonical Intermediate Layout Model (ILM)
- Convert IR to a canonical “resolved slide model” first.
- ILM contains absolute geometry (x/y/w/h), typography tokens, and resolved text runs.
- Both HTML and PPTX emitters consume the same ILM.

### Why This Matters
- Prevents divergent logic between HTML and PowerPoint paths.
- Makes visual parity a data problem (token calibration) rather than two independent renderers.

## 6) Layout and Styling Scope for v1

### Layouts (fixed set)
1. Title slide
2. Title + body
3. Two-column content
4. Section divider
5. Image focus + caption
6. Quote
7. Comparison (left/right)

### Text Features (limited)
- Paragraphs and bullet lists (single nesting level initially).
- Basic inline styles: bold/italic.
- Alignment: left/center/right (layout-dependent defaults).

### Media Features
- Images from local paths or URLs (downloaded/cached before render).
- Fit modes: `contain`, `cover`, `stretch` (if supported by both renderers).

## 7) Parity Calibration Plan (Core Technical Risk)

### Baseline Theme
- Define a single default theme with:
  - font families and fallbacks
  - font sizes and line heights
  - spacing scale
  - color tokens
  - grid margins

### Calibration Harness
- Golden test cases covering each layout with edge-case content.
- Render both outputs:
  1. HTML -> PNG snapshot
  2. PPTX -> exported images (using a deterministic conversion path in CI/manual baseline process)
- Compute visual diffs with thresholds.

### Iteration Loop
1. Adjust geometry/typography token mapping.
2. Re-render golden set.
3. Track diff metrics over time.
4. Lock versions once acceptable parity is reached.

## 8) Python Package + Build/Distribution

- Use `maturin` for building wheels.
- Publish manylinux/macOS/windows wheels for common Python versions.
- Keep Rust internals hidden behind stable Python API.
- Include type hints (`py.typed`) and API docs.
- Publish publicly to PyPI for `pip install` workflows (including source dist + platform wheels).
- Reserve and standardize package naming early (`render-slides` on PyPI, `render_slides` import path).
- Follow semantic versioning and publish changelogs for every release.
- Provide signed release artifacts and provenance/attestations where supported by CI tooling.
- Include classifier metadata (Python versions, OS support, license) and explicit long-description docs.

## 9) Error Handling and Developer Experience

- Structured error model:
  - `ValidationError`
  - `AssetError`
  - `LayoutOverflowWarning` / `LayoutOverflowError`
  - `RenderError`
- Return machine-readable error objects with location paths (e.g., `slides[3].slots.body`).
- Validation failures should expose both:
  - a concise natural-language message for LLM consumption
  - structured metadata (`code`, `path`, `expected`, `actual`, `suggested_fix`, `severity`) for programmatic retry loops.
- Include optional aggregated validation mode that returns all fixable issues in one response (instead of failing at first error) to reduce iteration cycles.
- Optional debug mode:
  - output resolved ILM JSON
  - output generated HTML/CSS
  - draw layout boxes for diagnostics

### Introspection API (Runtime Discoverability)
- Expose runtime-discoverable APIs so an LLM can query what can be changed before suggesting edits:
  - `describe_schema()` -> IR + refinement config schema summary
  - `list_paths(slide_id?)` -> editable object paths
  - `list_operations(path)` -> supported operations, parameters, bounds, examples
  - `explain_operation(path, op)` -> semantics, side effects, and constraints
  - `get_examples(path, op)` -> multi-shot examples of valid requests and expected effect
  - `suggest_changes(context)` -> generated candidate changes based on current deck state.
- Introspection responses should be short, structured, and versioned for stable tool use.
- Include both machine-readable JSON and concise natural-language summaries.

## 10) Performance and Caching

- Cache decoded images and downloaded assets by content hash.
- Cache per-slide HTML where possible for repeated snapshots.
- Parallelize slide PNG rendering across CPU cores.
- Keep memory bounded with streaming asset loads.
- Use backpressure-aware buffered streams to avoid loading large assets/PPTX blobs in memory.
- Reuse pooled HTTP/S3 clients for connection efficiency.

## 11) Security and Safety

- Treat IR as untrusted input.
- Restrict filesystem access to allowed roots when configured.
- Sanitize/validate URLs and optionally disable remote fetch.
- Prevent arbitrary HTML/script injection in template path.
- For HTTP/S3 writes, enforce allowlists, max object sizes, and credential-scoping policies.
- Redact secrets (tokens, signed URLs, credentials) from logs and errors.

## 12) Testing Strategy

### Unit Tests (Rust)
- IR parsing/validation.
- Layout resolver deterministic geometry.
- Text wrapping and overflow behavior.

### Integration Tests
- Python API smoke tests for both output modes.
- Asset handling and error reporting.
- End-to-end streaming tests for all built-in transports:
  - local file read/write
  - HTTP `GET` + `PUT`/`POST`
  - S3 read/write (with mock/localstack in CI where practical)

## 13) Incremental Progress Log

### 2026-04-21
- ✅ Added shared theme token emission in `render_html_preview` (deterministic defaults plus optional IR overrides) so preview output now carries baseline typography/spacing/color tokens.
- ✅ Added first golden parity fixture pair under `fixtures/parity/` for deterministic HTML preview validation.
- ✅ Scaffolded an initial parity harness script (`scripts/parity_harness.py`) with `--update` and check modes for fixture workflows.
- ✅ Expanded parity fixture coverage across all v1 layouts (`title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, `comparison`) and locked them with Python golden tests.
- ✅ Added parity harness CI workflow (`.github/workflows/parity-harness.yml`) and artifact upload plumbing for mismatch diagnostics.
- ✅ Extended parity harness with `--artifacts-dir` output to persist expected/actual HTML and unified diffs on failures.
- ✅ Extended layout-aware semantic validation errors with richer corrective metadata (`expected_required`, `optional`, `provided`, deterministic `suggested_fix`) for retry-loop friendly diagnostics.
- ✅ Replaced `render_pngs` placeholder bytes with `hyper_render` rasterization of rendered slide HTML, emitting real 1366x768 PNGs per slide.
- ✅ Replaced placeholder PPTX payload emission with a deterministic OpenXML package generator that writes valid `.pptx` ZIP parts (`[Content_Types].xml`, relationships, presentation + slide XML, and media parts).
- ✅ Added layout-to-geometry/text mapping for all v1 layouts and `image_focus` image-slot media embedding in PPTX output.
- ✅ Upgraded parity harness to run renderer-backed checks (`--checks html,png,pptx`) and compare golden PNG/PPTX fixtures in addition to HTML previews.
- ✅ Added PNG diff thresholds (`--png-rmse-threshold`, `--png-diff-ratio-threshold`) and per-fixture metrics artifacts for CI debugging.
- ✅ Updated parity CI to execute HTML + PNG + PPTX checks and upload mismatch artifacts by check type.
- ✅ Switched renderer golden fixtures to Base64 text snapshots (`*.render.png.base64`, `*.render.pptx.base64`) to avoid binary-file PR tooling limitations.
- ✅ Added migration helper script (`scripts/migrate_renderer_fixtures_to_base64.py`) to convert accidentally-generated binary renderer fixtures into Base64 text fixtures.
- ✅ Wired an ILM-first shared geometry resolver into both render emitters so `render_pngs` and `render_pptx` consume the same absolute-positioned text/image model.
- ✅ Added runtime transport registration hooks (`register_source_handler`, `register_sink_handler`) with custom scheme alias dispatch onto built-in adapters.
- ⏭️ Next:
  1. Calibrate shared geometry/token mapping to tighten HTML/PPTX visual parity.
  2. Add optional PPTX-to-image export parity mode for visual diffs against PPTX-derived renders.
  3. Expand fixture coverage with more multi-slide and media-heavy decks to harden regression detection.
  4. Extend transport plugins from alias mapping to fully custom callback/provider adapters.

### 2026-04-20
- ✅ Added deterministic template-body consumption in the Rust core via a preview HTML pipeline (`render_html_preview`) with slot substitution and escaping.
- ✅ Extended manifest code generation to include per-layout template bodies so the preview path and future renderers share one canonical template source.
- ✅ Added Rust + Python tests covering HTML preview substitution and escaping behavior.
- ⏭️ Next:
  1. Emit shared theme tokens into preview HTML for closer ILM-aligned defaults.
  2. Add first golden fixtures that can be used by both HTML/PNG and PPTX parity checks.
  3. Scaffold parity harness commands and CI wiring around those fixtures.
- Validation contract tests asserting error payload quality for LLM retry:
  - deterministic error codes and stable field paths
  - actionable `suggested_fix` text
  - aggregated multi-error responses for malformed IR.
- Introspection contract tests:
  - stable path discovery and operation listings
  - schema/version compatibility for tool consumers
  - correctness of qualitative alias mapping (`smaller`, `larger`, `left justify`).
- Suggestion engine tests:
  - `suggest_changes` returns only valid operations for current deck
  - suggested operations adapt to state changes between iterations.
- Packaging/publication tests:
  - wheel + sdist build checks
  - install tests from built artifacts in clean virtualenvs (`pip install dist/*.whl`)
  - import/API smoke tests after install.

### Visual Regression
- Golden PNG snapshots for representative slides.
- Diff thresholds and artifact uploads in CI.

## 13) Milestones

### Milestone 0: Architecture Spike (1-2 weeks)
- Validate `pyo3` + `maturin` packaging.
- Confirm `hyper_render` viability for deterministic snapshots.
- Confirm `ppt-rs` supports required primitives.
- Deliver a single hard-coded layout in both outputs.

### Milestone 1: Minimal v1 Vertical Slice (2-3 weeks)
- IR schema + parser.
- Layouts: `title`, `title_body`, `two_column`.
- Text + image basics.
- Python API and first published pre-release wheel.
- Transport layer with local + HTTP adapters (GET/PUT/POST) and streaming abstractions wired end-to-end.

### Milestone 2: Parity + Robustness (2-4 weeks)
- Add remaining v1 layouts.
- Calibration harness and golden diffs.
- Overflow behavior and clear warnings/errors.
- Add AWS S3 adapter and runtime registration hooks for custom source/sink providers.
- Add introspection APIs and qualitative-operation aliasing for iterative LLM refinement.

### Milestone 3: Production Readiness (2-3 weeks)
- Documentation, examples, and migration notes.
- CI for wheels + regression artifacts.
- Versioned theme/profile support.
- Public release automation:
  - PyPI publish workflow (trusted publishing/OIDC preferred)
  - release notes + changelog generation
  - post-publish install verification across supported platforms.

## 14) Recommended Repository Structure

- `python/render_slides/` – Python package entrypoints and wrappers.
- `rust/core/` – IR, ILM, layout resolver, emitters.
- `rust/core/src/html/` – HTML/CSS emitter.
- `rust/core/src/pptx/` – `ppt-rs` emitter.
- `schemas/` – JSON schema for IR.
- `fixtures/` – golden slides and assets.
- `tests/` – Python integration and regression tests.
- `docs/` – user docs and design notes.
- `examples/` – runnable examples for common `pip`-installed usage patterns.
- `.github/workflows/` – CI for wheels/tests and PyPI publish automation.

## 15) Immediate Next Steps (First 7 Tasks)

1. Initialize Python+Rust project scaffold with `maturin`.
2. Implement `Source`/`Sink` streaming interfaces and URI router with local + HTTP (`GET`/`PUT`/`POST`) adapters.
3. Add AWS S3 adapter for streaming reads/writes.
4. Define JSON schema for v1 IR **plus** refinement-config schema (paths + operation vocabulary + aliases).
5. Implement ILM data structures and conversion from IR.
6. Build introspection endpoints (`list_paths`, `list_operations`, `get_examples`, `suggest_changes`).
7. Implement one layout (`title_body`) in both emitters and add first snapshot/introspection golden tests.


## 13) Implementation Status and Immediate Next Steps

### Status Update (April 21, 2026)
- ✅ Project scaffolding in place for Python + Rust (`maturin` + `pyo3`).
- ✅ JSON Schema validation and refinement/introspection APIs are implemented and tested.
- ✅ Template manifest generation is active at build time.
- ✅ Layout templates now cover the full v1 set: `title`, `title_body`, `two_column`, `section`, `image_focus`, `quote`, `comparison`.
- ✅ Transport router scaffolding (local, HTTP(S), S3-style URI mapping) is implemented with tests.
- ✅ Operation-spec snapshot coverage now locks path + op + params + bounds for the introspection surface.
- ✅ Layout-aware semantic validation now enforces required slot sets for each v1 layout.
- ✅ HTML preview now emits deterministic shared theme tokens and accepts IR theme overrides for baseline styling parity work.
- ✅ First parity fixture + harness plumbing now exists for deterministic preview HTML checks.
- ✅ PNG backend now rasterizes rendered slide HTML into real per-slide PNG images.
- ✅ PPTX backend now emits deterministic OpenXML output with layout text/image placement.
- ✅ ILM-first shared geometry flow now feeds both PNG and PPTX emitters.
- ✅ Python runtime transport registration hooks are available for custom URI scheme alias routing.

### Immediate Next Steps
1. Tune ILM geometry/token calibration to tighten visual parity between HTML-rendered PNGs and PPTX.
2. Extend parity CI with optional PPTX-to-image export/diff flow.
3. Expand plugin registration from alias routing to fully custom provider callbacks.
