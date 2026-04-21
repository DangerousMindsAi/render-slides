//! Core Rust implementation for the `render_slides` Python package.
//!
//! This crate currently exposes a small Python API surface for:
//! - validating slideshow IR JSON payloads,
//! - describing the currently supported schema summary,
//! - introspecting editable IR paths and operations, and
//! - copying bytes across local or HTTP(S) transports.
//!
//! Rendering entry points now emit deterministic artifacts; PNG output is
//! rasterized from rendered slide HTML, while PPTX output is still
//! placeholder-only.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::LazyLock;

use hyper_render::{render_to_png, Config};
use jsonschema::{error::ValidationErrorKind, Draft, Validator};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde::Serialize;
use serde_json::Value;
use url::Url;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

        let (required_slots, optional_slots): (&[&str], &[&str]) = match layout {
            "title" => (&["title", "subtitle"], &[]),
            "title_body" => (&["title", "body"], &["subtitle"]),
            "two_column" => (&["title", "left", "right"], &["subtitle"]),
            "section" => (&["title", "subtitle"], &[]),
            "image_focus" => (&["title", "image", "caption"], &["subtitle"]),
            "quote" => (&["quote", "attribution"], &[]),
            "comparison" => (&["title", "left", "right"], &["subtitle"]),
            _ => continue,
        };

        let Some(slots) = slide.get("slots").and_then(Value::as_object) else {
            continue;
        };

        for required_slot in required_slots {
            if !slots.contains_key(*required_slot) {
                let mut provided_slots: Vec<&str> = slots.keys().map(String::as_str).collect();
                provided_slots.sort_unstable();
                let suggested_fix = format!("Add slots.{required_slot} as a string value.");
                return Err(format!(
                    "ValidationError: missing required slot '{required_slot}' for layout '{layout}' at $.slides[{index}].slots. expected_required={required_slots:?}; optional={optional_slots:?}; provided={provided_slots:?}; suggested_fix=\"{suggested_fix}\"."
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

fn resolve_theme_token_overrides(
    theme: Option<&serde_json::Map<String, Value>>,
) -> BTreeMap<String, String> {
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
    let parsed = parse_ir(ir_json)?;
    render_preview_html_from_parsed(&parsed)
}

fn render_preview_html_from_parsed(parsed: &Value) -> Result<String, String> {
    validate_ir(parsed)?;

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

fn parse_ir(ir_json: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(ir_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    validate_ir(&parsed)?;
    Ok(parsed)
}

fn slide_sink_uri(base_output_target: &str, filename: &str) -> Result<String, String> {
    match Url::parse(base_output_target) {
        Ok(url) => match url.scheme() {
            "http" | "https" | "s3" => {
                let mut base = base_output_target.to_string();
                if !base.ends_with('/') {
                    base.push('/');
                }
                base.push_str(filename);
                Ok(base)
            }
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| format!("Invalid file URI output target: {base_output_target}"))?;
                std::fs::create_dir_all(&path)
                    .map_err(|e| format!("Failed to create output directory '{path:?}': {e}"))?;
                let file_path = path.join(filename);
                Url::from_file_path(&file_path)
                    .map_err(|_| format!("Failed to build file URI for '{file_path:?}'"))
                    .map(|u| u.to_string())
            }
            other => Err(format!(
                "Unsupported output target scheme for PNG rendering: {other}"
            )),
        },
        Err(_) => {
            let base_path = PathBuf::from(base_output_target);
            std::fs::create_dir_all(&base_path).map_err(|e| {
                format!("Failed to create output directory '{base_output_target}': {e}")
            })?;
            Ok(base_path.join(filename).to_string_lossy().to_string())
        }
    }
}

fn rasterize_html_to_png_bytes(html: &str) -> Result<Vec<u8>, String> {
    let config = Config::new().width(1366).height(768);
    render_to_png(html, config).map_err(|e| format!("PNG render error: {e}"))
}

#[derive(Clone)]
struct IlmTextRun {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
    text: String,
    font_size_pt: i64,
    bold: bool,
}

#[derive(Clone)]
struct IlmImage {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
    uri: String,
}

#[derive(Clone)]
struct IlmSlide {
    text_runs: Vec<IlmTextRun>,
    image: Option<IlmImage>,
}

fn xml_escape(input: &str) -> String {
    html_escape(input)
}

fn slot_text(slots: &serde_json::Map<String, Value>, name: &str) -> String {
    normalize_slot_text(slots.get(name))
}

fn ilm_slide_from_ir(slide: &Value) -> Option<IlmSlide> {
    let layout = slide.get("layout")?.as_str()?;
    let slots = slide.get("slots")?.as_object()?;
    let emu = |px: i64| px * 9525;

    let spec = match layout {
        "title" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(180),
                    cx: emu(1174),
                    cy: emu(180),
                    text: slot_text(slots, "title"),
                    font_size_pt: 44,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(390),
                    cx: emu(1174),
                    cy: emu(140),
                    text: slot_text(slots, "subtitle"),
                    font_size_pt: 28,
                    bold: false,
                },
            ],
            image: None,
        },
        "title_body" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(72),
                    cx: emu(1174),
                    cy: emu(120),
                    text: slot_text(slots, "title"),
                    font_size_pt: 40,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(220),
                    cx: emu(1174),
                    cy: emu(430),
                    text: slot_text(slots, "body"),
                    font_size_pt: 24,
                    bold: false,
                },
            ],
            image: None,
        },
        "two_column" | "comparison" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(48),
                    cx: emu(1174),
                    cy: emu(110),
                    text: slot_text(slots, "title"),
                    font_size_pt: 36,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(190),
                    cx: emu(560),
                    cy: emu(520),
                    text: slot_text(slots, "left"),
                    font_size_pt: 22,
                    bold: false,
                },
                IlmTextRun {
                    x: emu(710),
                    y: emu(190),
                    cx: emu(560),
                    cy: emu(520),
                    text: slot_text(slots, "right"),
                    font_size_pt: 22,
                    bold: false,
                },
            ],
            image: None,
        },
        "section" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(240),
                    cx: emu(1174),
                    cy: emu(170),
                    text: slot_text(slots, "title"),
                    font_size_pt: 46,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(430),
                    cx: emu(1174),
                    cy: emu(120),
                    text: slot_text(slots, "subtitle"),
                    font_size_pt: 24,
                    bold: false,
                },
            ],
            image: None,
        },
        "image_focus" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(72),
                    y: emu(48),
                    cx: emu(1220),
                    cy: emu(90),
                    text: slot_text(slots, "title"),
                    font_size_pt: 32,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(72),
                    y: emu(650),
                    cx: emu(1220),
                    cy: emu(80),
                    text: slot_text(slots, "caption"),
                    font_size_pt: 20,
                    bold: false,
                },
            ],
            image: slots
                .get("image")
                .and_then(Value::as_str)
                .map(|uri| IlmImage {
                    x: emu(170),
                    y: emu(150),
                    cx: emu(1026),
                    cy: emu(470),
                    uri: uri.to_string(),
                }),
        },
        "quote" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(120),
                    y: emu(180),
                    cx: emu(1120),
                    cy: emu(320),
                    text: slot_text(slots, "quote"),
                    font_size_pt: 34,
                    bold: false,
                },
                IlmTextRun {
                    x: emu(120),
                    y: emu(540),
                    cx: emu(1120),
                    cy: emu(90),
                    text: format!("— {}", slot_text(slots, "attribution")),
                    font_size_pt: 22,
                    bold: true,
                },
            ],
            image: None,
        },
        _ => return None,
    };
    Some(spec)
}

fn resolve_ilm_slides(parsed: &Value) -> Result<Vec<IlmSlide>, String> {
    let slides = parsed
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| "ValidationError: expected $.slides to be an array.".to_string())?;
    let ilm: Vec<IlmSlide> = slides.iter().filter_map(ilm_slide_from_ir).collect();
    if ilm.len() != slides.len() {
        return Err(
            "RenderError: failed to resolve one or more slide layouts for ILM emission."
                .to_string(),
        );
    }
    Ok(ilm)
}

fn build_single_slide_html_from_ilm(
    slide: &IlmSlide,
    theme: Option<&serde_json::Map<String, Value>>,
) -> String {
    let to_px = |emu: i64| emu / 9525;
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html>\n  <head>\n");
    html.push_str(&render_theme_style_block(theme));
    html.push_str("    <style>\n      html, body { width: 1366px; height: 768px; }\n");
    html.push_str(
        "      body { margin: 0; padding: 0; overflow: hidden; position: relative; box-sizing: border-box; }\n",
    );
    html.push_str("      .ilm-text { position: absolute; white-space: pre-wrap; }\n");
    html.push_str("      .ilm-image { position: absolute; object-fit: cover; }\n");
    html.push_str("    </style>\n  </head>\n  <body>\n");
    if let Some(image) = &slide.image {
        html.push_str(&format!(
            "    <img class=\"ilm-image\" src=\"{}\" style=\"left:{}px;top:{}px;width:{}px;height:{}px;\"/>\n",
            html_escape(&image.uri),
            to_px(image.x),
            to_px(image.y),
            to_px(image.cx),
            to_px(image.cy)
        ));
    }
    for run in &slide.text_runs {
        html.push_str(&format!(
            "    <div class=\"ilm-text\" style=\"left:{}px;top:{}px;width:{}px;height:{}px;font-size:{}pt;font-weight:{};\">{}</div>\n",
            to_px(run.x),
            to_px(run.y),
            to_px(run.cx),
            to_px(run.cy),
            run.font_size_pt,
            if run.bold { "700" } else { "400" },
            html_escape(&run.text).replace('\n', "<br/>")
        ));
    }
    html.push_str("  </body>\n</html>\n");
    html
}

fn detect_image_extension(image_uri: &str, bytes: &[u8]) -> &'static str {
    let lower = image_uri.to_ascii_lowercase();
    if lower.ends_with(".png") || bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "png";
    }
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
    {
        return "jpg";
    }
    if lower.ends_with(".gif") || bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "gif";
    }
    "bin"
}

fn add_zip_file(
    zip: &mut ZipWriter<std::io::Cursor<Vec<u8>>>,
    path: &str,
    data: &str,
) -> Result<(), String> {
    zip.start_file(path, SimpleFileOptions::default())
        .map_err(|e| format!("PPTX zip start_file error for {path}: {e}"))?;
    zip.write_all(data.as_bytes())
        .map_err(|e| format!("PPTX zip write error for {path}: {e}"))
}

fn build_pptx_bytes(parsed: &Value) -> Result<Vec<u8>, String> {
    let specs = resolve_ilm_slides(parsed)?;

    let router = transport::TransportRouter::new();
    let mut media: Vec<(String, Vec<u8>, &'static str)> = Vec::new();
    for (idx, spec) in specs.iter().enumerate() {
        if let Some(image) = &spec.image {
            let mut reader = router.open_read(&image.uri).map_err(|e| {
                format!(
                    "AssetError: failed to read image for slide {}: {}",
                    idx + 1,
                    e
                )
            })?;
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).map_err(|e| {
                format!(
                    "AssetError: failed to read image bytes for slide {}: {}",
                    idx + 1,
                    e
                )
            })?;
            let ext = detect_image_extension(&image.uri, &bytes);
            media.push((format!("image{}.{}", media.len() + 1, ext), bytes, ext));
        }
    }

    let cursor = std::io::Cursor::new(Vec::<u8>::new());
    let mut zip = ZipWriter::new(cursor);

    let mut slide_rel_targets = Vec::new();
    let mut media_idx = 0usize;
    for (idx, spec) in specs.iter().enumerate() {
        let slide_number = idx + 1;
        let mut shapes_xml = String::new();
        let mut shape_id = 2usize;
        for tb in &spec.text_runs {
            let run_attr = if tb.bold { " b=\"1\"" } else { "" };
            let lines: Vec<&str> = tb.text.lines().collect();
            let mut paragraphs = String::new();
            for line in lines {
                paragraphs.push_str(&format!(
                    "<a:p><a:r><a:rPr lang=\"en-US\" sz=\"{}\"{} /><a:t>{}</a:t></a:r></a:p>",
                    tb.font_size_pt * 100,
                    run_attr,
                    xml_escape(line)
                ));
            }
            if paragraphs.is_empty() {
                paragraphs.push_str("<a:p/>");
            }
            shapes_xml.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"TextBox {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\"/><a:lstStyle/>{}</p:txBody></p:sp>",
                shape_id, shape_id, tb.x, tb.y, tb.cx, tb.cy, paragraphs
            ));
            shape_id += 1;
        }

        let mut rels_xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        let mut pic_xml = String::new();
        if let Some(img) = &spec.image {
            let (_, _, ext) = &media[media_idx];
            let rid = "rId1";
            rels_xml.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>",
                rid, media[media_idx].0
            ));
            pic_xml = format!(
                "<p:pic><p:nvPicPr><p:cNvPr id=\"{}\" name=\"Image\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
                shape_id, rid, img.x, img.y, img.cx, img.cy
            );
            let _ = ext;
            media_idx += 1;
        }
        rels_xml.push_str("</Relationships>");

        let slide_xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>{}{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
            shapes_xml, pic_xml
        );
        add_zip_file(
            &mut zip,
            &format!("ppt/slides/slide{slide_number}.xml"),
            &slide_xml,
        )?;
        if spec.image.is_some() {
            add_zip_file(
                &mut zip,
                &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
                &rels_xml,
            )?;
        }
        slide_rel_targets.push(format!("slides/slide{slide_number}.xml"));
    }

    for (name, bytes, _) in &media {
        zip.start_file(format!("ppt/media/{name}"), SimpleFileOptions::default())
            .map_err(|e| format!("PPTX zip start_file error for media {name}: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("PPTX zip write error for media {name}: {e}"))?;
    }

    let mut content_types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/><Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    for i in 1..=slide_rel_targets.len() {
        content_types.push_str(&format!("<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"));
    }
    for (_, _, ext) in &media {
        let ct = match *ext {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            _ => "application/octet-stream",
        };
        content_types.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            ext, ct
        ));
    }
    content_types.push_str("</Types>");
    add_zip_file(&mut zip, "[Content_Types].xml", &content_types)?;

    add_zip_file(
        &mut zip,
        "_rels/.rels",
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>",
    )?;
    add_zip_file(
        &mut zip,
        "docProps/app.xml",
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>render-slides</Application></Properties>",
    )?;
    add_zip_file(
        &mut zip,
        "docProps/core.xml",
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>render-slides deck</dc:title><dc:creator>render-slides</dc:creator></cp:coreProperties>",
    )?;

    let mut presentation = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldSz cx=\"13004800\" cy=\"7315200\" type=\"wide\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/><p:sldIdLst>");
    for i in 1..=slide_rel_targets.len() {
        presentation.push_str(&format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i));
    }
    presentation.push_str("</p:sldIdLst></p:presentation>");
    add_zip_file(&mut zip, "ppt/presentation.xml", &presentation)?;

    let mut pres_rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    for (i, target) in slide_rel_targets.iter().enumerate() {
        pres_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"{}\"/>", i + 1, target));
    }
    pres_rels.push_str("</Relationships>");
    add_zip_file(&mut zip, "ppt/_rels/presentation.xml.rels", &pres_rels)?;

    let cursor = zip
        .finish()
        .map_err(|e| format!("PPTX zip finalize error: {e}"))?;
    Ok(cursor.into_inner())
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
/// Renders one slide PNG image per IR slide to the output target.
fn render_pngs(ir_json: &str, output_target: &str) -> PyResult<()> {
    let parsed = parse_ir(ir_json).map_err(PyValueError::new_err)?;
    let ilm_slides = resolve_ilm_slides(&parsed).map_err(PyValueError::new_err)?;
    let theme = parsed.get("theme").and_then(Value::as_object);

    let router = transport::TransportRouter::new();
    for (index, slide) in ilm_slides.iter().enumerate() {
        let slide_html = build_single_slide_html_from_ilm(slide, theme);
        let png_bytes = rasterize_html_to_png_bytes(&slide_html).map_err(PyValueError::new_err)?;
        let filename = format!("slide-{:03}.png", index + 1);
        let sink_uri = slide_sink_uri(output_target, &filename).map_err(PyValueError::new_err)?;
        let mut writer = router
            .open_write(&sink_uri)
            .map_err(|e| PyValueError::new_err(format!("Transport sink error: {e}")))?;
        writer
            .write_all(&png_bytes)
            .map_err(|e| PyValueError::new_err(format!("Write error: {e}")))?;
        writer
            .flush()
            .map_err(|e| PyValueError::new_err(format!("Flush error: {e}")))?;
    }
    Ok(())
}

#[pyfunction]
/// Registers a custom URI scheme alias for source reads.
fn register_source_handler(scheme: &str, handler: &str) -> PyResult<()> {
    transport::register_source_handler(scheme, handler)
        .map_err(|e| PyValueError::new_err(format!("Transport source registration error: {e}")))
}

#[pyfunction]
/// Registers a custom URI scheme alias for sink writes.
fn register_sink_handler(scheme: &str, handler: &str) -> PyResult<()> {
    transport::register_sink_handler(scheme, handler)
        .map_err(|e| PyValueError::new_err(format!("Transport sink registration error: {e}")))
}

#[pyfunction]
/// Writes a deterministic standards-compliant OpenXML PPTX payload to the output target.
fn render_pptx(ir_json: &str, output_target: &str) -> PyResult<()> {
    let parsed = parse_ir(ir_json).map_err(PyValueError::new_err)?;
    let bytes = build_pptx_bytes(&parsed).map_err(PyValueError::new_err)?;
    let router = transport::TransportRouter::new();
    let mut writer = router
        .open_write(output_target)
        .map_err(|e| PyValueError::new_err(format!("Transport sink error: {e}")))?;
    writer
        .write_all(&bytes)
        .map_err(|e| PyValueError::new_err(format!("Write error: {e}")))?;
    writer
        .flush()
        .map_err(|e| PyValueError::new_err(format!("Flush error: {e}")))?;
    Ok(())
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
    m.add_function(wrap_pyfunction!(register_source_handler, m)?)?;
    m.add_function(wrap_pyfunction!(register_sink_handler, m)?)?;
    m.add_function(wrap_pyfunction!(render_html_preview, m)?)?;
    m.add_function(wrap_pyfunction!(render_pngs, m)?)?;
    m.add_function(wrap_pyfunction!(render_pptx, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    use zip::ZipArchive;

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

    #[test]
    fn ilm_single_slide_html_resets_body_padding_for_geometry_parity() {
        let slide = ilm_slide_from_ir(&json!({
            "layout": "title",
            "slots": {
                "title": "Quarterly Update",
                "subtitle": "FY26"
            }
        }))
        .expect("ilm slide");

        let html = build_single_slide_html_from_ilm(&slide, None);
        assert!(html.contains("body { margin: 0; padding: 0;"));
    }

    #[test]
    fn build_pptx_bytes_emits_openxml_package_entries() {
        let parsed = json!({
            "slides": [{
                "layout": "title_body",
                "slots": { "title": "Roadmap", "body": "Phase 1\nPhase 2" }
            }]
        });
        let bytes = build_pptx_bytes(&parsed).expect("pptx should build");
        let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide present")
            .read_to_string(&mut slide_xml)
            .expect("slide xml readable");
        assert!(slide_xml.contains("Roadmap"));
        assert!(slide_xml.contains("Phase 1"));
    }
}
