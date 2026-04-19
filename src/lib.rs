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

use std::collections::BTreeSet;
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

/// Validates the minimal contract for a render-slides IR payload.
fn validate_ir(parsed: &Value) -> Result<(), String> {
    let validator = IR_VALIDATOR.as_ref().map_err(ToString::to_string)?;
    let mut errors = validator.iter_errors(parsed);
    if let Some(first) = errors.next() {
        return Err(format_validation_error(first));
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
    fn operation_specs_missing_for_unknown_path() {
        assert!(operation_specs_for("slides[*].slots.unknown").is_none());
    }
}
