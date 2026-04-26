use serde_json::Value;

use super::model::{IlmElement, IlmImage, IlmSlide, IlmTextRun, ImageScaling, TextAlignment};
use crate::generated;
use crate::ilm::expr::{evaluate, EvalContext};
use std::collections::BTreeMap;
use base64::{engine::general_purpose::STANDARD as b64, Engine};

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

fn normalize_slot_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn slot_text(slots: &serde_json::Map<String, Value>, name: &str) -> String {
    normalize_slot_text(slots.get(name))
}

use crate::ilm::markdown::{RichBlock, ListType};

fn calculate_autofit_font_size(
    font_system: &mut FontSystem,
    blocks: &[RichBlock],
    width_emu: i64,
    height_emu: i64,
    default_font_size_pt: f64,
    bold_default: bool,
) -> i64 {
    let width_px = width_emu as f32 / 9525.0;
    let height_px = height_emu as f32 / 9525.0;
    
    let mut current_pt = default_font_size_pt;
    let min_pt = 10.0;
    
    loop {
        let font_size_px = current_pt * 96.0 / 72.0;
        let line_height_px = font_size_px * 1.2;
        let paragraph_spacing_px = line_height_px * 0.3; // 0.3 lines between paragraphs
        
        let mut total_height = 0.0;
        
        for block in blocks {
            match block {
                RichBlock::Paragraph(para) => {
                    let indent_px = para.list_level as f64 * 342900.0 / 9525.0;
                    let effective_width = (width_px as f64 - indent_px).max(10.0) as f32;
                    
                    let mut buffer = Buffer::new(font_system, Metrics::new(font_size_px as f32, line_height_px as f32));
                    buffer.set_size(Some(effective_width), None);
                    
                    let mut spans_data: Vec<(String, Attrs)> = Vec::new();
                    
                    for run in &para.runs {
                        let weight = if bold_default || run.bold { cosmic_text::Weight::BOLD } else { cosmic_text::Weight::NORMAL };
                        let style = if run.italic { cosmic_text::Style::Italic } else { cosmic_text::Style::Normal };
                        let family = if run.is_code || para.is_code_block { Family::Name("Courier New") } else { Family::Name("Arial") };
                        
                        let attrs = Attrs::new().weight(weight).style(style).family(family);
                        spans_data.push((run.text.clone(), attrs));
                    }
                    
                    let spans_refs: Vec<(&str, Attrs)> = spans_data.iter().map(|x| (x.0.as_str(), x.1.clone())).collect();
                    
                    buffer.set_rich_text(spans_refs.into_iter(), &Attrs::new(), Shaping::Advanced, None);
                    buffer.shape_until_scroll(font_system, true);
                    
                    let mut text_height = 0.0;
                    for run in buffer.layout_runs() {
                        text_height += run.line_height;
                    }
                    
                    total_height += text_height + paragraph_spacing_px as f32;
                }
                RichBlock::Table(_) => {
                    // Ignore for now
                }
            }
        }
        
        if total_height > 0.0 {
            total_height -= paragraph_spacing_px as f32;
        }
        
        if total_height <= height_px as f32 || current_pt <= min_pt {
            break;
        }
        
        current_pt -= 0.5;
    }
    
    current_pt as i64
}

pub(crate) fn ilm_slide_from_ir(slide: &Value, font_system: &mut FontSystem) -> Option<IlmSlide> {
    let layout_name = slide.get("layout")?.as_str()?;
    let slots = slide.get("slots")?.as_object()?;
    
    let layout_def = generated::LAYOUT_DEFINITIONS
        .iter()
        .find(|def| def.layout == layout_name)?;
        
    let mut vars = BTreeMap::new();
    for &(k, v) in layout_def.parameters {
        vars.insert(k.to_string(), v);
    }
    if let Some(params_map) = slide.get("params").and_then(Value::as_object) {
        for (k, v) in params_map {
            if let Some(num) = v.as_f64() {
                vars.insert(k.to_string(), num);
            }
        }
    }

    let slide_width_emu = 13004800_f64;
    let slide_height_emu = 7315200_f64;
    
    let mut alignment = TextAlignment::Left;
    if let Some(style) = slide.get("style").and_then(Value::as_object) {
        if let Some(align_str) = style.get("alignment").and_then(Value::as_str) {
            alignment = match align_str {
                "center" => TextAlignment::Center,
                "right" => TextAlignment::Right,
                "justify" => TextAlignment::Justify,
                _ => TextAlignment::Left,
            };
        }
    }

    let mut elements = Vec::new();
    
    for elem in layout_def.elements {
        let default_fs = elem.font_size.unwrap_or(24) as f64;
        
        let ctx_w = EvalContext { vars: &vars, reference_length: slide_width_emu, font_size_pt: default_fs };
        let ctx_h = EvalContext { vars: &vars, reference_length: slide_height_emu, font_size_pt: default_fs };
        
        let x = evaluate(elem.x, &ctx_w) as i64;
        let y = evaluate(elem.y, &ctx_h) as i64;
        let cx = evaluate(elem.width, &ctx_w) as i64;
        let cy = evaluate(elem.height, &ctx_h) as i64;
        
        let slot_value = slots.get(elem.slot);
        
        if let Some(obj) = slot_value.and_then(Value::as_object) {
            if let Some(b64_str) = obj.get("image_data").and_then(Value::as_str) {
                let image_data = b64.decode(b64_str).unwrap_or_default();
                
                let scaling = match elem.image_scaling {
                    "stretch" => ImageScaling::Stretch,
                    "cover" => ImageScaling::Cover,
                    "fit_width" => ImageScaling::FitWidth,
                    "fit_height" => ImageScaling::FitHeight,
                    _ => ImageScaling::Contain,
                };
                
                elements.push(IlmElement::Image(IlmImage {
                    x, y, cx, cy,
                    uri: format!("inline_{}.png", elem.slot),
                    image_data,
                    scaling,
                }));
                continue;
            }
        }
        
        let raw_text = normalize_slot_text(slot_value);
        let bold = elem.bold.unwrap_or(false);
        
        let mut text = raw_text.clone();
        if layout_name == "quote" && elem.slot == "attribution" {
            text = format!("— {}", text);
        }
        
        let blocks = crate::ilm::markdown::parse_markdown(&text);
        
        let font_size_pt = calculate_autofit_font_size(
            font_system,
            &blocks,
            cx,
            cy,
            default_fs,
            bold,
        );
        
        elements.push(IlmElement::Text(IlmTextRun {
            x, y, cx, cy, blocks, font_size_pt, bold, alignment
        }));
    }

    Some(IlmSlide { elements })
}

pub(crate) fn resolve_ilm_slides(parsed: &Value) -> Result<Vec<IlmSlide>, String> {
    let mut font_system = FontSystem::new();
    let slides = parsed
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| "ValidationError: expected $.slides to be an array.".to_string())?;
        
    let ilm: Vec<IlmSlide> = slides
        .iter()
        .filter_map(|s| ilm_slide_from_ir(s, &mut font_system))
        .collect();
        
    if ilm.len() != slides.len() {
        return Err(
            "RenderError: failed to resolve one or more slide layouts for ILM emission."
                .to_string(),
        );
    }
    Ok(ilm)
}
