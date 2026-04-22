# Callpath Walkthroughs

This document traces each major exported API route end-to-end and maps walkthrough examples (`W1..W6`) to tests and subsystems.

## W1 — `validate(ir_json)`

### Example payload
```json
{"slides":[{"layout":"title_body","slots":{"title":"Hello","body":"World"}}]}
```

### Callpath
1. Python caller invokes `render_slides.validate(...)`.
2. pyo3 binding in `src/lib.rs` parses JSON to `serde_json::Value`.
3. JSON Schema validator checks structural rules.
4. Semantic validator checks required slot set for `title_body`.
5. Function returns `"ok"`.

### Output
- String: `"ok"`.

### Common failure mode
- Missing required slot, e.g. no `body` for `title_body`.
- Debug by inspecting reported path and suggested fix in returned `ValidationError` text.

---

## W2 — `render_html_preview(ir_json)`

### Example payload
```json
{
  "theme": {"colors": {"background": "#101820"}},
  "slides": [
    {"layout":"title_body","slots":{"title":"Quarterly Update","body":"Highlights"}}
  ]
}
```

### Callpath
1. Python API forwards JSON string into Rust `render_html_preview`.
2. IR parse + validation run first.
3. Template registry resolves `title_body` layout body.
4. Slot placeholders (`{{ slide.slots.* }}`) are replaced with escaped slot values.
5. Theme token block is emitted and merged into final HTML document.

### Output
- Deterministic HTML document string.

### Common failure mode
- Unknown layout template (`RenderError: no template registered ...`).
- Debug by checking layout name and generated template manifest source.

---

## W3 — `render_pngs(ir_json, output_uri)`

### Example payload
```json
{"slides":[{"layout":"title","slots":{"title":"A","subtitle":"B"}}]}
```
with output `"./out"`.

### Callpath
1. API validates IR and iterates slides.
2. Each slide is converted into HTML snapshot content.
3. `hyper_render` rasterizes HTML at fixed geometry (1366x768).
4. Sink URI is resolved (`./out/slide-001.png`).
5. Bytes are written through transport sink adapter.

### Output
- `slide-001.png` (and additional files for more slides).

### Common failure mode
- Unsupported output URI scheme.
- Debug by switching to local path/file URI and confirming scheme support.

---

## W4 — `render_pptx(ir_json, output_uri)`

### Example payload
```json
{"slides":[{"layout":"title","slots":{"title":"A","subtitle":"B"}}]}
```
with output `"./deck.pptx"`.

### Callpath
1. API parses and validates IR.
2. Slides are mapped into ILM-style text/image geometry structures.
3. OpenXML parts are generated (`[Content_Types].xml`, `ppt/presentation.xml`, `ppt/slides/slideN.xml`, etc.).
4. Zip package is finalized.
5. Bytes are written to destination through sink router.

### Output
- Standards-compliant `.pptx` package.

### Common failure mode
- Write failure to output destination.
- Debug with local filesystem output first, then expand to remote sink.

---

## W5 — `copy_source_to_sink(source_uri, sink_uri)`

### Example payload
- `source_uri`: `"/tmp/source.txt"`
- `sink_uri`: `"/tmp/destination.txt"`

### Callpath
1. Source and sink URIs are routed by scheme.
2. Source adapter opens a `Read` stream.
3. Sink adapter opens a `Write` stream.
4. Stream copy executes and sink flushes/finalizes.

### Output
- String: `"ok"`.

### Common failure mode
- HTTP sink returns non-success status.
- Debug by capturing endpoint responses and confirming PUT/POST behavior.

---

## W6 — Introspection flow

### Example sequence
1. `list_paths()`
2. `list_operations("slides[*].style.alignment")`
3. `explain_operation("slides[*].style.alignment", "set_alignment")`
4. `get_examples("slides[*].style.alignment", "set_alignment")`

### Callpath
1. Path list is served from generated manifest + deduped set.
2. Operation list is filtered by selected path.
3. Explanation wraps operation metadata into structured narrative fields.
4. Examples return operation-specific request/effect snippets.

### Output
- JSON arrays/objects for discoverability and assistive UX.

### Common failure mode
- Unsupported path-operation combination.
- Debug by validating path against `list_paths` output first.

---

## Walkthrough coverage matrix

| ID | APIs exercised | Core modules/functions touched | Primary tests | Fixture linkage |
|---|---|---|---|---|
| W1 | `validate` | `src/lib.rs`: `validate`, `validate_ir`, schema + slot checks | `tests/test_api.py::test_validate_*` | N/A |
| W2 | `render_html_preview` | `src/lib.rs`: preview render pipeline, template registry, theme tokens | `tests/test_api.py::test_render_html_preview_*` | `fixtures/parity/*.preview.html` |
| W3 | `render_pngs` | `src/lib.rs`: rasterization + sink URI handling; `src/transport.rs` sink open/write | `tests/test_api.py::test_render_pngs_writes_one_file_per_slide` | `fixtures/parity/*.render.png.base64` |
| W4 | `render_pptx` | `src/lib.rs`: ILM mapping + OpenXML zip emit + sink write | `tests/test_api.py::test_render_pptx_writes_openxml_package` | `fixtures/parity/*.render.pptx.base64` |
| W5 | `copy_source_to_sink` | `src/lib.rs`: API wrapper; `src/transport.rs`: routing + adapters + copy | `tests/test_api.py::test_copy_source_to_sink_*` | N/A |
| W6 | `list_paths`, `list_operations`, `explain_operation`, `get_examples` | `src/lib.rs`: manifest-backed introspection helpers | `tests/test_api.py::test_list_*`, `test_explain_operation_*`, `test_get_examples_*` | Template manifest generated from `templates/layouts/*.slide.jinja` |

## Coverage gaps and follow-ups

- Gap G1: no dedicated doctest/snippet harness currently validates this document’s examples directly.
- Gap G2: CI workflow does not yet run `pytest -q` + rustdoc/link checks in a docs-quality gate.
- Gap G3: branch-level coverage report is not currently published with walkthough ID annotations.
