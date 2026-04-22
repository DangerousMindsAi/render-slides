# Python API Reference (`render_slides`)

This document covers every symbol exported by `python/render_slides/__init__.py::__all__`.

> All APIs return JSON as strings (or side-effect writes for artifact/copy operations) and raise `ValueError` for invalid input or failed operations.

## Quick recipes

### Validate then render HTML preview

```python
import json
import render_slides

ir = {
    "slides": [
        {"layout": "title_body", "slots": {"title": "Q2 Review", "body": "Highlights"}}
    ]
}

render_slides.validate(json.dumps(ir))
html = render_slides.render_html_preview(json.dumps(ir))
print(html[:120])
```

### Render PNG slides

```python
import json
import render_slides

ir = {"slides": [{"layout": "title", "slots": {"title": "Roadmap", "subtitle": "2026"}}]}
render_slides.render_pngs(json.dumps(ir), "./out")
# writes ./out/slide-001.png
```

### Render PPTX deck

```python
import json
import render_slides

ir = {"slides": [{"layout": "title", "slots": {"title": "Launch", "subtitle": "Plan"}}]}
render_slides.render_pptx(json.dumps(ir), "./deck.pptx")
```

### Transport copy (including custom aliases)

```python
import render_slides

render_slides.register_source_handler("customsrc", "file")
render_slides.register_sink_handler("customsink", "file")
render_slides.copy_source_to_sink("customsrc:///tmp/in.txt", "customsink:///tmp/out.txt")
```

---

## API details

## `validate(ir_json: str) -> str`
Validate an IR payload against schema + semantic layout slot rules.

- **Input contract**: `ir_json` must be valid JSON with required IR fields (notably `slides`).
- **Returns**: string literal `"ok"`.
- **Failure modes**:
  - `ValueError("Invalid JSON: ...")`
  - `ValueError("ValidationError: ...")`
- **Minimal example**:
  ```python
  import render_slides
  assert render_slides.validate('{"slides": []}') == "ok"
  ```
- **Advanced example**: include `refinement_config` paths/ops and layout slot values.

## `describe_schema() -> str`
Return a pretty-printed JSON summary of supported schema facets.

- **Input contract**: none.
- **Returns**: JSON string containing `version`, `slide_layouts`, `qualitative_aliases`.
- **Failure modes**: serialization failure (rare) as `ValueError`.
- **Minimal example**:
  ```python
  import json, render_slides
  schema = json.loads(render_slides.describe_schema())
  print(schema["slide_layouts"])
  ```

## `list_paths(slide_id: int | None = None) -> str`
List editable IR paths available for operations.

- **Input contract**:
  - `slide_id=None` returns wildcard paths (e.g., `slides[*].slots.title`).
  - `slide_id=n` rewrites wildcard to `slides[n]`.
- **Returns**: pretty JSON list of path strings.
- **Failure modes**: serialization failure as `ValueError`.

## `list_operations(path: str) -> str`
List allowed operations for a specific editable path.

- **Input contract**: `path` must match a supported manifest path.
- **Returns**: pretty JSON list of operation objects (`name`, `description`, `params`, `bounds`).
- **Failure modes**:
  - `ValueError("Unsupported editable path: ...")`

## `explain_operation(path: str, operation: str) -> str`
Explain semantics, side effects, and constraints for a path-operation pair.

- **Input contract**: valid path + operation combination.
- **Returns**: JSON object with `path`, `operation`, `semantics`, `side_effects`, `constraints`.
- **Failure modes**:
  - Unsupported path
  - Unsupported operation for path

## `get_examples(path: str, operation: str) -> str`
Return request/effect examples for a supported path-operation pair.

- **Input contract**: valid path and operation.
- **Returns**: JSON list with objects containing `request` and `effect`.
- **Failure modes**:
  - Unsupported path
  - Unsupported operation

## `copy_source_to_sink(source_uri: str, sink_uri: str) -> str`
Copy bytes between transport endpoints.

- **Input contract**:
  - `source_uri`/`sink_uri` can be local path, `file://`, `http(s)://`, `s3://`, or registered alias.
- **Returns**: `"ok"` when copy completes.
- **Failure modes**:
  - Unsupported scheme
  - Invalid URI/path
  - HTTP non-success upload/download
  - Filesystem I/O issues
- **Advanced example**: alias `customsrc://` and `customsink://` to `file` handlers.

## `register_source_handler(alias_scheme: str, target_scheme: str) -> str`
Register a source alias to an existing built-in source scheme.

- **Input contract**:
  - `alias_scheme` is the new scheme name.
  - `target_scheme` must already exist (for example `file`, `http`, `https`, `s3`).
- **Returns**: `"ok"`.
- **Failure modes**: unknown target scheme or invalid alias.

## `register_sink_handler(alias_scheme: str, target_scheme: str) -> str`
Register a sink alias to an existing built-in sink scheme.

- **Input contract** mirrors `register_source_handler`.
- **Returns**: `"ok"`.
- **Failure modes**: unknown target scheme or invalid alias.

## `render_html_preview(ir_json: str) -> str`
Render deterministic preview HTML from IR slides + theme tokens.

- **Input contract**: valid IR JSON with known `layout` values and required slots.
- **Returns**: full HTML document string.
- **Failure modes**:
  - Invalid JSON
  - Validation errors (schema/semantic)
  - Missing template registration
- **Advanced example**: include `theme.typography` and `theme.colors` overrides.

## `render_pngs(ir_json: str, output_uri: str) -> str`
Render one PNG file per slide to target output location.

- **Input contract**:
  - Valid IR JSON.
  - `output_uri` can be directory path or URI (file/http/s3).
- **Returns**: `"ok"`.
- **Artifacts**: `slide-001.png`, `slide-002.png`, ...
- **Failure modes**:
  - Validation/render errors
  - Unsupported output target scheme
  - Transport write failures

## `render_pptx(ir_json: str, output_uri: str) -> str`
Render a deterministic OpenXML `.pptx` package to target output.

- **Input contract**: valid IR JSON and writable destination URI/path.
- **Returns**: `"ok"`.
- **Artifacts**: `.pptx` zip package with presentation + slide parts.
- **Failure modes**:
  - Validation/render failures
  - Packaging/serialization errors
  - Transport write failures

---

## Versioning note

When package versions are cut, this file should be tagged/referenced in release notes so users can pair API behavior to the exact shipped version.
