# Documentation Coverage Plan

## Objective
Ship a complete, maintainable documentation set that supports contributors, library users, and API integrators, with clear automation paths and explicit coverage goals.

## Scope (Minimum Required)
1. **Detailed function documentation** generated automatically from Rust inline docs (`///` and module-level `//!` docs).
2. **Internal code overview** that explains architecture, module responsibilities, and key data flows.
3. **External API documentation** for the Python package (`render_slides`).
4. **Callpath walkthroughs** for major API calls, with executable examples that collectively drive full code coverage.

---

## 1) Detailed Function Documentation (Rustdoc-first)

### Deliverables
- Rustdoc pages for all public functions and core internal helpers relevant to renderer behavior.
- A docs quality gate that fails CI when public items miss rustdoc comments.
- A generated docs artifact published per commit/tag.

### Implementation plan
1. Add/expand inline docs in Rust source:
   - Public Python-exposed Rust functions (`#[pyfunction]` wrappers).
   - Core validation, template, render, and transport functions.
   - Module-level architecture summaries (`//!`) per module.
2. Keep examples in doc comments for key entry points where feasible.
3. Generate docs with existing script:
   - `./scripts/generate-docs.sh`
4. Add CI checks:
   - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items`
   - (Optional strictness) `#![deny(missing_docs)]` once baseline is complete.

### Acceptance criteria
- 100% of public Rust items in this crate have rustdoc comments.
- `scripts/generate-docs.sh` completes successfully in CI.
- Rust docs are discoverable from a single index link in `README.md`.

---

## 2) Internal Code Overview

### Deliverables
Create `docs/internal-architecture.md` with:
- System context (Python API -> Rust core -> outputs).
- Module map:
  - `src/lib.rs`
  - `src/transport.rs`
  - `templates/` and generated manifest flow (`build.rs`)
  - schema and fixtures directories.
- Data model walkthrough:
  - IR parse/validation
  - layout/template resolution
  - preview/render pipelines (HTML, PNG, PPTX)
- Error model and debugging tips.

### Implementation plan
1. Add one high-level architecture diagram (ASCII or Mermaid).
2. Include "Where to change what" table:
   - validation rules
   - path introspection
   - transport behavior
   - rendering output logic
3. Add contributor flow:
   - how to run tests
   - parity harness usage
   - fixture update expectations.

### Acceptance criteria
- New contributors can identify where to make changes for each subsystem in <10 minutes.
- Internal architecture doc links directly to concrete files and scripts.

---

## 3) External API Documentation (Python)

### Deliverables
Create `docs/python-api.md` (or Sphinx/MkDocs equivalent) documenting:
- Public API from `python/render_slides/__init__.py`:
  - `validate`
  - `describe_schema`
  - `list_paths`
  - `list_operations`
  - `explain_operation`
  - `get_examples`
  - `copy_source_to_sink`
  - `register_source_handler`
  - `register_sink_handler`
  - `render_html_preview`
  - `render_pngs`
  - `render_pptx`
- For each API:
  - signature
  - input contract
  - return shape
  - exceptions/failure modes
  - minimal and advanced examples.

### Implementation plan
1. Add Python docstrings for public wrappers where missing.
2. Autogenerate API reference using a docs toolchain (recommended: Sphinx autodoc or MkDocs + mkdocstrings).
3. Add a "quick recipes" section:
   - validate + render preview
   - render PNGs
   - render PPTX
   - transport copy.
4. Publish docs with version markers aligned to package releases.

### Acceptance criteria
- Every symbol exported in `__all__` appears in generated API docs.
- All examples are runnable in CI (doctest or snippet test harness).

---

## 4) Full Callpath Walkthroughs + Coverage-driven Examples

### Deliverables
Create `docs/callpath-walkthroughs.md` with step-by-step traces for each major API route:
1. `validate(ir_json)`
2. `render_html_preview(ir_json)`
3. `render_pngs(ir_json, output_uri)`
4. `render_pptx(ir_json, output_uri)`
5. `copy_source_to_sink(source_uri, sink_uri)`
6. Introspection flow (`list_paths` -> `list_operations` -> `explain_operation` -> `get_examples`)

Each walkthrough should include:
- Example request payload.
- Function boundary transitions (Python -> Rust -> helper functions -> output).
- Expected output artifact(s).
- Common failure mode + how to debug.

### Coverage strategy
Build a **walkthrough coverage matrix** mapping examples to code areas and tests:
- Example IDs (`W1`..`Wn`)
- APIs exercised
- Rust functions/modules touched
- Python tests that validate behavior
- parity fixture linkage (when applicable).

Target matrix outcome:
- Combined walkthrough examples correspond to full line/branch coverage goals for critical paths.
- Any uncovered paths get explicit "gap" entries with planned follow-up tests.

### Acceptance criteria
- Walkthrough docs include enough examples to explain all exported APIs.
- Coverage report can be traced from docs example ID -> test file -> touched subsystem.

---

## Execution Phases

### Phase 1: Baseline inventory (1-2 days)
- Audit current docs and inline comments.
- Produce "missing docs" checklist for Rust public items + Python exported API.

### Phase 2: Authoring + automation (2-4 days)
- Fill rustdoc gaps.
- Create `internal-architecture.md`, `python-api.md`, `callpath-walkthroughs.md`.
- Add docs build/check commands to CI.

### Phase 3: Coverage alignment (2-3 days)
- Create walkthrough coverage matrix.
- Add/adjust tests for uncovered branches.
- Validate matrix against coverage report.

### Phase 4: Publish + maintain (ongoing)
- Add docs ownership in CODEOWNERS or review checklist.
- Require docs updates for public API/behavior changes.
- Version docs alongside releases.

---

## Required CI checks (documentation quality gate)
- `./scripts/generate-docs.sh`
- `cargo test`
- `pytest -q`
- docs link check (tooling choice: `lychee` or equivalent)
- (Optional) docs spell/style checks.

---

## Definition of Done
Documentation is complete when:
1. Rust function-level documentation is auto-generated and warning-clean.
2. Internal architecture documentation exists and is accurate to current code.
3. Python external API documentation covers all exported public APIs with runnable examples.
4. Callpath walkthroughs exist for every major API and include a coverage matrix tied to tests.
