//! Core Rust implementation for the `render_slides` Python package.
//!
//! This crate currently exposes a small Python API surface for:
//! - validating slideshow IR JSON payloads,
//! - describing the currently supported schema summary,
//! - introspecting editable IR paths and operations, and
//! - copying bytes across local or HTTP(S) transports.
//!
//! Rendering entry points are intentionally scaffolded and return
//! `NotImplementedError` until rendering backends are integrated.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use jsonschema::{error::ValidationErrorKind, Draft, Validator};
use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use serde::Serialize;
use serde_json::Value;

pub mod transport;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/template_manifest.rs"));
}

const IR_SCHEMA_JSON: &str = include_str!("../schemas/v1/ir.schema.json");

static IR_VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(IR_SCHEMA_JSON)
        .map_err(|err| format!("Schema compile error: invalid JSON schema: {err}"))?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .map_err(|err| format!("Schema compile error: {err}"))
});

#[derive(Serialize)]
struct SchemaSummary {
    version: &'static str,
    slide_layouts: Vec<&'static str>,
    qualitative_aliases: Vec<&'static str>,
}

#[derive(Serialize)]
struct OperationSpec {
    name: &'static str,
    description: &'static str,
    params: Vec<&'static str>,
    bounds: &'static str,
}

#[derive(Serialize)]
struct OperationExplanation {
    path: String,
    operation: String,
    semantics: &'static str,
    side_effects: Vec<&'static str>,
    constraints: Vec<&'static str>,
}

#[derive(Serialize)]
struct OperationExample {
    request: &'static str,
    effect: &'static str,
}

#[derive(Clone)]
struct SlideTemplate {
    body: &'static str,
    slot_names: Vec<String>,
}

fn all_editable_paths() -> Vec<&'static str> {
    let mut unique = BTreeSet::new();
    unique.extend(generated::TEMPLATE_EDITABLE_PATHS.iter().copied());
    unique.into_iter().collect()
}

fn supports_path(path: &str) -> bool {
    all_editable_paths().contains(&path)
}

fn operation_specs_for(path: &str) -> Option<Vec<OperationSpec>> {
    let from_template: Vec<_> = generated::TEMPLATE_OPERATION_SPECS
        .iter()
        .filter(|entry| entry.path == path)
        .map(|entry| OperationSpec {
            name: entry.name,
            description: entry.description,
            params: entry.params.to_vec(),
            bounds: entry.bounds,
        })
        .collect();

    if from_template.is_empty() {
        return None;
    }

    Some(from_template)
}

fn template_registry() -> BTreeMap<&'static str, SlideTemplate> {
    generated::TEMPLATE_DEFINITIONS
        .iter()
        .map(|entry| {
            (
                entry.layout,
                SlideTemplate {
                    body: entry.body,
                    slot_names: collect_slot_names(entry.body),
                },
            )
        })
        .collect()
}

fn collect_slot_names(template_body: &str) -> Vec<String> {
    let mut slot_names = BTreeSet::new();
    let mut cursor = template_body;
    let needle = "data-slot=\"";

    while let Some(start_idx) = cursor.find(needle) {
        let after = &cursor[start_idx + needle.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        slot_names.insert(after[..end_idx].to_string());
        cursor = &after[end_idx + 1..];
    }

    slot_names.into_iter().collect()
}

/// Validates the minimal contract for a render-slides IR payload.
fn validate_ir(parsed: &Value) -> Result<(), String> {
    let validator = IR_VALIDATOR.as_ref().map_err(ToString::to_string)?;
    let mut errors = validator.iter_errors(parsed);
    if let Some(first) = errors.next() {
        return Err(format_validation_error(first));
    }

    validate_layout_required_slots(parsed)?;

    Ok(())
}

fn validate_layout_required_slots(parsed: &Value) -> Result<(), String> {
    let Some(slides) = parsed.get("slides").and_then(Value::as_array) else {
        return Ok(());
    };

    for (index, slide) in slides.iter().enumerate() {
        let Some(layout) = slide.get("layout").and_then(Value::as_str) else {
            continue;
        };

        let required_slots: &[&str] = match layout {
            "title" => &["title", "subtitle"],
            "title_body" => &["title", "body"],
            "two_column" => &["title", "left", "right"],
            "section" => &["title", "subtitle"],
            "image_focus" => &["title", "image", "caption"],
            "quote" => &["quote", "attribution"],
            "comparison" => &["title", "left", "right"],
            _ => continue,
        };

        let Some(slots) = slide.get("slots").and_then(Value::as_object) else {
            continue;
        };

        for required_slot in required_slots {
            if !slots.contains_key(*required_slot) {
                return Err(format!(
                    "ValidationError: missing required slot '{required_slot}' for layout '{layout}' at $.slides[{index}].slots."
                ));
            }
        }
    }

    Ok(())
}

fn format_validation_error(error: jsonschema::ValidationError<'_>) -> String {
    let instance_path = error.instance_path().to_string();
    let path = if instance_path.is_empty() {
        "$".to_string()
    } else {
        format!("$.{instance_path}")
    };

    let hint = match error.kind() {
        ValidationErrorKind::Required { property } => {
            format!("missing required field '{property}'")
        }
        ValidationErrorKind::Type { kind } => format!("expected type {kind:?}"),
        ValidationErrorKind::Enum { .. } => {
            "value must be one of the allowed enum values".to_string()
        }
        _ => error.to_string(),
    };

    format!("ValidationError: {hint} at {path}.")
}

/// Builds a small, human-readable summary of the supported schema surface.
fn schema_summary() -> SchemaSummary {
    SchemaSummary {
        version: "0.1",
        slide_layouts: vec![
            "title",
            "title_body",
            "two_column",
            "section",
            "image_focus",
            "quote",
            "comparison",
        ],
        qualitative_aliases: vec!["smaller", "larger", "left justify"],
    }
}

#[pyfunction]
/// Validates an IR JSON document and returns `"ok"` when it is accepted.
fn validate(ir_json: &str) -> PyResult<String> {
    let parsed: Value = serde_json::from_str(ir_json)
        .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {e}")))?;

    validate_ir(&parsed).map_err(PyValueError::new_err)?;

    Ok("ok".to_string())
}

#[pyfunction]
/// Returns a pretty-printed JSON summary of schema version, layouts, and aliases.
fn describe_schema() -> PyResult<String> {
    let summary = schema_summary();

    serde_json::to_string_pretty(&summary)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize schema summary: {e}")))
}

#[pyfunction(signature = (slide_id=None))]
/// Lists editable IR paths for refinement operations.
fn list_paths(slide_id: Option<usize>) -> PyResult<String> {
    let mut paths: Vec<String> = all_editable_paths()
        .into_iter()
        .map(ToString::to_string)
        .collect();

    if let Some(id) = slide_id {
        paths = paths
            .into_iter()
            .map(|path| path.replacen("slides[*]", &format!("slides[{id}]"), 1))
            .collect();
    }

    serde_json::to_string_pretty(&paths)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize path listing: {e}")))
}

#[pyfunction]
/// Lists operations supported for a specific editable path.
fn list_operations(path: &str) -> PyResult<String> {
    let operations = operation_specs_for(path)
        .ok_or_else(|| PyValueError::new_err(format!("Unsupported editable path: {path}")))?;

    serde_json::to_string_pretty(&operations)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize operation listing: {e}")))
}

#[pyfunction]
/// Explains semantics and constraints for one path + operation pair.
fn explain_operation(path: &str, operation: &str) -> PyResult<String> {
    let operations = operation_specs_for(path)
        .ok_or_else(|| PyValueError::new_err(format!("Unsupported editable path: {path}")))?;

    let op = operations
        .into_iter()
        .find(|op| op.name == operation)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "Unsupported operation '{operation}' for path '{path}'"
            ))
        })?;

    let explanation = OperationExplanation {
        path: path.to_string(),
        operation: operation.to_string(),
        semantics: op.description,
        side_effects: vec![
            "May trigger text reflow inside the resolved layout box.",
            "May require overflow checks before render emitters run.",
        ],
        constraints: vec![op.bounds],
    };

    serde_json::to_string_pretty(&explanation).map_err(|e| {
        PyValueError::new_err(format!("Failed to serialize operation explanation: {e}"))
    })
}

#[pyfunction]
/// Returns examples of valid operation requests and expected effects.
fn get_examples(path: &str, operation: &str) -> PyResult<String> {
    if !supports_path(path) {
        return Err(PyValueError::new_err(format!(
            "Unsupported editable path: {path}"
        )));
    }

    let supported_operations = operation_specs_for(path)
        .ok_or_else(|| PyValueError::new_err(format!("Unsupported editable path: {path}")))?;

    if !supported_operations.iter().any(|op| op.name == operation) {
        return Err(PyValueError::new_err(format!(
            "Unsupported operation '{operation}' for path '{path}'"
        )));
    }

    let examples = match operation {
        "increase" => vec![OperationExample {
            request: r#"{"path":"slides[1].style.body.font_size","op":"increase","step":1}"#,
            effect:
                "Increases body font size for slide 1 by one point, clamped to configured bounds.",
        }],
        "decrease" => vec![OperationExample {
            request: r#"{"path":"slides[1].style.body.font_size","op":"decrease","step":2}"#,
            effect:
                "Decreases body font size for slide 1 by two points, clamped to configured bounds.",
        }],
        "set_alignment" => vec![OperationExample {
            request: r#"{"path":"slides[0].style.alignment","op":"set_alignment","alignment":"left"}"#,
            effect: "Aligns text in the targeted style scope to left alignment.",
        }],
        "set_text" => vec![OperationExample {
            request: r#"{"path":"slides[2].slots.title","op":"set_text","text":"Q3 Rollout Update"}"#,
            effect: "Replaces the target slot text with the provided string.",
        }],
        "set_layout" => vec![OperationExample {
            request: r#"{"path":"slides[2].layout","op":"set_layout","layout":"comparison"}"#,
            effect: "Changes slide layout and triggers layout-specific required-slot checks.",
        }],
        _ => {
            return Err(PyValueError::new_err(format!(
                "Unsupported operation '{operation}' for path '{path}'"
            )));
        }
    };

    serde_json::to_string_pretty(&examples)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize operation examples: {e}")))
}

fn normalize_slot_text(slot_value: Option<&Value>) -> String {
    let Some(value) = slot_value else {
        return String::new();
    };

    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn default_theme_tokens() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("font-family-base", "'Inter', 'Segoe UI', sans-serif"),
        ("font-size-title", "48px"),
        ("font-size-body", "28px"),
        ("line-height-title", "1.15"),
        ("line-height-body", "1.35"),
        ("space-slide-padding", "48px"),
        ("color-bg", "#ffffff"),
        ("color-text-primary", "#111111"),
        ("color-text-muted", "#4f4f4f"),
    ])
}

fn resolve_theme_token_overrides(theme: Option<&serde_json::Map<String, Value>>) -> BTreeMap<String, String> {
    let mut tokens: BTreeMap<String, String> = default_theme_tokens()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    let Some(theme_obj) = theme else {
        return tokens;
    };

    let flat_overrides = [
        ("font_family_base", "font-family-base"),
        ("font_size_title", "font-size-title"),
        ("font_size_body", "font-size-body"),
        ("line_height_title", "line-height-title"),
        ("line_height_body", "line-height-body"),
        ("space_slide_padding", "space-slide-padding"),
        ("color_bg", "color-bg"),
        ("color_text_primary", "color-text-primary"),
        ("color_text_muted", "color-text-muted"),
    ];

    for (source_key, token_key) in flat_overrides {
        if let Some(value) = theme_obj.get(source_key).and_then(Value::as_str) {
            tokens.insert(token_key.to_string(), value.to_string());
        }
    }

    let nested_overrides = [
        ("typography.base_font_family", "font-family-base"),
        ("typography.title_font_size", "font-size-title"),
        ("typography.body_font_size", "font-size-body"),
        ("typography.title_line_height", "line-height-title"),
        ("typography.body_line_height", "line-height-body"),
        ("spacing.slide_padding", "space-slide-padding"),
        ("colors.background", "color-bg"),
        ("colors.text_primary", "color-text-primary"),
        ("colors.text_muted", "color-text-muted"),
    ];

    for (path, token_key) in nested_overrides {
        let mut cursor = Some(Value::Object(theme_obj.clone()));
        for segment in path.split('.') {
            cursor = cursor
                .and_then(|value| value.as_object().cloned().map(Value::Object))
                .and_then(|value| value.get(segment).cloned());
        }
        if let Some(Value::String(value)) = cursor {
            tokens.insert(token_key.to_string(), value);
        }
    }

    tokens
}

fn render_theme_style_block(theme: Option<&serde_json::Map<String, Value>>) -> String {
    let tokens = resolve_theme_token_overrides(theme);
    let mut css = String::new();
    css.push_str("    <style>\n");
    css.push_str("      :root {\n");
    for (key, value) in tokens {
        css.push_str(&format!("        --rs-{key}: {};\n", html_escape(&value)));
    }
    css.push_str("      }\n");
    css.push_str("      body {\n");
    css.push_str("        margin: 0;\n");
    css.push_str("        padding: var(--rs-space-slide-padding);\n");
    css.push_str("        background: var(--rs-color-bg);\n");
    css.push_str("        color: var(--rs-color-text-primary);\n");
    css.push_str("        font-family: var(--rs-font-family-base);\n");
    css.push_str("      }\n");
    css.push_str("    </style>\n");
    css
}

fn render_preview_html(ir_json: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(ir_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    validate_ir(&parsed)?;

    let templates = template_registry();
    let slides = parsed
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| "ValidationError: expected $.slides to be an array.".to_string())?;

    let mut rendered_sections = Vec::new();

    for (index, slide) in slides.iter().enumerate() {
        let layout = slide
            .get("layout")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("ValidationError: missing layout at $.slides[{index}]."))?;
        let template = templates
            .get(layout)
            .ok_or_else(|| format!("RenderError: no template registered for layout '{layout}'"))?;

        let slot_values = slide
            .get("slots")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("ValidationError: missing slots at $.slides[{index}].slots."))?;

        let mut section = template.body.to_string();
        for slot_name in &template.slot_names {
            let slot_path = format!("{{{{ slide.slots.{slot_name} }}}}");
            let slot_value = normalize_slot_text(slot_values.get(slot_name));
            section = section.replace(&slot_path, &html_escape(&slot_value));
        }

        rendered_sections.push(section);
    }

    let theme = parsed.get("theme").and_then(Value::as_object);
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html>\n  <head>\n");
    html.push_str(&render_theme_style_block(theme));
    html.push_str("  </head>\n  <body>\n");
    for section in rendered_sections {
        html.push_str("    ");
        html.push_str(&section);
        html.push('\n');
    }
    html.push_str("  </body>\n</html>\n");

    Ok(html)
}

#[pyfunction]
/// Copies bytes from a source URI to a sink URI using the transport router.
fn copy_source_to_sink(source_uri: &str, sink_uri: &str) -> PyResult<()> {
    use std::io::{Read, Write};

    let router = transport::TransportRouter::new();

    let mut reader = router
        .open_read(source_uri)
        .map_err(|e| PyValueError::new_err(format!("Transport source error: {e}")))?;

    let mut writer = router
        .open_write(sink_uri)
        .map_err(|e| PyValueError::new_err(format!("Transport sink error: {e}")))?;

    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| PyValueError::new_err(format!("Read error: {e}")))?;
        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .map_err(|e| PyValueError::new_err(format!("Write error: {e}")))?;
    }

    writer
        .flush()
        .map_err(|e| PyValueError::new_err(format!("Flush error: {e}")))?;

    Ok(())
}

#[pyfunction]
/// Renders deterministic HTML from IR using the template manifest and slot values.
fn render_html_preview(ir_json: &str) -> PyResult<String> {
    render_preview_html(ir_json).map_err(PyValueError::new_err)
}

#[pyfunction]
/// Placeholder API for PNG rendering while the renderer is not yet implemented.
fn render_pngs(_ir_json: &str, _output_target: &str) -> PyResult<()> {
    Err(PyNotImplementedError::new_err(
        "PNG rendering is not implemented yet. This scaffold only provides API placeholders.",
    ))
}

#[pyfunction]
/// Placeholder API for PPTX rendering while the renderer is not yet implemented.
fn render_pptx(_ir_json: &str, _output_target: &str) -> PyResult<()> {
    Err(PyNotImplementedError::new_err(
        "PPTX rendering is not implemented yet. This scaffold only provides API placeholders.",
    ))
}

#[pymodule]
/// Registers the Python module exports provided by this Rust extension.
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(describe_schema, m)?)?;
    m.add_function(wrap_pyfunction!(list_paths, m)?)?;
    m.add_function(wrap_pyfunction!(list_operations, m)?)?;
    m.add_function(wrap_pyfunction!(explain_operation, m)?)?;
    m.add_function(wrap_pyfunction!(get_examples, m)?)?;
    m.add_function(wrap_pyfunction!(copy_source_to_sink, m)?)?;
    m.add_function(wrap_pyfunction!(render_html_preview, m)?)?;
    m.add_function(wrap_pyfunction!(render_pngs, m)?)?;
    m.add_function(wrap_pyfunction!(render_pptx, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_ir_accepts_minimal_valid_input() {
        let parsed = json!({ "slides": [] });
        assert!(validate_ir(&parsed).is_ok());
    }

    #[test]
    fn validate_ir_rejects_missing_slides() {
        let parsed = json!({ "meta": { "title": "x" } });
        let err = validate_ir(&parsed).expect_err("expected missing-slides validation error");
        assert!(err.contains("ValidationError"));
        assert!(err.contains("missing required field"));
    }

    #[test]
    fn validate_ir_rejects_non_array_slides() {
        let parsed = json!({ "slides": {} });
        let err = validate_ir(&parsed).expect_err("expected non-array slides validation error");
        assert!(err.contains("ValidationError"));
        assert!(err.contains("expected type"));
    }

    #[test]
    fn schema_summary_contains_expected_layouts_and_aliases() {
        let summary = schema_summary();
        assert_eq!(summary.version, "0.1");
        assert!(summary.slide_layouts.contains(&"title_body"));
        assert!(summary.qualitative_aliases.contains(&"left justify"));
    }

    #[test]
    fn operation_specs_exist_for_known_path() {
        let operations = operation_specs_for("slides[*].style.body.font_size")
            .expect("operations should exist for font-size path");
        assert!(operations.iter().any(|op| op.name == "increase"));
        assert!(operations.iter().any(|op| op.name == "decrease"));
    }

    #[test]
    fn template_manifest_operations_are_available() {
        let operations = operation_specs_for("slides[*].slots.title")
            .expect("template-generated operation should be available");
        assert!(operations.iter().any(|op| op.name == "set_text"));
    }

    #[test]
    fn quote_layout_template_operations_are_available() {
        let operations = operation_specs_for("slides[*].slots.quote")
            .expect("quote slot operation should be available");
        assert!(operations.iter().any(|op| op.name == "set_text"));
    }

    #[test]
    fn template_manifest_paths_snapshot_is_stable() {
        assert_eq!(
            all_editable_paths(),
            vec![
                "slides[*].layout",
                "slides[*].slots.attribution",
                "slides[*].slots.body",
                "slides[*].slots.caption",
                "slides[*].slots.image",
                "slides[*].slots.left",
                "slides[*].slots.quote",
                "slides[*].slots.right",
                "slides[*].slots.subtitle",
                "slides[*].slots.title",
                "slides[*].style.alignment",
                "slides[*].style.body.font_size",
            ]
        );
    }

    #[test]
    fn operation_specs_missing_for_unknown_path() {
        assert!(operation_specs_for("slides[*].slots.unknown").is_none());
    }

    #[test]
    fn validate_ir_rejects_missing_required_slot_for_layout() {
        let parsed = json!({
            "slides": [{
                "layout": "title_body",
                "slots": {
                    "title": "Missing body slot"
                }
            }]
        });

        let err = validate_ir(&parsed).expect_err("expected required-slot validation error");
        assert!(err.contains("missing required slot 'body'"));
        assert!(err.contains("$.slides[0].slots"));
    }

    #[test]
    fn validate_ir_accepts_required_slots_for_layout() {
        let parsed = json!({
            "slides": [{
                "layout": "comparison",
                "slots": {
                    "title": "Tradeoffs",
                    "left": "Pros",
                    "right": "Cons"
                }
            }]
        });

        assert!(validate_ir(&parsed).is_ok());
    }

    #[test]
    fn render_preview_html_renders_template_slot_values() {
        let ir_json = r#"{
            "slides": [{
                "layout": "title_body",
                "slots": {
                    "title": "Roadmap",
                    "body": "Phase 1"
                }
            }]
        }"#;

        let html = render_preview_html(ir_json).expect("html preview should render");
        assert!(html.contains("layout-title-body"));
        assert!(html.contains(">Roadmap<"));
        assert!(html.contains(">Phase 1<"));
    }

    #[test]
    fn render_preview_html_escapes_slot_html() {
        let ir_json = r#"{
            "slides": [{
                "layout": "title_body",
                "slots": {
                    "title": "<script>alert(1)</script>",
                    "body": "safe"
                }
            }]
        }"#;

        let html = render_preview_html(ir_json).expect("html preview should render");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
    }
}
