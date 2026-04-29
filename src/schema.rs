use std::sync::LazyLock;

use jsonschema::{error::ValidationErrorKind, Draft, Validator};
use serde_json::Value;



const IR_SCHEMA_JSON: &str = include_str!("../schemas/v1/ir.schema.json");

static IR_VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(IR_SCHEMA_JSON)
        .map_err(|err| format!("Schema compile error: invalid JSON schema: {err}"))?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .map_err(|err| format!("Schema compile error: {err}"))
});

pub(crate) fn validate_ir(parsed: &Value) -> Result<(), String> {
    let validator = IR_VALIDATOR.as_ref().map_err(ToString::to_string)?;
    let mut errors = validator.iter_errors(parsed);
    if let Some(first) = errors.next() {
        return Err(format_validation_error(first));
    }

    validate_layout_required_slots(parsed)
}

pub(crate) fn parse_ir(ir_json: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(ir_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    validate_ir(&parsed)?;
    Ok(parsed)
}

pub(crate) fn describe_layouts() -> crate::types::LayoutsSummary {
    let mut slide_layouts = Vec::new();
    for layout_def in crate::generated::LAYOUT_DEFINITIONS {
        // Exclude internal layouts
        if layout_def.layout == "refinement_controls" || layout_def.layout == "image_test" {
            continue;
        }

        let (req, opt) = match layout_def.layout {
            "title" => (vec!["title", "subtitle"], vec![]),
            "title_body" => (vec!["title", "body"], vec!["subtitle"]),
            "two_column" => (vec!["title", "left", "right"], vec!["subtitle"]),
            "section" => (vec!["title", "subtitle"], vec![]),
            "image_focus" => (vec!["title", "image", "caption"], vec!["subtitle"]),
            "quote" => (vec!["quote", "attribution"], vec![]),
            "comparison" => (vec!["title", "left", "right"], vec!["subtitle"]),
            _ => (vec![], vec![]),
        };

        slide_layouts.push(crate::types::LayoutSpec {
            name: layout_def.layout,
            description: layout_def.description,
            required_slots: req,
            optional_slots: opt,
        });
    }

    crate::types::LayoutsSummary {
        version: "0.1",
        slide_layouts,
    }
}

pub(crate) fn get_initial_instructions() -> String {
    r#"Generate a strictly valid JSON response containing a `slides` array matching the IR format. Example:
{
  "slides": [
    {
      "id": "slide_1",
      "layout": "title_body",
      "slots": {
        "title": "My Title",
        "body": "Markdown text here"
      }
    }
  ]
}

# Formatting Rules
- Every slide object MUST have a unique string `"id"` property (e.g., `"id": "slide_1"`). This is strictly required for the rendering engine to track components across layout tweaks.
- If a layout requires an image slot (e.g., `image`, `stretch_img`), you MUST pass it as an object referencing the `FileId`, formatted exactly as: `{"image_data": "<FileId>"}`.
- For scientific exponents, subscripts, or superscripts, ALWAYS use explicit HTML tags (e.g., 10<sup>-5</sup> or H<sub>2</sub>O) instead of literal Unicode superscript characters (e.g. 10⁻⁵). Do NOT wrap these HTML tags in code block backticks!
- CRITICAL JSON ESCAPING: When generating lengthy text or Markdown tables for JSON tool parameters (like `body`), you MUST properly escape all newlines as `\n`! Never output raw line breaks inside a JSON string.

You operate on a strict 960x720 canvas! Design for impact by correctly breaking up large bodies of text across multiple clean slides!
"#.to_string()
}

pub(crate) fn get_tweak_instructions() -> String {
    r#"# Tweaking and Refinement
You are reviewing the generated presentation. If you need to modify the slides, you may ONLY use the available tweaking operations provided.

The available tweaks are grouped into categories:
1. **Qualitative Tweaks**: Shift the slide conceptually (e.g., `increase` font size, `set_layout`).
2. **Quantitative Tweaks**: Force an absolute value (e.g., `set_font_size`, `set_text`).
3. **Structural Operations**: Modify the array of slides (`add_slide`, `remove_slide`, `reorder_slide`).

To apply a tweak, provide an array of operation objects. For example, to make the title on slide 0 larger:
[
  {
    "path": "slides[id=slide_1].style.title.font_size",
    "operation": "increase",
    "step": 1
  }
]

CRITICAL VISUAL QA & AUTO-FIT: You will receive an image of the rendered layout. You MUST visually inspect this image carefully!
1. Compare the rendered image against your expectations.
2. The rendering engine has an AUTO-FIT feature. Font sizes are automatically scaled down to fit the bounding box. If text STILL overflows the slide, it means the absolute minimum font size (10pt) was reached! 
3. DO NOT attempt to fix overflow by using the `decrease` font_size tweak! It will not work if the text is too long.
4. Instead, you MUST resolve overflow by either summarizing the text (using `set_text`), or splitting the content into an additional slide (using the `add_slide` structural operation).
5. If there is a problem you cannot correct after trying, you MUST note the uncorrectable problem explicitly in your response log.
"#.to_string()
}

pub(crate) fn describe_tweaks(ir_json: &str) -> Result<crate::types::TweakInstructions, String> {
    let parsed = parse_ir(ir_json)?;
    let empty_slides = vec![];
    let slides = parsed.get("slides").and_then(Value::as_array).unwrap_or(&empty_slides);

    let mut qualitative = Vec::new();
    let mut quantitative = Vec::new();

    let mut seen = std::collections::HashSet::new();

    for (_, slide) in slides.iter().enumerate() {
        let layout = slide.get("layout").and_then(Value::as_str).unwrap_or("");
        let Some(slide_id) = slide.get("id").and_then(Value::as_str) else { continue; };
        let Some(layout_def) = crate::generated::LAYOUT_DEFINITIONS.iter().find(|l| l.layout == layout) else { continue; };

        for spec in crate::generated::TEMPLATE_OPERATION_SPECS {
            let path = spec.path;
            let parts: Vec<&str> = path.split('.').collect();
            
            let is_valid = if parts.len() >= 2 {
                if parts[1] == "layout" {
                    true
                } else if parts[1] == "style" {
                    if parts.len() > 2 && (parts[2] == "alignment" || parts[2] == "background") {
                        true
                    } else if parts.len() > 2 {
                        layout_def.elements.iter().any(|e| e.slot == parts[2])
                    } else {
                        false
                    }
                } else if parts[1] == "slots" && parts.len() > 2 {
                    layout_def.elements.iter().any(|e| e.slot == parts[2])
                } else {
                    false
                }
            } else {
                false
            };

            if is_valid {
                let instantiated_path = path.replacen("slides[*]", &format!("slides[id={}]", slide_id), 1);
                
                let key = format!("{}:{}", instantiated_path, spec.name);
                if !seen.insert(key) {
                    continue;
                }

                let op = crate::types::OperationSpec {
                    path: Some(instantiated_path),
                    name: spec.name,
                    description: spec.description,
                    params: spec.params.to_vec(),
                    bounds: spec.bounds,
                };

                match spec.name {
                    "increase" | "decrease" | "set_alignment" | "set_layout" => qualitative.push(op),
                    "set_font_size" | "set_text" => quantitative.push(op),
                    _ => qualitative.push(op), // default fallback
                }
            }
        }
    }

    let structural = vec![
        crate::types::OperationSpec {
            path: None,
            name: "add_slide",
            description: "Appends a new slide with the specified layout to the presentation.",
            params: vec!["layout"],
            bounds: "layout must be a valid layout name",
        },
        crate::types::OperationSpec {
            path: None,
            name: "remove_slide",
            description: "Removes the slide with the target ID.",
            params: vec!["id"],
            bounds: "id must refer to an existing slide id",
        },
        crate::types::OperationSpec {
            path: None,
            name: "reorder_slide",
            description: "Moves a slide from its current index to a new target index.",
            params: vec!["id", "to_index"],
            bounds: "id must refer to an existing slide id, to_index must be within bounds",
        },
    ];

    Ok(crate::types::TweakInstructions {
        qualitative_tweaks: qualitative,
        quantitative_tweaks: quantitative,
        structural_operations: structural,
    })
}

fn validate_layout_required_slots(parsed: &Value) -> Result<(), String> {
    let Some(slides) = parsed.get("slides").and_then(Value::as_array) else {
        return Ok(());
    };

    for (index, slide) in slides.iter().enumerate() {
        let Some(layout) = slide.get("layout").and_then(Value::as_str) else {
            continue;
        };

        let (required_slots, optional_slots): (&[&str], &[&str]) = match layout {
            "title" => (&["title", "subtitle"], &[]),
            "title_body" => (&["title", "body"], &["subtitle"]),
            "two_column" => (&["title", "left", "right"], &["subtitle"]),
            "section" => (&["title", "subtitle"], &[]),
            "image_focus" => (&["title", "image", "caption"], &["subtitle"]),
            "quote" => (&["quote", "attribution"], &[]),
            "comparison" => (&["title", "left", "right"], &["subtitle"]),
            _ => continue,
        };

        let Some(slots) = slide.get("slots").and_then(Value::as_object) else {
            continue;
        };

        for required_slot in required_slots {
            if !slots.contains_key(*required_slot) {
                let mut provided_slots: Vec<&str> = slots.keys().map(String::as_str).collect();
                provided_slots.sort_unstable();
                let suggested_fix = format!("Add slots.{required_slot} as a string value.");
                return Err(format!(
                    "ValidationError: missing required slot '{required_slot}' for layout '{layout}' at $.slides[{index}].slots. expected_required={required_slots:?}; optional={optional_slots:?}; provided={provided_slots:?}; suggested_fix=\"{suggested_fix}\"."
                ));
            }
        }
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
