# Template System Refactor Proposal

## Problem statement

We want to replace hardcoded Rust layout strings with HTML-like templates while preserving:

1. **Inline slot definitions** (e.g., title/body/left/right in template markup).
2. **Compile-time guarantees** (template errors should fail `cargo build`).
3. **Existing introspection UX** (we must continue exposing editable paths, operations, and user-facing explanatory strings).

## What we researched

We compared libraries that are realistic for this crate and our constraints:

- Askama (`askama`)
- Rinja (`rinja`)
- Sailfish (`sailfish`)
- MiniJinja (`minijinja`)
- Tera (`tera`)
- Maud (`maud`)

### Selection criteria

- HTML-like authoring model.
- Compile-time parsing/codegen.
- Maturity and docs quality.
- How easily we can build an **introspection manifest** from templates.

## Comparison

| Library | HTML-like templates | Compile-time behavior | Notes for introspection | Fit |
|---|---|---|---|---|
| **Askama** | Yes (Jinja-like files or inline `source`) | `derive(Template)` generates Rust at compile time; supports `extends`/`block`/`include` | Strong fit; can pair with metadata extraction in `build.rs` | **Best fit** |
| **Rinja** | Yes (Jinja-like) | Compile-time generation and stable Rust support | Also good; largely same architecture options as Askama | Good fallback |
| **Sailfish** | Template syntax is EJS-like rather than Jinja/HTML-first | Compiles templates, good performance | Introspection feasible, but syntax is less aligned with desired authoring style | Medium |
| **MiniJinja** | Yes | Primarily runtime environment + runtime template registration | Good dynamic features, weaker compile-time guarantee for our requirement | Low |
| **Tera** | Yes | Primarily runtime; `compile_templates!` can pre-validate at startup/build stage | Introspection possible but less strict compile-time integration | Low |
| **Maud** | HTML-like Rust macro DSL | Compile-time macro expansion | Great type-safety, but template authoring is Rust DSL, not external HTML-like templates | Medium/Low |

## Recommendation

Use **Askama + single-file template metadata with YAML front matter**.

### Why this structure

- It keeps visual template and introspection metadata together in one reviewable artifact.
- It avoids drift between `*.html` and `*.yml` files.
- It still allows strict compile-time checks by splitting/parsing in `build.rs` before Askama compiles templates.

### File format convention

Use one file per layout, with extension `*.slide.jinja`, for example `templates/layouts/title_body.slide.jinja`:

```text
---
layout: title_body
editable_paths:
  - path: slides[*].slots.title
    operation: set_text
    description: Replaces text content for the selected slot.
    params: [text]
    bounds: text length must be <= 2000 characters
  - path: slides[*].slots.body
    operation: set_text
    description: Replaces text content for the selected slot.
    params: [text]
    bounds: text length must be <= 2000 characters
---
<section class="slide layout-title-body" data-layout="title_body">
  <h1 data-slot="title">{{ slide.slots.title }}</h1>
  <p data-slot="body">{{ slide.slots.body }}</p>
</section>
```

- First `--- ... ---` block is YAML metadata.
- Remaining content is Askama-compatible HTML/Jinja template.
- `data-slot` markers define editable slot endpoints.

## Proposed architecture

```text
templates/
  layouts/
    title.slide.jinja
    title_body.slide.jinja
    two_column.slide.jinja

build.rs
  -> read .slide.jinja files
  -> split YAML front matter + template body
  -> validate metadata vs data-slot markers
  -> write generated askama templates to $OUT_DIR/templates/*.html
  -> generate src/generated/template_manifest.rs

src/templates.rs
  -> #[derive(Template)] + #[template(path = "...")] using generated files
```

### Build-time validations

`build.rs` should fail fast if:

- YAML front matter is missing or invalid.
- A `data-slot` exists in HTML but no metadata entry exists.
- Metadata references a non-existent slot.
- Unsupported path prefixes are used.
- Required operation fields are missing.

This preserves the introspection contract while moving rendering markup out of hardcoded Rust strings.

### Practical fallback

If Askama path handling with generated files proves awkward in practice, fallback to **side-by-side files in one directory**:

```text
templates/layouts/title_body.html
templates/layouts/title_body.yml
```

The generator/validator logic remains the same; only file-loading changes.

### Editor ergonomics for single-file `.slide.jinja`

You can get good editor behavior with single-file templates, and `*.slide.jinja` helps by default:

- **VS Code**: if needed, map `*.slide.jinja` to a Jinja mode with `files.associations`, and keep YAML front matter at top.  
  Example:
  ```json
  {
    "files.associations": {
      "*.slide.jinja": "jinja-html"
    }
  }
  ```
- **JetBrains IDEs**: register `*.slide.jinja` as a Jinja2 file type in *Settings → Editor → File Types* if auto-detection does not kick in.
- **Neovim/tree-sitter**: set `*.slide.jinja` filetype to `jinja`/`html` and add front-matter injection rules if desired.

**Important tradeoff:** generic file association usually gives great highlighting for the HTML/Jinja body, but YAML front matter may be plain text unless the editor supports mixed-language injection.

Our chosen compromise is extension **`*.slide.jinja`** so editors default to Jinja-aware highlighting while we still parse YAML front matter in `build.rs`.

## Migration plan

1. **Introduce Askama scaffolding**
   - Add `askama` dependency.
   - Add one `title_body` template + typed render context.
2. **Add `.slide.jinja` parser + generator**
   - Add `build.rs` producing generated HTML templates and `src/generated/template_manifest.rs`.
   - Wire `list_paths` and `list_operations` to generated constants.
3. **Move current hardcoded strings into `.slide.jinja` metadata blocks**
   - Keep output JSON identical to current API during migration.
4. **Incrementally migrate all layouts**
   - `title`, `two_column`, `section`, `image_focus`, `quote`, `comparison`.
5. **Add snapshot tests**
   - Assert generated manifest and introspection responses.

## Risks and mitigations

- **Risk: custom front-matter parser complexity.**  
  Mitigation: keep parser minimal and deterministic (`---` delimiter, strict schema, explicit errors).

- **Risk: compile times rise.**  
  Mitigation: keep template generation lightweight; isolate in dedicated module/crate if needed.

- **Risk: future requirement for runtime template loading.**  
  Mitigation: if this becomes a hard requirement, reassess MiniJinja/Tera then.

## Open questions

1. Should we standardize on `.slide.jinja` only, or also allow `.html` with front matter?
2. Should we expose manifest versioning in `describe_schema()` (e.g., `template_manifest_version`)?
3. Should we allow per-layout custom operations beyond today’s global set?

## Proposed next implementation task

If you agree, next step should be:

- Implement a **single-layout vertical slice** (`title_body`) using a `.slide.jinja` file with YAML front matter,
- Keep Python API output backward-compatible,
- Add tests proving compile-time and introspection behavior remain intact.

## External references consulted

- Askama creating templates: https://askama.rs/en/latest/creating_templates.html
- Askama template syntax: https://askama.readthedocs.io/en/latest/template_syntax.html
- Rinja introduction: https://rinja.readthedocs.io/en/stable/
- Maud docs and book entrypoint: https://docs.rs/maud/latest/maud/
- Sailfish repository and user guide links: https://github.com/rust-sailfish/sailfish
- MiniJinja docs: https://docs.rs/minijinja/latest/minijinja/
- Tera docs: https://keats.github.io/tera/docs/
