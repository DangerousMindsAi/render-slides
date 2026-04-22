use std::io::Write;
use std::path::PathBuf;

use hyper_render::{render_to_png, Config};
use pyo3::exceptions::PyValueError;
use pyo3::PyResult;
use serde_json::Value;
use url::Url;

use crate::ilm::{build_single_slide_html_from_ilm, resolve_ilm_slides};
use crate::schema::parse_ir;
use crate::transport;

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

pub(crate) fn render_pngs(ir_json: &str, output_target: &str) -> PyResult<()> {
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
