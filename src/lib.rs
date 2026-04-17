use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;

#[derive(Serialize)]
struct SchemaSummary {
    version: &'static str,
    slide_layouts: Vec<&'static str>,
    qualitative_aliases: Vec<&'static str>,
}

const ALLOWED_LAYOUTS: [&str; 7] = [
    "title",
    "title_body",
    "two_column",
    "section",
    "image_focus",
    "quote",
    "comparison",
];

fn required_slots_for_layout(layout: &str) -> &'static [&'static str] {
    match layout {
        "title" => &["title"],
        "title_body" => &["title", "body"],
        "two_column" => &["left", "right"],
        "section" => &["title"],
        "image_focus" => &["image"],
        "quote" => &["quote"],
        "comparison" => &["left", "right"],
        _ => &[],
    }
}

fn collect_validation_errors(parsed: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    if !parsed.is_object() {
        errors.push(
            "ValidationError at $: root value must be a JSON object containing slideshow fields."
                .to_string(),
        );
        return errors;
    }

    let Some(slides) = parsed.get("slides") else {
        errors.push(
            "ValidationError at $.slides: missing required field; expected an array of slide objects."
                .to_string(),
        );
        return errors;
    };

    let Some(slides_array) = slides.as_array() else {
        errors.push("ValidationError at $.slides: field must be an array.".to_string());
        return errors;
    };

    for (idx, slide) in slides_array.iter().enumerate() {
        let slide_path = format!("$.slides[{idx}]");

        let Some(slide_obj) = slide.as_object() else {
            errors.push(format!(
                "ValidationError at {slide_path}: each slide must be an object."
            ));
            continue;
        };

        let layout_path = format!("{slide_path}.layout");
        let layout = match slide_obj.get("layout").and_then(Value::as_str) {
            Some(layout) => layout,
            None => {
                errors.push(format!(
                    "ValidationError at {layout_path}: missing required string field."
                ));
                continue;
            }
        };

        if !ALLOWED_LAYOUTS.contains(&layout) {
            errors.push(format!(
                "ValidationError at {layout_path}: unsupported layout '{layout}'. Allowed: {}.",
                ALLOWED_LAYOUTS.join(", ")
            ));
        }

        let slots_path = format!("{slide_path}.slots");
        let Some(slots) = slide_obj.get("slots") else {
            errors.push(format!(
                "ValidationError at {slots_path}: missing required object for slot content."
            ));
            continue;
        };

        let Some(slots_obj) = slots.as_object() else {
            errors.push(format!(
                "ValidationError at {slots_path}: must be an object containing named slots."
            ));
            continue;
        };

        for required_slot in required_slots_for_layout(layout) {
            match slots_obj.get(*required_slot).and_then(Value::as_str) {
                Some(value) if !value.trim().is_empty() => {}
                _ => {
                    errors.push(format!(
                        "ValidationError at {slots_path}.{required_slot}: required non-empty string for layout '{layout}'."
                    ));
                }
            }
        }
    }

    errors
}

fn validate_ir(parsed: &Value) -> Result<(), Vec<String>> {
    let errors = collect_validation_errors(parsed);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

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
fn validate(ir_json: &str) -> PyResult<String> {
    let parsed: Value = serde_json::from_str(ir_json)
        .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {e}")))?;

    validate_ir(&parsed).map_err(|errors| PyValueError::new_err(errors.join("\n")))?;

    Ok("ok".to_string())
}

#[pyfunction]
fn validate_detailed(ir_json: &str) -> PyResult<String> {
    let parsed: Value = serde_json::from_str(ir_json)
        .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {e}")))?;

    let errors = collect_validation_errors(&parsed);
    let payload = json!({
        "valid": errors.is_empty(),
        "error_count": errors.len(),
        "errors": errors,
    });

    serde_json::to_string_pretty(&payload)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize validation payload: {e}")))
}

#[pyfunction]
fn describe_schema() -> PyResult<String> {
    let summary = schema_summary();

    serde_json::to_string_pretty(&summary)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize schema summary: {e}")))
}

#[pyfunction]
fn render_pngs(_ir_json: &str, _output_target: &str) -> PyResult<()> {
    Err(PyNotImplementedError::new_err(
        "PNG rendering is not implemented yet. This scaffold only provides API placeholders.",
    ))
}

#[pyfunction]
fn render_pptx(_ir_json: &str, _output_target: &str) -> PyResult<()> {
    Err(PyNotImplementedError::new_err(
        "PPTX rendering is not implemented yet. This scaffold only provides API placeholders.",
    ))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(validate_detailed, m)?)?;
    m.add_function(wrap_pyfunction!(describe_schema, m)?)?;
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
        assert!(err[0].contains("$.slides"));
    }

    #[test]
    fn validate_ir_rejects_non_array_slides() {
        let parsed = json!({ "slides": {} });
        let err = validate_ir(&parsed).expect_err("expected non-array slides validation error");
        assert!(err[0].contains("must be an array"));
    }

    #[test]
    fn validate_ir_rejects_unknown_layout() {
        let parsed = json!({
            "slides": [{
                "layout": "made_up_layout",
                "slots": {}
            }]
        });
        let err = validate_ir(&parsed).expect_err("expected unknown-layout validation error");
        assert!(err.iter().any(|e| e.contains("unsupported layout")));
    }

    #[test]
    fn validate_ir_requires_layout_slots() {
        let parsed = json!({
            "slides": [{
                "layout": "title_body",
                "slots": { "title": "Hello" }
            }]
        });
        let err = validate_ir(&parsed).expect_err("expected missing required slot validation");
        assert!(
            err.iter()
                .any(|e| e.contains("$.slides[0].slots.body") && e.contains("required non-empty string"))
        );
    }

    #[test]
    fn schema_summary_contains_expected_layouts_and_aliases() {
        let summary = schema_summary();
        assert_eq!(summary.version, "0.1");
        assert!(summary.slide_layouts.contains(&"title_body"));
        assert!(summary.qualitative_aliases.contains(&"left justify"));
    }
}
