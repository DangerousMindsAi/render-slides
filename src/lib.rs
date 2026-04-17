use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use serde::Serialize;
use serde_json::Value;

mod transport;

#[derive(Serialize)]
struct SchemaSummary {
    version: &'static str,
    slide_layouts: Vec<&'static str>,
    qualitative_aliases: Vec<&'static str>,
}

fn validate_ir(parsed: &Value) -> Result<(), String> {
    if parsed.get("slides").is_none() {
        return Err(
            "ValidationError: missing required field at $.slides; expected an array of slide objects."
                .to_string(),
        );
    }

    if !parsed.get("slides").is_some_and(|v| v.is_array()) {
        return Err("ValidationError: field $.slides must be an array.".to_string());
    }

    Ok(())
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

    validate_ir(&parsed).map_err(PyValueError::new_err)?;

    Ok("ok".to_string())
}

#[pyfunction]
fn describe_schema() -> PyResult<String> {
    let summary = schema_summary();

    serde_json::to_string_pretty(&summary)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize schema summary: {e}")))
}

#[pyfunction]
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
    m.add_function(wrap_pyfunction!(describe_schema, m)?)?;
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
        assert!(err.contains("$.slides"));
    }

    #[test]
    fn validate_ir_rejects_non_array_slides() {
        let parsed = json!({ "slides": {} });
        let err = validate_ir(&parsed).expect_err("expected non-array slides validation error");
        assert!(err.contains("must be an array"));
    }

    #[test]
    fn schema_summary_contains_expected_layouts_and_aliases() {
        let summary = schema_summary();
        assert_eq!(summary.version, "0.1");
        assert!(summary.slide_layouts.contains(&"title_body"));
        assert!(summary.qualitative_aliases.contains(&"left justify"));
    }
}
