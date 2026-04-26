use serde_json::Value;

use crate::ilm::model::{IlmElement, IlmImage, IlmSlide, IlmTextRun, IlmTable, IlmTableRow, IlmTableCell, ImageScaling, TextAlignment};
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

use crate::ilm::markdown::{RichBlock, ListType, RichParagraph};

fn measure_paragraph(
    para: &RichParagraph,
    font_system: &mut FontSystem,
    width_px: f32,
    font_size_px: f32,
    line_height_px: f32,
    bold_default: bool,
) -> f32 {
    let indent_px = para.list_level as f64 * 342900.0 / 9525.0;
    
    // Pango renders text slightly narrower than cosmic_text on Linux due to shaping engine differences.
    // To ensure cosmic_text doesn't wrap prematurely (which leaves invisible trailing gaps),
    // we apply a compensation factor to slightly expand its bounding box calculation.
    let cosmic_width_compensation = 1.15;
    let effective_width = ((width_px as f64 - indent_px) * cosmic_width_compensation).max(10.0) as f32;
    
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size_px, line_height_px));
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
    let mut line_count = 0;
    for run in buffer.layout_runs() {
        text_height += run.line_height;
        line_count += 1;
    }
    println!("measure_paragraph: w={} px={} -> {} lines", effective_width, font_size_px, line_count);
    text_height
}

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
        let font_size_px = current_pt as f32 * 96.0 / 72.0;
        let line_height_px = font_size_px * 1.2;
        let paragraph_spacing_px = font_size_px * 0.05;
        
        let mut total_height = 0.0;
        
        for block in blocks {
            match block {
                RichBlock::Paragraph(para) => {
                    let h = measure_paragraph(para, font_system, width_px, font_size_px, line_height_px, bold_default);
                    total_height += h + paragraph_spacing_px;
                }
                RichBlock::Table(table) => {
                    let num_cols = table.column_alignments.len().max(1);
                    let col_width_px = width_px / num_cols as f32;
                    for row in &table.rows {
                        let mut max_cell_height: f32 = 0.0;
                        for cell in &row.cells {
                            let mut cell_height: f32 = 0.0;
                            for p in &cell.paragraphs {
                                cell_height += measure_paragraph(p, font_system, col_width_px, font_size_px, line_height_px, bold_default) + paragraph_spacing_px;
                            }
                            if cell_height > 0.0 { cell_height -= paragraph_spacing_px; }
                            max_cell_height = max_cell_height.max(cell_height);
                        }
                        total_height += max_cell_height;
                    }
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
        
        let mut text = raw_text.clone();
        if layout_name == "quote" && elem.slot == "attribution" {
            text = format!("— {}", text);
        }
        
        let blocks = crate::ilm::markdown::parse_markdown(&text);
        
        let bold_default = elem.bold.unwrap_or(false);
        let font_size_pt = calculate_autofit_font_size(
            font_system,
            &blocks,
            cx,
            cy,
            default_fs,
            bold_default,
        );

        let font_size_px = font_size_pt as f32 * 96.0 / 72.0;
        let line_height_px = font_size_px * 1.2;
        let paragraph_spacing_px = font_size_px * 0.05;
        let width_px = cx as f32 / 9525.0;

        let mut current_y_px = 0.0;
        let mut current_text_blocks = Vec::new();

        macro_rules! flush_text {
            () => {
                if !current_text_blocks.is_empty() {
                    let mut h = 0.0;
                    for p in current_text_blocks.iter() {
                        if let RichBlock::Paragraph(para) = p {
                            h += measure_paragraph(para, font_system, width_px, font_size_px, line_height_px, bold_default) + paragraph_spacing_px;
                        }
                    }
                    if h > 0.0 { h -= paragraph_spacing_px; }
                    
                    elements.push(IlmElement::Text(IlmTextRun {
                        x,
                        y: y + (current_y_px * 9525.0) as i64,
                        cx,
                        cy: (h * 9525.0) as i64,
                        blocks: current_text_blocks.clone(),
                        font_size_pt,
                        bold: bold_default,
                        alignment: alignment.clone(),
                    }));
                    
                    current_y_px += h + paragraph_spacing_px;
                    current_text_blocks.clear();
                }
            };
        }

        for block in blocks {
            match block {
                RichBlock::Paragraph(_) => {
                    current_text_blocks.push(block);
                }
                RichBlock::Table(table) => {
                    flush_text!();
                    
                    let num_cols = table.column_alignments.len().max(1);
                    let col_width_px = width_px / num_cols as f32;
                    
                    let mut ilm_rows = Vec::new();
                    let mut total_table_height = 0.0;
                    
                    for row in &table.rows {
                        let cell_padding_x_emu = 91440.0; // 0.1 inches
                        let cell_padding_y_emu = 45720.0; // 0.05 inches
                        let cell_padding_x_px = cell_padding_x_emu / 9525.0;
                        let cell_padding_y_px = cell_padding_y_emu / 9525.0;
                        
                        let effective_col_width = col_width_px - (cell_padding_x_px * 2.0);
                        let mut max_cell_height: f32 = 0.0;
                        let mut ilm_cells = Vec::new();

                        for (c_idx, cell) in row.cells.iter().enumerate() {
                            let mut cell_height: f32 = 0.0;
                            for p in &cell.paragraphs {
                                cell_height += measure_paragraph(p, font_system, effective_col_width, font_size_px, line_height_px, bold_default) + paragraph_spacing_px;
                            }
                            if cell_height > 0.0 { cell_height -= paragraph_spacing_px; }
                            cell_height += (cell_padding_y_px * 2.0);
                            max_cell_height = max_cell_height.max(cell_height);
                            
                            ilm_cells.push(IlmTableCell {
                                text: IlmTextRun {
                                    x: 0, y: 0, cx: 0, cy: 0, // Unused internal coords
                                    blocks: cell.paragraphs.iter().map(|p| RichBlock::Paragraph(p.clone())).collect(),
                                    font_size_pt,
                                    bold: row.is_header || bold_default,
                                    alignment: cell.alignment.clone(),
                                },
                                alignment: cell.alignment.clone(),
                            });
                        }
                        
                        total_table_height += max_cell_height;
                        ilm_rows.push(IlmTableRow {
                            cells: ilm_cells,
                            is_header: row.is_header,
                            row_height_emu: (max_cell_height * 9525.0) as i64,
                        });
                    }
                    
                    let table_margin_px = font_size_px * 0.5;
                    current_y_px += table_margin_px;

                    elements.push(IlmElement::Table(IlmTable {
                        x,
                        y: y + (current_y_px * 9525.0) as i64,
                        cx,
                        cy: (total_table_height * 9525.0) as i64,
                        rows: ilm_rows,
                        col_widths_emu: vec![(col_width_px * 9525.0) as i64; num_cols],
                    }));
                    
                    current_y_px += total_table_height + table_margin_px;
                }
            }
        }
        flush_text!();
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
