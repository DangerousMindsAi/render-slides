# Documentation Inventory Checklist

_Last updated: 2026-04-21._

This checklist tracks what exists today versus what is still needed to satisfy `docs/documentation-plan.md`.

## Rust public-item docs inventory

### Status summary
- Module-level docs exist for the crate root and transport module.
- Python-exposed entrypoints have rustdoc summaries.
- Internal helper functions are partially documented.
- A strict missing-docs gate (`#![deny(missing_docs)]`) is **not** enabled.

### Public API checklist (`src/py_api.rs` / `src/transport.rs`)

| Item | Doc status | Notes |
|---|---|---|
| `validate` | ✅ Documented | Includes purpose and return contract. |
| `describe_schema` | ✅ Documented | Includes summary behavior. |
| `list_paths` | ✅ Documented | Includes wildcard + slide-scoped behavior. |
| `list_operations` | ✅ Documented | Includes path contract. |
| `explain_operation` | ✅ Documented | Includes semantics/constraints behavior. |
| `get_examples` | ✅ Documented | Includes request/effect examples behavior. |
| `copy_source_to_sink` | ✅ Documented | Includes cross-transport copy behavior. |
| `register_source_handler` | ✅ Documented | Includes aliasing intent. |
| `register_sink_handler` | ✅ Documented | Includes aliasing intent. |
| `render_html_preview` | ✅ Documented | Includes deterministic preview description. |
| `render_pngs` | ✅ Documented | Includes one-file-per-slide behavior. |
| `render_pptx` | ✅ Documented | Includes OpenXML package behavior. |
| `TransportError` and variants | ✅ Documented | Variant-level docs present. |
| `Source` / `Sink` traits | ✅ Documented | Method-level docs present. |
| `LocalAdapter` / `HttpAdapter` / `S3Adapter` | ✅ Documented | Struct + method docs present. |
| `default_source_handlers` / `default_sink_handlers` | ✅ Documented | Function docs present. |
| `register_source_handler` / `register_sink_handler` (Rust transport layer) | ✅ Documented | Function docs present. |
| `open_source` / `open_sink` / `copy_uri_to_uri` | ✅ Documented | Routing/copy behavior documented. |

### Remaining rustdoc gap tasks
- [ ] Add runnable doc examples to selected public APIs (`validate`, `render_html_preview`, `copy_source_to_sink`).
- [ ] Decide if `#![deny(missing_docs)]` should be enabled now or after additional internal stabilization.

## Python API docs inventory

Export source: `python/render_slides/__init__.py::__all__`.

| Exported symbol | API reference doc | Example coverage |
|---|---|---|
| `validate` | ✅ Planned in `docs/python-api.md` | ✅ Minimal + advanced |
| `describe_schema` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `list_paths` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `list_operations` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `explain_operation` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `get_examples` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `copy_source_to_sink` | ✅ Planned in `docs/python-api.md` | ✅ Minimal + advanced |
| `register_source_handler` | ✅ Planned in `docs/python-api.md` | ✅ Advanced |
| `register_sink_handler` | ✅ Planned in `docs/python-api.md` | ✅ Advanced |
| `render_html_preview` | ✅ Planned in `docs/python-api.md` | ✅ Minimal + advanced |
| `render_pngs` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |
| `render_pptx` | ✅ Planned in `docs/python-api.md` | ✅ Minimal |

Remaining tasks:
- [ ] Add Python wrapper docstrings (currently import-only re-exports in `__init__.py`; behavior docs live in generated extension API).
- [ ] Pick/autowire a docs generator (`Sphinx` or `MkDocs`) so this reference can be rendered and versioned automatically.

## CI/documentation automation inventory

| Check | Current status | Gap |
|---|---|---|
| `./scripts/generate-docs.sh` | ⚠️ Script exists but not in CI workflow | Add a docs job or step. |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items` | ❌ Not wired in CI | Add dedicated rustdoc gate. |
| `cargo test` | ⚠️ Used locally, not explicit in workflow | Add to CI docs quality gate (or central test workflow). |
| `pytest -q` | ⚠️ Used locally, parity harness workflow installs package and runs parity only | Add Python unit test step. |
| docs link checker (`lychee` or equivalent) | ❌ Not present | Add docs link check step. |
| runnable snippet/doctest harness for docs examples | ❌ Not present | Add doc example execution path. |

## Recommended next actions
1. Add a dedicated `docs-quality` workflow that runs rustdoc, tests, and link checks.
2. Choose docs toolchain (`Sphinx` preferred for autodoc of Python symbols) and scaffold build/publish.
3. Add a short `docs/README.md` index that links architecture/API/callpath pages.
