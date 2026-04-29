use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::Value;

use crate::operations;
use crate::output;
use crate::schema;
use crate::transport;

#[pyfunction]
pub(crate) fn validate(ir_json: &str) -> PyResult<String> {
    let parsed: Value = serde_json::from_str(ir_json)
        .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {e}")))?;
    schema::validate_ir(&parsed).map_err(PyValueError::new_err)?;
    Ok("ok".to_string())
}

#[pyfunction]
pub(crate) fn describe_layouts() -> PyResult<String> {
    serde_json::to_string_pretty(&schema::describe_layouts())
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize layout summary: {e}")))
}

#[pyfunction]
pub(crate) fn describe_tweaks(ir_json: &str) -> PyResult<String> {
    serde_json::to_string_pretty(&schema::describe_tweaks(ir_json).map_err(PyValueError::new_err)?)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize tweaks summary: {e}")))
}

#[pyfunction]
pub(crate) fn get_initial_instructions() -> PyResult<String> {
    Ok(schema::get_initial_instructions())
}

#[pyfunction]
pub(crate) fn get_tweak_instructions() -> PyResult<String> {
    Ok(schema::get_tweak_instructions())
}

#[pyfunction(signature = (slide_id=None))]
pub(crate) fn list_paths(slide_id: Option<usize>) -> PyResult<String> {
    let mut paths: Vec<String> = operations::all_editable_paths()
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
pub(crate) fn list_operations(path: &str) -> PyResult<String> {
    let operations = operations::operation_specs_for(path)
        .ok_or_else(|| PyValueError::new_err(format!("Unsupported editable path: {path}")))?;
    serde_json::to_string_pretty(&operations)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize operation listing: {e}")))
}

#[pyfunction]
pub(crate) fn explain_operation(path: &str, operation: &str) -> PyResult<String> {
    let explanation =
        operations::explain_operation(path, operation).map_err(PyValueError::new_err)?;
    serde_json::to_string_pretty(&explanation).map_err(|e| {
        PyValueError::new_err(format!("Failed to serialize operation explanation: {e}"))
    })
}

#[pyfunction]
pub(crate) fn get_examples(path: &str, operation: &str) -> PyResult<String> {
    let examples = operations::get_examples(path, operation).map_err(PyValueError::new_err)?;
    serde_json::to_string_pretty(&examples)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize operation examples: {e}")))
}

#[pyfunction]
pub(crate) fn copy_source_to_sink(source_uri: &str, sink_uri: &str) -> PyResult<()> {
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
pub(crate) fn render_pngs(ir_json: &str, output_target: &str) -> PyResult<()> {
    output::render_pngs(ir_json, output_target)
}

#[pyfunction]
pub(crate) fn register_source_handler(scheme: &str, handler: &str) -> PyResult<()> {
    transport::register_source_handler(scheme, handler)
        .map_err(|e| PyValueError::new_err(format!("Transport source registration error: {e}")))
}

#[pyfunction]
pub(crate) fn register_sink_handler(scheme: &str, handler: &str) -> PyResult<()> {
    transport::register_sink_handler(scheme, handler)
        .map_err(|e| PyValueError::new_err(format!("Transport sink registration error: {e}")))
}

#[pyfunction]
pub(crate) fn render_pptx(ir_json: &str, output_target: &str) -> PyResult<()> {
    output::render_pptx(ir_json, output_target)
}

#[pyfunction]
pub(crate) fn apply_tweaks(ir_json: &str, tweaks_json: &str) -> PyResult<String> {
    crate::patch::apply_tweaks(ir_json, tweaks_json)
        .map_err(PyValueError::new_err)
}
