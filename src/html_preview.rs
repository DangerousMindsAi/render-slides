use serde_json::Value;

use crate::schema::{parse_ir, validate_ir};
use crate::templating::template_registry;
use crate::theme::render_theme_style_block;

pub(crate) fn normalize_slot_text(slot_value: Option<&Value>) -> String {
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

pub(crate) fn html_escape(input: &str) -> String {
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

pub(crate) fn render_preview_html(ir_json: &str) -> Result<String, String> {
    let parsed = parse_ir(ir_json)?;
    render_preview_html_from_parsed(&parsed)
}

pub(crate) fn render_preview_html_from_parsed(parsed: &Value) -> Result<String, String> {
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
