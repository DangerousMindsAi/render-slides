use std::io::Write;
use std::path::PathBuf;

use cairo::{Context, Format, ImageSurface};
use pango::FontDescription;
use pangocairo::functions::{create_layout, show_layout};
use pyo3::exceptions::PyValueError;
use pyo3::PyResult;
use serde_json::Value;
use url::Url;

use crate::ilm::model::{IlmElement, IlmSlide, ImageScaling, TextAlignment};
use crate::ilm::resolve_ilm_slides;
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

fn rasterize_ilm_to_png_bytes(
    slide: &IlmSlide,
    _theme: Option<&serde_json::Map<String, Value>>,
) -> Result<Vec<u8>, String> {
    let width = 1366;
    let height = 768;
    let surface = ImageSurface::create(Format::ARgb32, width, height)
        .map_err(|e| format!("Failed to create surface: {e}"))?;

    {
        let cr = Context::new(&surface).map_err(|e| format!("Failed to create context: {e}"))?;

        // Fill background
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().map_err(|e| format!("Paint error: {e}"))?;

        let to_px = |emu: i64| (emu as f64) / 9525.0;

        for elem in &slide.elements {
            match elem {
                IlmElement::Image(img_elem) => {
                    if let Ok(dyn_img) = image::load_from_memory(&img_elem.image_data) {
                        let rgba = dyn_img.into_rgba8();
                        let img_w = rgba.width();
                        let img_h = rgba.height();

                        let mut cairo_data = vec![0u8; (img_w * img_h * 4) as usize];
                        for (i, pixel) in rgba.pixels().enumerate() {
                            let r = pixel[0] as f64 / 255.0;
                            let g = pixel[1] as f64 / 255.0;
                            let b = pixel[2] as f64 / 255.0;
                            let a = pixel[3] as f64 / 255.0;

                            cairo_data[i * 4 + 0] = (b * a * 255.0) as u8;
                            cairo_data[i * 4 + 1] = (g * a * 255.0) as u8;
                            cairo_data[i * 4 + 2] = (r * a * 255.0) as u8;
                            cairo_data[i * 4 + 3] = (a * 255.0) as u8;
                        }

                        if let Ok(img_surf) = ImageSurface::create_for_data(
                            cairo_data,
                            Format::ARgb32,
                            img_w as i32,
                            img_h as i32,
                            (img_w * 4) as i32,
                        ) {
                            let target_w = to_px(img_elem.cx);
                            let target_h = to_px(img_elem.cy);
                            let target_x = to_px(img_elem.x);
                            let target_y = to_px(img_elem.y);

                            cr.save().map_err(|e| format!("Context save error: {e}"))?;

                            cr.rectangle(target_x, target_y, target_w, target_h);
                            cr.clip();

                            let img_w_f = img_w as f64;
                            let img_h_f = img_h as f64;

                            match img_elem.scaling {
                                ImageScaling::Stretch => {
                                    cr.translate(target_x, target_y);
                                    cr.scale(target_w / img_w_f, target_h / img_h_f);
                                }
                                ImageScaling::Contain => {
                                    let scale = (target_w / img_w_f).min(target_h / img_h_f);
                                    let out_w = img_w_f * scale;
                                    let out_h = img_h_f * scale;
                                    cr.translate(
                                        target_x + (target_w - out_w) / 2.0,
                                        target_y + (target_h - out_h) / 2.0,
                                    );
                                    cr.scale(scale, scale);
                                }
                                ImageScaling::Cover => {
                                    let scale = (target_w / img_w_f).max(target_h / img_h_f);
                                    let out_w = img_w_f * scale;
                                    let out_h = img_h_f * scale;
                                    cr.translate(
                                        target_x + (target_w - out_w) / 2.0,
                                        target_y + (target_h - out_h) / 2.0,
                                    );
                                    cr.scale(scale, scale);
                                }
                                ImageScaling::FitWidth => {
                                    let scale = target_w / img_w_f;
                                    let out_h = img_h_f * scale;
                                    cr.translate(target_x, target_y + (target_h - out_h) / 2.0);
                                    cr.scale(scale, scale);
                                }
                                ImageScaling::FitHeight => {
                                    let scale = target_h / img_h_f;
                                    let out_w = img_w_f * scale;
                                    cr.translate(target_x + (target_w - out_w) / 2.0, target_y);
                                    cr.scale(scale, scale);
                                }
                            }

                            cr.set_source_surface(&img_surf, 0.0, 0.0)
                                .map_err(|e| format!("Set source surface error: {e}"))?;
                            cr.paint().map_err(|e| format!("Paint error: {e}"))?;

                            cr.restore()
                                .map_err(|e| format!("Context restore error: {e}"))?;
                        }
                    }
                }
                IlmElement::Text(run) => {
                    cr.save().map_err(|e| format!("Context save error: {e}"))?;
                    cr.set_source_rgb(0.0, 0.0, 0.0);

                    let target_w = to_px(run.cx);
                    let target_x = to_px(run.x);
                    let target_y = to_px(run.y);

                    use crate::ilm::markdown::{ListType, RichBlock};

                    let font_size_px = run.font_size_pt as f64 * 96.0 / 72.0;
                    // Match OpenXML's default of zero extra space between paragraphs
                    let paragraph_spacing_px = 0.0;

                    // OpenXML has a slight default top padding/ascent shift compared to Pango.
                    // This offset matches the PowerPoint text box start position.
                    let mut current_y = target_y + font_size_px * 0.08;

                    for block in &run.blocks {
                        match block {
                            RichBlock::Paragraph(para) => {
                                let indent_px = para.list_level as f64 * 342900.0 / 9525.0;
                                let effective_width = (target_w - indent_px).max(10.0);
                                let effective_x = target_x + indent_px;

                                let mut bullet_str = String::new();
                                if para.list_level > 0 {
                                    if let Some(ListType::Ordered(n)) = para.list_type {
                                        bullet_str = format!("{}. ", n);
                                    } else {
                                        bullet_str = "• ".to_string();
                                    }
                                }

                                if !bullet_str.is_empty() {
                                    let bullet_x = target_x + (para.list_level as f64 - 1.0) * 342900.0 / 9525.0;
                                    cr.move_to(bullet_x, current_y);
                                    let b_layout = create_layout(&cr);
                                    let mut b_font = FontDescription::new();
                                    b_font.set_family("Arial");
                                    b_font.set_absolute_size(font_size_px * pango::SCALE as f64);
                                    b_layout.set_font_description(Some(&b_font));
                                    b_layout.set_text(&bullet_str);
                                    show_layout(&cr, &b_layout);
                                }

                                cr.move_to(effective_x, current_y);
                                let layout = create_layout(&cr);
                                let mut font = FontDescription::new();
                                font.set_family("Arial");
                                font.set_absolute_size(font_size_px * pango::SCALE as f64);
                                layout.set_font_description(Some(&font));
                                
                                // Line spacing offset to match OpenXML's default Arial metrics.
                                // OpenXML's line gap increases non-linearly compared to Pango.
                                // At 10pt (13.33px), they match perfectly (0px). At 28pt (37.33px), OpenXML is ~2.6px taller per line.
                                let line_spacing_px = ((font_size_px - 13.33) * 0.10875).max(0.0);
                                layout.set_spacing((line_spacing_px * pango::SCALE as f64) as i32);

                                let mut markup = String::new();
                                for r in &para.runs {
                                    let escaped = glib::markup_escape_text(&r.text);
                                    let mut span = escaped.to_string();

                                    if r.bold || run.bold {
                                        span = format!("<b>{}</b>", span);
                                    }
                                    if r.italic {
                                        span = format!("<i>{}</i>", span);
                                    }
                                    if r.strikethrough {
                                        span = format!("<s>{}</s>", span);
                                    }
                                    if r.is_code || para.is_code_block {
                                        span = format!(
                                            "<span font_family=\"Courier New\">{}</span>",
                                            span
                                        );
                                    } else {
                                        // OpenXML kerning offsets also scale non-linearly with font size.
                                        // At 10pt (13.33px), no kerning offset is needed.
                                        // At 24pt (32px), OpenXML text is tighter by ~0.128px per character.
                                        let letter_spacing_px = ((font_size_px - 13.33) * 0.00685).max(0.0);
                                        let letter_spacing = -(letter_spacing_px * pango::SCALE as f64) as i32;
                                        span = format!("<span letter_spacing=\"{}\">{}</span>", letter_spacing, span);
                                    }

                                    markup.push_str(&span);
                                }

                                layout.set_markup(&markup);
                                layout.set_width((effective_width * pango::SCALE as f64) as i32);
                                layout.set_wrap(pango::WrapMode::WordChar);

                                match run.alignment {
                                    TextAlignment::Left => {
                                        layout.set_alignment(pango::Alignment::Left)
                                    }
                                    TextAlignment::Center => {
                                        layout.set_alignment(pango::Alignment::Center)
                                    }
                                    TextAlignment::Right => {
                                        layout.set_alignment(pango::Alignment::Right)
                                    }
                                    TextAlignment::Justify => {
                                        layout.set_justify(true);
                                        layout.set_alignment(pango::Alignment::Left);
                                    }
                                }

                                show_layout(&cr, &layout);

                                let (_, height) = layout.pixel_size();
                                current_y += height as f64 + paragraph_spacing_px;
                            }
                            RichBlock::Table(_) => {}
                        }
                    }
                    cr.restore()
                        .map_err(|e| format!("Context restore error: {e}"))?;
                }
            }
        }
    }

    let mut output = Vec::new();
    surface
        .write_to_png(&mut output)
        .map_err(|e| format!("PNG encode error: {e}"))?;
    Ok(output)
}

pub(crate) fn render_pngs(ir_json: &str, output_target: &str) -> PyResult<()> {
    let parsed = parse_ir(ir_json).map_err(PyValueError::new_err)?;
    let ilm_slides = resolve_ilm_slides(&parsed).map_err(PyValueError::new_err)?;
    let theme = parsed.get("theme").and_then(Value::as_object);

    let router = transport::TransportRouter::new();
    for (index, slide) in ilm_slides.iter().enumerate() {
        let png_bytes = rasterize_ilm_to_png_bytes(slide, theme).map_err(PyValueError::new_err)?;
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
