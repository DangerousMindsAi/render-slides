//! Core Rust implementation for the `render_slides` Python package.

use pyo3::prelude::*;

pub mod transport;

pub(crate) mod generated {
    include!(concat!(env!("OUT_DIR"), "/template_manifest.rs"));
}

mod ilm;
mod operations;
mod output;
mod py_api;
mod schema;
mod theme;
mod types;

#[pymodule]
/// Registers the Python module exports provided by this Rust extension.
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_api::validate, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::describe_layouts, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::describe_tweaks, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::get_initial_instructions, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::get_tweak_instructions, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::list_paths, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::list_operations, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::explain_operation, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::get_examples, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::copy_source_to_sink, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::register_source_handler, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::register_sink_handler, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::render_pngs, m)?)?;
    m.add_function(wrap_pyfunction!(py_api::render_pptx, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
